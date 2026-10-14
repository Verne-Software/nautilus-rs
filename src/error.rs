use serde_json::{Map, Value};

/// The top-level error type returned by every fallible SDK operation.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// A non-2xx response from the Verne API.
    ///
    /// Branch on [`ApiError::status`] and [`ApiError::code`]. The error is boxed to keep
    /// `Result<T, Error>` small; its fields are reachable directly (`e.status`), and
    /// `*e` moves the [`ApiError`] out.
    #[error("API error {}: {}", .0.status, .0.message)]
    Api(Box<ApiError>),

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

    /// A signed token (see `GateTokenVerifier`, feature `jwt`) must not be trusted.
    /// [`TokenError::reason`] says why.
    #[error("Token error: {0}")]
    Token(#[from] TokenError),
}

impl From<ApiError> for Error {
    fn from(error: ApiError) -> Self {
        Error::Api(Box::new(error))
    }
}

/// A structured error returned by the Verne API.
///
/// Returned inside [`Error::Api`] when the server responds with a non-2xx status. Every API
/// route answers with `{"error": {"code", "message", "request_id", …}}`. Other shapes are
/// still read, as a fallback that costs nothing: the string form `{"error": "<sentence>"}`
/// Clockwork used before October 2026, and an identity-service body with a numeric `code`
/// (still what Kratos' own `/auth/*` self-service routes answer). Parsing never fails — an
/// unrecognised body gives `code = "unknown"` and `message = "HTTP <status>"`.
///
/// # Branching
///
/// Compare [`status`](Self::status) and [`code`](Self::code); codes are an open set of
/// strings, so keep a catch-all arm:
///
/// ```no_run
/// # use nautilus_rs::{Error, ApiError};
/// # fn handle(err: Error) {
/// match err {
///     Error::Api(e) if e.status == 403 && e.code == "mfa_required" => {
///         // e.details["required_aal"] == "aal2"
///     }
///     Error::Api(e) if e.status == 402 => {
///         // never retried; e.details has `plan`, `limit`, `current`
///     }
///     Error::Api(e) if e.status == 429 => {
///         // e.retry_after is the seconds to wait, when the server said
///     }
///     other => eprintln!("{other}"),
/// }
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct ApiError {
    /// Machine-readable error code (e.g. `"invalid_payload"`, `"forbidden"`,
    /// `"conflict"`, `"mfa_required"`, `"ACTIVE_USER_LIMIT_REACHED"`).
    ///
    /// Codes are an open set; do not assume you have seen them all. `"unknown"` when the
    /// response carried no string code — which the API's own routes no longer do; it is
    /// left for a body that did not come from the gateway (a proxy in between, an
    /// identity-service answer passed through on `/auth/*`).
    pub code: String,
    /// Human-readable description of the error.
    pub message: String,
    /// HTTP status code (e.g. `404`, `429`).
    pub status: u16,
    /// Unique request identifier for support/debugging. Empty when the response did
    /// not carry one (the Passepartout bridge and `POST /v1/gate/tokens` do not).
    pub request_id: String,
    /// The members of `error` other than `code`, `message` and `request_id` — for
    /// example `plan`, `limit` and `current` on a `402` limit refusal, `kind` on a
    /// Clockwork one, `required_aal` and `current_aal` on `403 mfa_required`. Empty when
    /// there are none.
    pub details: Map<String, Value>,
    /// Seconds from the `Retry-After` header, when the response had one (delta-seconds
    /// only). On a `429` it is how long to wait; for `RELAY_MESSAGE_LIMIT_REACHED` it
    /// runs to the 1st of next month.
    pub retry_after: Option<u64>,
    /// The raw parsed body, kept so an unrecognised shape stays diagnosable. `None`
    /// when the body was not JSON.
    pub body: Option<Value>,
}

impl ApiError {
    /// Build an error from a response body of *unknown* shape. Never fails.
    pub(crate) fn from_response(status: u16, body: &[u8], retry_after: Option<u64>) -> Self {
        let body: Option<Value> = serde_json::from_slice(body).ok();
        let envelope = body
            .as_ref()
            .and_then(|b| b.get("error"))
            .and_then(Value::as_object);

        let mut details = Map::new();
        if let Some(envelope) = envelope {
            for (key, value) in envelope {
                if !matches!(key.as_str(), "code" | "message" | "request_id") {
                    details.insert(key.clone(), value.clone());
                }
            }
        }

        let string_member = |key: &str| {
            envelope
                .and_then(|e| e.get(key))
                .and_then(Value::as_str)
                .map(str::to_owned)
        };

        Self {
            // A non-string code (Kratos' numeric `404`) is never used.
            code: string_member("code").unwrap_or_else(|| "unknown".into()),
            message: extract_message(body.as_ref(), status),
            status,
            request_id: string_member("request_id").unwrap_or_default(),
            details,
            retry_after,
            body,
        }
    }
}

/// Best-effort human-readable message from an arbitrary error body: the envelope's
/// `message`, a string `error` (Clockwork), a `detail` string or list, a bare `message`.
fn extract_message(body: Option<&Value>, status: u16) -> String {
    if let Some(body) = body {
        match body.get("error") {
            Some(Value::Object(envelope)) => {
                if let Some(message) = envelope.get("message").and_then(Value::as_str) {
                    return message.to_owned();
                }
            }
            Some(Value::String(sentence)) if !sentence.trim().is_empty() => {
                return sentence.clone();
            }
            _ => {}
        }

        match body.get("detail") {
            Some(Value::String(detail)) => return detail.clone(),
            Some(Value::Array(items)) => {
                let messages: Vec<&str> = items
                    .iter()
                    .filter_map(|item| item.get("msg").and_then(Value::as_str))
                    .collect();
                if !messages.is_empty() {
                    return messages.join("; ");
                }
            }
            _ => {}
        }

        if let Some(message) = body.get("message").and_then(Value::as_str) {
            return message.to_owned();
        }
    }
    format!("HTTP {status}")
}

impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{}] {}", self.code, self.message)?;
        if !self.request_id.is_empty() {
            write!(f, " (request_id: {})", self.request_id)?;
        }
        Ok(())
    }
}

impl std::error::Error for ApiError {}

/// Why a signed token was refused, in [`TokenError::reason`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum TokenErrorReason {
    /// Not a compact JWS, or a part is not base64url JSON, or the header has no `kid`.
    Malformed,
    /// The header's `alg` is not `ES256` (`none` and `HS256` included).
    UnsupportedAlg,
    /// No key with the token's `kid` in the tenant's key set, even after a refetch.
    UnknownKid,
    /// The signature does not verify under the key the `kid` names.
    BadSignature,
    /// `iss` is not this tenant's issuer — a token of another Gate tenant.
    WrongIssuer,
    /// `exp` has passed (beyond the leeway).
    Expired,
    /// `nbf` or `iat` is in the future (beyond the leeway).
    NotYetValid,
    /// `exp` or `sub` is missing.
    MissingClaim,
    /// The tenant's key set could not be fetched.
    JwksUnavailable,
}

impl TokenErrorReason {
    /// The reason as the lowercase snake-case word the other SDKs use.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Malformed => "malformed",
            Self::UnsupportedAlg => "unsupported_alg",
            Self::UnknownKid => "unknown_kid",
            Self::BadSignature => "bad_signature",
            Self::WrongIssuer => "wrong_issuer",
            Self::Expired => "expired",
            Self::NotYetValid => "not_yet_valid",
            Self::MissingClaim => "missing_claim",
            Self::JwksUnavailable => "jwks_unavailable",
        }
    }
}

impl std::fmt::Display for TokenErrorReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A signed token that must not be trusted, returned inside [`Error::Token`] by
/// `GateTokenVerifier` (feature `jwt`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TokenError {
    /// Why the token was refused.
    pub reason: TokenErrorReason,
    /// A sentence for people.
    pub message: String,
}

impl TokenError {
    #[cfg_attr(not(feature = "jwt"), allow(dead_code))]
    pub(crate) fn new(reason: TokenErrorReason, message: impl Into<String>) -> Self {
        Self {
            reason,
            message: message.into(),
        }
    }
}

impl std::fmt::Display for TokenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{}] {}", self.reason, self.message)
    }
}

impl std::error::Error for TokenError {}
