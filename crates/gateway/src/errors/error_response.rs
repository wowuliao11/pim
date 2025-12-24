use actix_web::http::StatusCode;
use chrono::{DateTime, Utc};
use serde::Serialize;

/// Standardized error response format
/// Consistent JSON structure for all API errors
#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub success: bool,
    pub error: ErrorDetail,
}

#[derive(Debug, Serialize)]
pub struct ErrorDetail {
    pub code: u16,
    pub message: String,
    pub timestamp: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<Vec<String>>,
}

impl ErrorResponse {
    pub fn new(status: StatusCode, message: String) -> Self {
        Self {
            success: false,
            error: ErrorDetail {
                code: status.as_u16(),
                message,
                timestamp: Utc::now(),
                details: None,
            },
        }
    }

    pub fn with_details(status: StatusCode, message: String, details: Vec<String>) -> Self {
        Self {
            success: false,
            error: ErrorDetail {
                code: status.as_u16(),
                message,
                timestamp: Utc::now(),
                details: Some(details),
            },
        }
    }
}
