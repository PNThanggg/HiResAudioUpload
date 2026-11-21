use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ErrorCode {
    InvalidFormat,
    TooLarge,
    TooShort,
    Silent,
    Timeout,
    Internal,
}

#[derive(Debug, Clone, thiserror::Error, Serialize)]
#[error("{code:?}: {message}")]
#[serde(rename_all = "camelCase")]
pub struct PipelineError {
    pub code: ErrorCode,
    pub message: String,
}

impl PipelineError {
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self { code, message: message.into() }
    }
}
