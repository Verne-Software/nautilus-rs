/// A user identity managed by Gate.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct Identity {
    /// Unique identity identifier.
    pub id: String,
    /// The schema this identity conforms to (e.g. `"default"`).
    pub schema_id: String,
    /// Current lifecycle state (`"active"`, `"inactive"`, …).
    pub state: String,
    /// Profile traits attached to this identity.
    pub traits: IdentityTraits,
}

/// Profile traits for an existing [`Identity`].
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct IdentityTraits {
    /// Primary email address.
    pub email: String,
    /// Arbitrary application-defined JSON data.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub custom_data: Option<serde_json::Value>,
    /// Tenant the identity belongs to.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tenant_id: Option<String>,
}

/// Profile traits supplied when creating a new identity.
///
/// Identical to [`IdentityTraits`] but omits the server-assigned `tenant_id`.
#[derive(Debug, Clone, serde::Serialize)]
pub struct IdentityTraitsInput {
    /// Primary email address.
    pub email: String,
    /// Arbitrary application-defined JSON data.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub custom_data: Option<serde_json::Value>,
}

/// Parameters for [`IdentitiesClient::create`](super::IdentitiesClient::create).
///
/// # Example
///
/// ```no_run
/// use nautilus::{Gate, CreateIdentityParams, IdentityTraitsInput};
///
/// # async fn run() -> Result<(), nautilus::Error> {
/// let gate = Gate::new("vrn_gate_live_sk_…");
/// let identity = gate.identities().create(CreateIdentityParams {
///     schema_id: "default".into(),
///     traits: IdentityTraitsInput {
///         email: "alice@example.com".into(),
///         custom_data: None,
///     },
///     credentials: None,
///     state: Some("active".into()),
/// }).await?;
/// println!("created: {}", identity.id);
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone, serde::Serialize)]
pub struct CreateIdentityParams {
    /// Schema identifier the identity should conform to.
    pub schema_id: String,
    /// Profile traits for the new identity.
    pub traits: IdentityTraitsInput,
    /// Optional credential material (passwords, OIDC tokens, …) in the format
    /// expected by the schema.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub credentials: Option<serde_json::Value>,
    /// Initial lifecycle state. Defaults to the schema default when `None`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state: Option<String>,
}

/// A single [RFC 6902](https://datatracker.ietf.org/doc/html/rfc6902) JSON
/// Patch operation used with
/// [`IdentitiesClient::patch`](super::IdentitiesClient::patch).
///
/// # Example — update an email address
///
/// ```no_run
/// use nautilus::{Gate, JsonPatchOp};
/// use serde_json::json;
///
/// # async fn run() -> Result<(), nautilus::Error> {
/// let gate = Gate::new("vrn_gate_live_sk_…");
/// gate.identities().patch("idn_…", vec![JsonPatchOp {
///     op: "replace".into(),
///     path: "/traits/email".into(),
///     value: Some(json!("bob@example.com")),
///     from: None,
/// }]).await?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone, serde::Serialize)]
pub struct JsonPatchOp {
    /// Operation type: `"add"`, `"remove"`, `"replace"`, `"move"`, `"copy"`,
    /// or `"test"`.
    pub op: String,
    /// JSON Pointer (RFC 6901) to the target location.
    pub path: String,
    /// Value to apply (required for `add`, `replace`, `test`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<serde_json::Value>,
    /// Source location (required for `move` and `copy`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from: Option<String>,
}

/// A short-lived access token issued by Gate.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct AccessToken {
    /// The signed access token string to pass to downstream services.
    pub access_token: String,
    /// ISO 8601 expiry timestamp.
    pub expires_at: String,
    /// The identity this token was issued for.
    pub subject: String,
    /// Tenant the subject belongs to.
    pub tenant_id: String,
}

/// Parameters for [`TokensClient::create`](super::TokensClient::create).
///
/// # Example
///
/// ```no_run
/// use nautilus::{Gate, CreateTokenParams};
///
/// # async fn run() -> Result<(), nautilus::Error> {
/// let gate = Gate::new("vrn_gate_live_sk_…");
/// let token = gate.tokens().create(CreateTokenParams {
///     subject: "idn_alice".into(),
///     scopes: Some(vec!["read:orders".into(), "write:orders".into()]),
///     ttl_seconds: Some(3600),
/// }).await?;
/// println!("token: {}", token.access_token);
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone, serde::Serialize)]
pub struct CreateTokenParams {
    /// The identity ID the token is issued for.
    pub subject: String,
    /// Permission scopes to embed in the token. The server grants all
    /// configured scopes when `None`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scopes: Option<Vec<String>>,
    /// Token lifetime in seconds. Uses the server-side default when `None`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ttl_seconds: Option<u64>,
}

/// Token validity and claims returned by
/// [`TokensClient::introspect`](super::TokensClient::introspect).
#[derive(Debug, Clone, serde::Deserialize)]
pub struct TokenInfo {
    /// `true` if the token is valid, unexpired, and not revoked.
    pub active: bool,
    /// The identity the token was issued for.
    pub subject: String,
    /// Tenant the subject belongs to.
    pub tenant_id: String,
    /// Scopes embedded in the token.
    pub scopes: Vec<String>,
    /// ISO 8601 expiry timestamp.
    pub expires_at: String,
}

/// Parameters for [`Gate::authorize`](crate::Gate::authorize).
///
/// # Example
///
/// ```no_run
/// use nautilus::{Gate, AuthorizeParams};
///
/// # async fn run() -> Result<(), nautilus::Error> {
/// let gate = Gate::new("vrn_gate_live_sk_…");
/// let decision = gate.authorize(AuthorizeParams {
///     subject: "idn_alice".into(),
///     action: "delete".into(),
///     resource: "order:ord_123".into(),
///     context: None,
/// }).await?;
///
/// if decision.allowed {
///     println!("access granted (decision {})", decision.decision_id);
/// } else {
///     println!("denied: {}", decision.reason);
/// }
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone, serde::Serialize)]
pub struct AuthorizeParams {
    /// The identity requesting access.
    pub subject: String,
    /// The action being performed (e.g. `"read"`, `"delete"`).
    pub action: String,
    /// The resource being accessed (e.g. `"order:ord_123"`).
    pub resource: String,
    /// Optional arbitrary JSON context passed to policy evaluation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<serde_json::Value>,
}

/// The result of an authorization check.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct AuthorizationDecision {
    /// `true` if the subject is permitted to perform the action on the
    /// resource.
    pub allowed: bool,
    /// Unique identifier for this specific decision, useful for audit logs.
    pub decision_id: String,
    /// Human-readable explanation of why access was granted or denied.
    pub reason: String,
}
