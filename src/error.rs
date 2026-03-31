#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("API error {}: {}", .0.status, .0.message)]
    Api(#[from] ApiError),

    #[error("HTTP client error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}

#[derive(Debug, Clone)]
pub struct ApiError {
    pub code: String,
    pub message: String,
    pub status: u16,
    pub request_id: String,
}

impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "[{}] {} (request_id: {})",
            self.code, self.message, self.request_id
        )
    }
}

impl std::error::Error for ApiError {}
