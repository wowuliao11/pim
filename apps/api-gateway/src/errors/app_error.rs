use actix_web::{http::StatusCode, HttpResponse, ResponseError};
use thiserror::Error;

use super::ErrorResponse;
use super::UserError;

/// Application error types
/// Maps to appropriate HTTP status codes and error responses
#[derive(Debug, Error)]
pub enum AppError {
    #[error(transparent)]
    User(#[from] UserError),

    #[error("Internal error")]
    Internal(#[from] anyhow::Error),
}

impl AppError {
    pub fn kind(&self) -> &'static str {
        match self {
            Self::User(_) => "user",
            Self::Internal(_) => "internal",
        }
    }
}

impl ResponseError for AppError {
    fn status_code(&self) -> StatusCode {
        match self {
            Self::User(UserError::NotFound { .. }) => StatusCode::NOT_FOUND,
            Self::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    fn error_response(&self) -> HttpResponse {
        let status = self.status_code();
        let body = ErrorResponse::new(status, self.to_string());
        HttpResponse::build(status).json(body)
    }
}

impl From<std::io::Error> for AppError {
    fn from(err: std::io::Error) -> Self {
        AppError::Internal(anyhow::Error::from(err))
    }
}
