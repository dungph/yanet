use serde::Serialize;
use serde_json::Value;
use std::fmt::Display;
use tide::Response;

pub mod account;
pub mod attribute;
pub mod device;
pub mod peer;

#[derive(Serialize)]
pub struct ApiResult {
    success: bool,
    message: String,
    payload: Value,
}

impl ApiResult {
    pub fn success(message: impl Display, payload: impl Serialize) -> Self {
        Self {
            success: true,
            message: message.to_string(),
            payload: serde_json::to_value(payload).unwrap(),
        }
    }
    pub fn failure(message: impl Display, payload: impl Serialize) -> Self {
        Self {
            success: false,
            message: message.to_string(),
            payload: serde_json::to_value(payload).unwrap(),
        }
    }
}

impl Into<tide::Result<Response>> for ApiResult {
    fn into(self) -> tide::Result<Response> {
        Ok(Response::builder(200)
            .body(serde_json::to_value(&self)?)
            .build())
    }
}
