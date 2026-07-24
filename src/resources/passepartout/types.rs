/// A Telegram user profile returned once a login flow completes.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct TelegramUser {
    /// Telegram user ID (numeric, delivered as a string).
    pub id: String,
    /// Telegram @username, if the user has one set.
    #[serde(default)]
    pub username: Option<String>,
    /// The user's first name.
    #[serde(default)]
    pub first_name: Option<String>,
    /// URL of the user's profile photo, if available.
    #[serde(default)]
    pub photo_url: Option<String>,
}

/// The result of starting a Passepartout login flow.
///
/// Returned by [`Passepartout::login_start`](super::Passepartout::login_start).
/// Direct the end-user to `deep_link` (a `https://t.me/…` link) and poll
/// [`Passepartout::login_status`](super::Passepartout::login_status) with the
/// `nonce` until the flow completes.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct LoginStart {
    /// Opaque one-time value identifying this login attempt.
    pub nonce: String,
    /// Telegram deep link the end-user must open to authenticate.
    pub deep_link: String,
    /// ISO 8601 timestamp after which the login attempt expires.
    pub expires_at: String,
}

/// The current state of a Passepartout login flow.
///
/// Returned by [`Passepartout::login_status`](super::Passepartout::login_status).
/// Inspect `status` — once it reaches a terminal success state the remaining
/// fields are populated with the issued token and the authenticated user.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct LoginStatus {
    /// Flow state, e.g. `"pending"`, `"completed"`, `"expired"`.
    pub status: String,
    /// The issued access token, present once the flow completes.
    #[serde(default)]
    pub access_token: Option<String>,
    /// ISO 8601 expiry timestamp of the access token, when issued.
    #[serde(default)]
    pub expires_at: Option<String>,
    /// The Gate identity ID linked to the Telegram user, when resolved.
    #[serde(default)]
    pub identity_id: Option<String>,
    /// The authenticated Telegram user, present once the flow completes.
    #[serde(default)]
    pub user: Option<TelegramUser>,
}

/// Token validity and claims returned by
/// [`Passepartout::introspect`](super::Passepartout::introspect).
#[derive(Debug, Clone, serde::Deserialize)]
pub struct TokenIntrospection {
    /// `true` if the token is valid, unexpired, and not revoked.
    pub active: bool,
    /// The subject (identity) the token was issued for.
    #[serde(default)]
    pub subject: Option<String>,
    /// Tenant the subject belongs to.
    #[serde(default)]
    pub tenant_id: Option<String>,
    /// Scopes embedded in the token.
    #[serde(default)]
    pub scopes: Option<Vec<String>>,
    /// ISO 8601 expiry timestamp.
    #[serde(default)]
    pub expires_at: Option<String>,
}
