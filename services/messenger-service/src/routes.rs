use std::convert::Infallible;

use uuid::Uuid;
use warp::{reject, Filter, Rejection, Reply};

use crate::models::{MessageRejection, SendRequest};
use crate::service::{process_message, AppState, ServiceError};

fn with_state(state: AppState) -> impl Filter<Extract = (AppState,), Error = Infallible> + Clone {
    warp::any().map(move || state.clone())
}

async fn handle_get_health() -> Result<impl Reply, Rejection> {
    Ok(warp::reply::with_status(
        warp::reply(),
        warp::http::StatusCode::OK,
    ))
}

async fn handle_send(req: SendRequest, state: AppState) -> Result<impl Reply, Rejection> {
    let event_id = req
        .event_id
        .clone()
        .unwrap_or_else(|| Uuid::new_v4().to_string());

    match process_message(&state, &event_id, &req.event_type, &req.to, &req.variables).await {
        Ok(response) => Ok(warp::reply::json(&response)),
        Err(ServiceError::NoRoute(event_type)) => Err(reject::custom(MessageRejection(format!(
            "no provider routed for {event_type}"
        )))),
        Err(e) => {
            tracing::error!(error = %e, "send failed");
            Err(reject::custom(MessageRejection(e.to_string())))
        }
    }
}

async fn handle_rejection(err: Rejection) -> std::result::Result<impl Reply, Infallible> {
    if let Some(custom) = err.find::<MessageRejection>() {
        Ok(warp::reply::with_status(
            warp::reply::json(&serde_json::json!({ "error": custom.0 })),
            warp::http::StatusCode::BAD_REQUEST,
        ))
    } else {
        Ok(warp::reply::with_status(
            warp::reply::json(&serde_json::json!({ "error": "internal server error" })),
            warp::http::StatusCode::INTERNAL_SERVER_ERROR,
        ))
    }
}

/// Build the router.
pub fn build_router(
    state: AppState,
) -> impl Filter<Extract = impl Reply, Error = Infallible> + Clone {
    let get_health = warp::get()
        .and(warp::path("health"))
        .and_then(handle_get_health);

    let send = warp::post()
        .and(warp::path("send"))
        .and(warp::body::json())
        .and(with_state(state))
        .and_then(handle_send);

    get_health.or(send).recover(handle_rejection)
}
