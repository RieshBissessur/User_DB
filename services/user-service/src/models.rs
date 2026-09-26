use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub struct RequestPassword {
    pub email: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct OTPSubmit {
    pub otp: String,
    pub email: String,
    pub password: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
    pub version: f32,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct LoginResponse {
    pub session_key: String,
    pub username: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct UserDataRequest {
    pub session_key: String,
    pub username: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct RegisterUser {
    pub username: String,
    pub email: String,
    pub password: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct UserData {
    pub username: String,
    pub email: Option<String>,
    pub avatar: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct UserDataUpdate {
    pub username: String,
    pub new_username: Option<String>,
    pub email: Option<String>,
    pub avatar: Option<String>,
    pub session_key: String,
}

#[derive(Debug)]
pub struct CustomRejection(pub String);

impl warp::reject::Reject for CustomRejection {}
