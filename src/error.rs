/// The top-level error type returned by every fallible SDK operation.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// A non-2xx response from the Verne API containing a structured error
    /// body.
    #[error("API error {}: {}", .0.status, .0.message)]
    Api(#[from] ApiError),

    /// A lower-level HTTP or networking failure (connection refused, timeout,
    /// TLS error, …).
    #[error("HTTP client error: {0}")]
    Http(#[from] reqwest::Error),

    /// The client was not configured correctly (e.g. a required API key was
    /// not provided).
    #[error("Configuration error: {0}")]
    Config(String),

    /// The response body could not be deserialised as the expected type.
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}

/// A structured error returned by the Verne API.
///
/// Returned inside [`Error::Api`] when the server responds with a non-2xx
/// status and a JSON error envelope.
#[derive(Debug, Clone)]
pub struct ApiError {
    /// Machine-readable error code (e.g. `"identity_not_found"`).
    pub code: String,
    /// Human-readable description of the error.
    pub message: String,
    /// HTTP status code (e.g. `404`, `422`).
    pub status: u16,
    /// Unique request identifier for support/debugging.
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
