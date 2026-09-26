use chrono::Local;
use sqlx::mysql::MySqlPool;
use std::convert::Infallible;
use uuid::Uuid;
use warp::{reject, Filter, Rejection, Reply};

use crate::db as repo;
use crate::models::*;
use crate::producer::Producer;
use crate::{normalize, otp, password, session_key, version};

/// Shared state handed to every handler.
#[derive(Clone)]
pub struct AppState {
    pub pool: MySqlPool,
    pub otp_ttl_minutes: i64,
    pub session_ttl_minutes: i64,
    pub app_version: f32,
    /// Absent in tests; the outbox publisher needs it at runtime.
    pub producer: Option<Producer>,
}

fn with_state(state: AppState) -> impl Filter<Extract = (AppState,), Error = Infallible> + Clone {
    warp::any().map(move || state.clone())
}

fn bad(message: impl Into<String>) -> Rejection {
    reject::custom(CustomRejection(message.into()))
}

fn internal<E: std::fmt::Display>(err: E) -> Rejection {
    tracing::error!("internal error: {err}");
    reject::custom(CustomRejection("Internal Error01".to_string()))
}

async fn handle_get_health() -> Result<impl Reply, Rejection> {
    Ok(warp::reply::with_status(
        warp::reply(),
        warp::http::StatusCode::OK,
    ))
}

async fn handle_custom_rejection(err: Rejection) -> std::result::Result<impl Reply, Infallible> {
    if let Some(custom_error) = err.find::<CustomRejection>() {
        let response = warp::reply::with_status(
            warp::reply::html(format!("Bad Request: {}", custom_error.0)),
            warp::http::StatusCode::BAD_REQUEST,
        );
        Ok(response)
    } else {
        Ok(warp::reply::with_status(
            warp::reply::html("Internal Server Error".to_string()),
            warp::http::StatusCode::INTERNAL_SERVER_ERROR,
        ))
    }
}

async fn handle_register(
    user_data: RegisterUser,
    state: AppState,
) -> Result<impl Reply, Rejection> {
    let username = normalize::normalize(&user_data.username);
    let email = normalize::normalize(&user_data.email);

    if repo::find_user_by_username(&state.pool, &username)
        .await
        .map_err(internal)?
        .is_some()
    {
        return Err(bad("Username exists"));
    }
    if repo::find_user_by_email(&state.pool, &email)
        .await
        .map_err(internal)?
        .is_some()
    {
        return Err(bad("Username or email already associated with an account"));
    }

    let guid = Uuid::new_v4().to_string();
    let password_hash = password::hash_password(&user_data.password).map_err(internal)?;

    repo::insert_user(&state.pool, &guid, &username, &email, &password_hash)
        .await
        .map_err(|_| bad("Username or email already associated with an account"))?;

    let response = LoginResponse {
        session_key: String::new(),
        username,
    };
    Ok(warp::reply::json(&response))
}

async fn handle_login(login: LoginRequest, state: AppState) -> Result<impl Reply, Rejection> {
    let login_id = normalize::normalize(&login.username);

    let user = repo::find_user_by_login(&state.pool, &login_id)
        .await
        .map_err(internal)?
        .ok_or_else(|| bad("Incorrect username or password for this account."))?;

    if !version::version_supported(login.version, state.app_version) {
        return Err(bad("Please update application version"));
    }

    if !password::verify_password(&login.password, &user.password_hash) {
        return Err(bad("Incorrect username or password for this account."));
    }

    let session_key = session_key::generate_session_key();

    // Session + last_logged_in + outbox event, atomically.
    let event = events::Event::login(&user.username, &user.email);
    let payload = serde_json::to_value(&event).map_err(internal)?;
    repo::record_login(
        &state.pool,
        user.id,
        &session_key,
        state.session_ttl_minutes,
        repo::NewOutbox {
            event_id: &event.event_id,
            topic: events::USER_EVENTS_TOPIC,
            payload: &payload,
        },
    )
    .await
    .map_err(|_| bad("Unable to save session data"))?;

    Ok(warp::reply::json(&LoginResponse {
        session_key,
        username: user.username,
    }))
}

async fn handle_user_data_retrieval(
    request_data: UserDataRequest,
    state: AppState,
) -> Result<impl Reply, Rejection> {
    let username = normalize::normalize(&request_data.username);

    let user = repo::find_user_by_username(&state.pool, &username)
        .await
        .map_err(internal)?
        .ok_or_else(|| bad("Can not read authentication key"))?;

    let session = repo::find_valid_session(&state.pool, user.id, Local::now().naive_local())
        .await
        .map_err(internal)?
        .ok_or_else(|| bad("Can not read authentication key"))?;

    if session.session_key != request_data.session_key {
        return Err(bad("Incorrect authentication key"));
    }

    Ok(warp::reply::json(&UserData {
        username: user.username,
        email: Some(user.email),
        avatar: user.avatar,
    }))
}

async fn handle_user_data_update(
    request_data: UserDataUpdate,
    state: AppState,
) -> Result<impl Reply, Rejection> {
    let username = normalize::normalize(&request_data.username);

    let user = repo::find_user_by_username(&state.pool, &username)
        .await
        .map_err(internal)?
        .ok_or_else(|| bad("Can not read authentication key"))?;

    let session = repo::find_valid_session(&state.pool, user.id, Local::now().naive_local())
        .await
        .map_err(internal)?
        .ok_or_else(|| bad("Can not read authentication key"))?;

    if session.session_key != request_data.session_key {
        return Err(bad("Incorrect authentication key"));
    }

    let new_username = request_data
        .new_username
        .map(|u| normalize::normalize(&u))
        .unwrap_or_else(|| user.username.clone());
    let email = request_data
        .email
        .map(|e| normalize::normalize(&e))
        .unwrap_or_else(|| user.email.clone());
    let avatar = request_data.avatar.or_else(|| user.avatar.clone());

    repo::update_profile(
        &state.pool,
        user.id,
        &new_username,
        &email,
        avatar.as_deref(),
    )
    .await
    .map_err(|_| bad("Username or email already associated with an account"))?;

    Ok(warp::reply::json(&UserData {
        username: new_username,
        email: Some(email),
        avatar,
    }))
}

async fn request_password_reset(
    req: RequestPassword,
    state: AppState,
) -> Result<impl Reply, Rejection> {
    let email = normalize::normalize(&req.email);

    let user = repo::find_user_by_email(&state.pool, &email)
        .await
        .map_err(internal)?
        .ok_or_else(|| bad("Failed to find email"))?;

    let otp_string = otp::generate_otp();
    let expires = otp::expires_at(Local::now().naive_local(), state.otp_ttl_minutes);

    // OTP row + outbox event, atomically.
    let event = events::Event::password_reset(&user.username, &user.email, &otp_string);
    let payload = serde_json::to_value(&event).map_err(internal)?;
    repo::create_password_reset(
        &state.pool,
        user.id,
        &otp_string,
        expires,
        repo::NewOutbox {
            event_id: &event.event_id,
            topic: events::USER_EVENTS_TOPIC,
            payload: &payload,
        },
    )
    .await
    .map_err(|_| bad("Failed to write otp data"))?;

    Ok(warp::reply::json(&"OTP Sent to email address".to_string()))
}

async fn check_otp(req: OTPSubmit, state: AppState) -> Result<impl Reply, Rejection> {
    let email = normalize::normalize(&req.email);

    let user = repo::find_user_by_email(&state.pool, &email)
        .await
        .map_err(internal)?
        .ok_or_else(|| bad("Failed to find email"))?;

    let now = Local::now().naive_local();
    let valid = repo::find_otp_if_valid(&state.pool, user.id, &req.otp, now)
        .await
        .map_err(internal)?;

    if valid.is_some() {
        let password_hash = password::hash_password(&req.password).map_err(internal)?;
        repo::update_password(&state.pool, user.id, &password_hash)
            .await
            .map_err(internal)?;
        repo::delete_otp(&state.pool, user.id)
            .await
            .map_err(internal)?;
        return Ok(warp::reply::json(&"OTP match and valid".to_string()));
    }

    Ok(warp::reply::json(&"OTP invalid or expired".to_string()))
}

/// Build the warp router. Handlers are constructible in tests by passing a
/// test pool/state (with no Kafka producer).
pub fn build_router(
    state: AppState,
) -> impl Filter<Extract = impl Reply, Error = Infallible> + Clone {
    let get_health = warp::get()
        .and(warp::path("health"))
        .and_then(handle_get_health);

    let register_user = warp::post()
        .and(warp::path("register"))
        .and(warp::body::json())
        .and(with_state(state.clone()))
        .and_then(handle_register);

    let login = warp::post()
        .and(warp::path("login"))
        .and(warp::body::json())
        .and(with_state(state.clone()))
        .and_then(handle_login);

    let retrieve_user_data = warp::post()
        .and(warp::path("user_data"))
        .and(warp::body::json())
        .and(with_state(state.clone()))
        .and_then(handle_user_data_retrieval);

    let reset_request = warp::post()
        .and(warp::path("reset_request"))
        .and(warp::body::json())
        .and(with_state(state.clone()))
        .and_then(request_password_reset);

    let otp_check = warp::post()
        .and(warp::path("check_otp"))
        .and(warp::body::json())
        .and(with_state(state.clone()))
        .and_then(check_otp);

    let update_user_data = warp::post()
        .and(warp::path("update_user_data"))
        .and(warp::body::json())
        .and(with_state(state))
        .and_then(handle_user_data_update);

    register_user
        .or(login)
        .or(retrieve_user_data)
        .or(update_user_data)
        .or(reset_request)
        .or(otp_check)
        .or(get_health)
        .recover(handle_custom_rejection)
}
