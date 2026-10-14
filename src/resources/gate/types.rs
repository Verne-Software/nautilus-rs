use serde_json::{Map, Value};

// ---------------------------------------------------------------------------
// Identities
// ---------------------------------------------------------------------------

/// An end user (a Kratos identity) managed by Gate.
///
/// The object is the identity service's own. The stable fields are typed; the rest stays
/// reachable through [`extra`](Self::extra) (`credentials`, …).
#[derive(Debug, Clone, serde::Deserialize)]
pub struct Identity {
    /// Unique identity identifier.
    pub id: String,
    /// The schema this identity conforms to (always `"user"` for end users).
    pub schema_id: String,
    /// Current lifecycle state (`"active"` or `"inactive"`).
    pub state: String,
    /// Profile traits attached to this identity.
    pub traits: IdentityTraits,
    /// ISO 8601 creation time.
    #[serde(default)]
    pub created_at: Option<String>,
    /// ISO 8601 time of the last update.
    #[serde(default)]
    pub updated_at: Option<String>,
    /// ISO 8601 time of the last state change.
    #[serde(default)]
    pub state_changed_at: Option<String>,
    /// Your custom metadata. Read and change it with
    /// [`IdentitiesClient::get_metadata`](super::IdentitiesClient::get_metadata) and
    /// [`update_metadata`](super::IdentitiesClient::update_metadata).
    #[serde(default)]
    pub metadata_public: Option<Value>,
    /// The addresses the user can verify, and whether they did.
    #[serde(default)]
    pub verifiable_addresses: Vec<VerifiableAddress>,
    /// The addresses the user can recover the account through.
    #[serde(default)]
    pub recovery_addresses: Vec<RecoveryAddress>,
    /// Every member not typed above, as the identity service sent it.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// An address of an [`Identity`] that can be verified.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct VerifiableAddress {
    /// The address, e.g. the email.
    pub value: String,
    /// Whether the user has verified it.
    #[serde(default)]
    pub verified: bool,
    /// How it is verified (`"email"`).
    #[serde(default)]
    pub via: String,
    /// Verification status (`"pending"`, `"completed"`, …).
    #[serde(default)]
    pub status: Option<String>,
}

/// An address an [`Identity`] can recover the account through.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct RecoveryAddress {
    /// The address, e.g. the email.
    pub value: String,
    /// How it is used (`"email"`).
    #[serde(default)]
    pub via: String,
}

/// Profile traits of an existing [`Identity`].
///
/// The `user` schema has exactly two traits: `email` (required) and `locale`. Application
/// data does not go here — keep it in custom metadata
/// ([`IdentitiesClient::update_metadata`](super::IdentitiesClient::update_metadata)).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct IdentityTraits {
    /// Primary email address.
    pub email: String,
    /// Preferred language, e.g. `"en"`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub locale: Option<String>,
    /// Never returned by the API.
    #[deprecated(
        since = "1.4.0",
        note = "the API never returns a tenant_id trait; this field is always None and will be removed"
    )]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tenant_id: Option<String>,
    /// Any other member the identity service sent.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// Profile traits supplied when creating a new identity.
///
/// The schema allows only these two traits; anything else is refused with
/// `400 invalid_payload`. Use custom metadata for your own fields.
#[derive(Debug, Clone, serde::Serialize)]
pub struct IdentityTraitsInput {
    /// Primary email address.
    pub email: String,
    /// Preferred language, e.g. `"en"`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub locale: Option<String>,
}

/// Parameters for [`IdentitiesClient::create`](super::IdentitiesClient::create).
///
/// A second end user with the same address *in your account* is `409 conflict`; the same
/// address in another account is a different person and is created.
///
/// # Example
///
/// ```no_run
/// use nautilus_rs::{Gate, CreateIdentityParams, IdentityTraitsInput};
///
/// # async fn run() -> Result<(), nautilus_rs::Error> {
/// let gate = Gate::new("vrn_9f8a7c1b2d3e4f50");
/// let identity = gate.identities().create(CreateIdentityParams {
///     schema_id: "user".into(),
///     traits: IdentityTraitsInput {
///         email: "alice@example.com".into(),
///         locale: Some("en".into()),
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
    /// Schema identifier the identity should conform to. Always `"user"`.
    pub schema_id: String,
    /// Profile traits for the new identity.
    pub traits: IdentityTraitsInput,
    /// Optional credential material in the format the schema expects, e.g.
    /// `{"password": {"config": {"password": "…"}}}`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub credentials: Option<Value>,
    /// Initial lifecycle state (`"active"` or `"inactive"`). Defaults to the schema
    /// default when `None`.
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
/// use nautilus_rs::{Gate, JsonPatchOp};
/// use serde_json::json;
///
/// # async fn run() -> Result<(), nautilus_rs::Error> {
/// let gate = Gate::new("vrn_9f8a7c1b2d3e4f50");
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
    pub value: Option<Value>,
    /// Source location (required for `move` and `copy`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from: Option<String>,
}

// ---------------------------------------------------------------------------
// Settings and flows
// ---------------------------------------------------------------------------

/// Security-related Gate Identity settings for a tenant.
///
/// Used both as the return value of
/// [`SettingsClient::get_security`](super::SettingsClient::get_security) and as
/// the input to
/// [`SettingsClient::update_security`](super::SettingsClient::update_security).
/// Updates are a full replacement — both fields are always sent.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SecuritySettings {
    /// Whether sign-in and sign-up by an emailed one-time code is enabled.
    pub passwordless_enabled: bool,
    /// Whether a second factor (TOTP) is required — enforced by
    /// [`SessionsClient::check`](super::SessionsClient::check).
    pub mfa_enabled: bool,
}

/// A social login (OAuth 2.0 / OIDC) provider and whether it is enabled for a
/// tenant's end-users.
///
/// Used both as an element of the list returned by
/// [`SettingsClient::get_oidc_providers`](super::SettingsClient::get_oidc_providers)
/// and as input to
/// [`SettingsClient::update_oidc_providers`](super::SettingsClient::update_oidc_providers).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct OidcProvider {
    /// Provider identifier, e.g. `"github"`, `"google"`, `"gitlab"`.
    pub provider: String,
    /// Whether the provider appears in the tenant's login/registration UI.
    pub enabled: bool,
}

/// Optional parameters of [`Gate::create_login_flow_with`](super::Gate::create_login_flow_with).
#[derive(Debug, Clone, Default)]
pub struct LoginFlowParams {
    /// One of the project's registered return URLs, exactly as registered (see
    /// [`SettingsClient::update_return_urls`](super::SettingsClient::update_return_urls)).
    /// The first one is used when absent.
    pub return_to: Option<String>,
    /// An opaque value of yours, 1–256 characters of letters, digits, `-`, `.`, `_`, `~`,
    /// returned unchanged on the redirect to your return URL. Always send one and check
    /// it when the user comes back from a social login.
    pub state: Option<String>,
    /// `"aal2"` starts a step-up (second factor) flow and needs
    /// [`session_token`](Self::session_token).
    pub aal: Option<String>,
    /// The end user's session token, for a step-up flow. Sent as `X-Session-Token`, never
    /// in the query.
    pub session_token: Option<String>,
}

/// Optional parameters of
/// [`Gate::create_registration_flow`](super::Gate::create_registration_flow).
#[derive(Debug, Clone, Default)]
pub struct RegistrationFlowParams {
    /// As for [`LoginFlowParams::return_to`].
    pub return_to: Option<String>,
    /// As for [`LoginFlowParams::state`].
    pub state: Option<String>,
}

/// What [`Gate::exchange_oidc_code`](super::Gate::exchange_oidc_code) returns: the same
/// shape a password login returns.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct OidcExchangeResult {
    /// The end user's session token. Keep it on your server.
    pub session_token: String,
    /// The session.
    pub session: Session,
    /// Any other member of the answer.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

// ---------------------------------------------------------------------------
// Access tokens and authorization
// ---------------------------------------------------------------------------

/// A short-lived access token issued by Gate.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct AccessToken {
    /// The access token string (`gat_live_…`) to pass to downstream services.
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
/// use nautilus_rs::{Gate, CreateTokenParams};
///
/// # async fn run() -> Result<(), nautilus_rs::Error> {
/// let gate = Gate::new("vrn_9f8a7c1b2d3e4f50");
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
    /// The ID of the user or service account the token represents.
    pub subject: String,
    /// Permission scopes to embed in the token. The server grants all
    /// configured scopes when `None`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scopes: Option<Vec<String>>,
    /// Token lifetime in seconds. Uses the server-side default when `None`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ttl_seconds: Option<u64>,
}

/// One organization a user belongs to: the unit Gate uses everywhere it says who a user
/// is — in introspection, the session check, the signed token's `orgs` claim and
/// [`IdentitiesClient::list_organizations`](super::IdentitiesClient::list_organizations).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct OrgClaim {
    /// The organization's id.
    pub id: String,
    /// The name of the role the user holds there.
    pub role: String,
    /// That role's permissions, e.g. `"invoices:write"`.
    #[serde(default)]
    pub permissions: Vec<String>,
}

/// Token validity and claims returned by
/// [`TokensClient::introspect`](super::TokensClient::introspect).
///
/// An inactive token (unknown, expired, another tenant's, or one whose user was
/// deactivated or deleted) is `{"active": false}` and nothing else: every other field is
/// `None`.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct TokenInfo {
    /// `true` if the token is valid, unexpired, and not revoked.
    pub active: bool,
    /// The identity the token was issued for.
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
    /// When `subject` is an end user's id: their organizations in the key's project, read
    /// live, the first 100 in the order they joined. `None` for any other subject and for
    /// an inactive token.
    #[serde(default)]
    pub orgs: Option<Vec<OrgClaim>>,
    /// `Some(true)`, and only then, when there are more organizations than
    /// [`orgs`](Self::orgs) shows; list them with
    /// [`IdentitiesClient::list_organizations`](super::IdentitiesClient::list_organizations).
    #[serde(default)]
    pub orgs_truncated: Option<bool>,
}

/// Parameters for [`Gate::authorize`](crate::Gate::authorize).
///
/// # Example
///
/// ```no_run
/// use nautilus_rs::{Gate, AuthorizeParams};
///
/// # async fn run() -> Result<(), nautilus_rs::Error> {
/// let gate = Gate::new("vrn_9f8a7c1b2d3e4f50");
/// let decision = gate.authorize(AuthorizeParams {
///     subject: "f47ac10b-58cc-4372-a567-0e02b2c3d479".into(),
///     action: "invoices:write".into(),
///     resource: "organization:550e8400-e29b-41d4-a716-446655440000".into(),
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
    /// The end user's identity id.
    pub subject: String,
    /// For an organization resource: the permission to check, e.g. `"invoices:write"`.
    /// Otherwise a label for your own records.
    pub action: String,
    /// `"organization:<uuid>"` to check a permission in an organization (prefer
    /// [`OrganizationsClient::authorize`](super::OrganizationsClient::authorize) /
    /// [`can`](super::OrganizationsClient::can), which build it); anything else asks only
    /// whether `subject` is one of your active end users.
    pub resource: String,
    /// Accepted and ignored by the API today.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<Value>,
}

/// Why an authorization check answered as it did.
///
/// For an organization resource it is one of the five named variants. For any other
/// resource the API answers with a free sentence ("subject `…` is an identity owned by
/// tenant `…`"), which lands in [`Other`](Self::Other) — as does any reason added later.
/// Do not branch on `Other`; it is for people.
///
/// It deserializes from, and [`Display`](std::fmt::Display)s back to, the wire string.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum AuthorizeReason {
    /// The user is a member and their role holds the permission.
    MemberWithPermission,
    /// The user is a member but their role does not hold the permission.
    MemberWithoutPermission,
    /// The user is not a member of the organization.
    NotAMember,
    /// The user is deactivated. [`AuthorizationDecision::role`] says what they held.
    SubjectInactive,
    /// There is no such organization in the key's project (another project's organization
    /// looks the same).
    NoSuchOrganization,
    /// Any other reason: the sentence of a non-organization check, or a reason this SDK
    /// does not know yet.
    Other(String),
}

impl AuthorizeReason {
    /// The wire string.
    pub fn as_str(&self) -> &str {
        match self {
            Self::MemberWithPermission => "member_with_permission",
            Self::MemberWithoutPermission => "member_without_permission",
            Self::NotAMember => "not_a_member",
            Self::SubjectInactive => "subject_inactive",
            Self::NoSuchOrganization => "no_such_organization",
            Self::Other(sentence) => sentence,
        }
    }
}

impl From<&str> for AuthorizeReason {
    fn from(value: &str) -> Self {
        match value {
            "member_with_permission" => Self::MemberWithPermission,
            "member_without_permission" => Self::MemberWithoutPermission,
            "not_a_member" => Self::NotAMember,
            "subject_inactive" => Self::SubjectInactive,
            "no_such_organization" => Self::NoSuchOrganization,
            other => Self::Other(other.to_owned()),
        }
    }
}

impl std::fmt::Display for AuthorizeReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl<'de> serde::Deserialize<'de> for AuthorizeReason {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let wire = String::deserialize(deserializer)?;
        Ok(Self::from(wire.as_str()))
    }
}

impl serde::Serialize for AuthorizeReason {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

/// The result of an authorization check.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct AuthorizationDecision {
    /// `true` if the subject is permitted to perform the action on the
    /// resource.
    pub allowed: bool,
    /// Identifies the decision in Gate's logs, for support.
    pub decision_id: String,
    /// Why — see [`AuthorizeReason`].
    pub reason: AuthorizeReason,
    /// The role the user holds in the organization. Present for
    /// [`MemberWithPermission`](AuthorizeReason::MemberWithPermission),
    /// [`MemberWithoutPermission`](AuthorizeReason::MemberWithoutPermission) and
    /// [`SubjectInactive`](AuthorizeReason::SubjectInactive); absent otherwise.
    #[serde(default)]
    pub role: Option<String>,
}

// ---------------------------------------------------------------------------
// Signed tokens (JWT)
// ---------------------------------------------------------------------------

/// What [`TokensClient::create_signed`](super::TokensClient::create_signed) returns.
#[derive(Debug, Clone)]
pub struct SignedToken {
    /// The compact ES256 JWT. Valid for 15 minutes; it cannot be revoked.
    pub token: String,
    /// The session the token was made from, as `whoami` returns it.
    pub session: Session,
}

/// The verified payload of a Gate signed token.
///
/// Returned by `GateTokenVerifier::verify` (feature `jwt`). The token lives 15 minutes and
/// **cannot be revoked**, and [`orgs`](Self::orgs) is as old as the token: for anything that
/// must follow a revocation or a role change, ask
/// [`SessionsClient::check`](super::SessionsClient::check), introspection or
/// [`OrganizationsClient::can`](super::OrganizationsClient::can).
#[derive(Debug, Clone, serde::Deserialize)]
pub struct SignedTokenClaims {
    /// `<base URL>/auth/<tenant id>` — checked by the verifier.
    pub iss: String,
    /// The end user's identity id.
    pub sub: String,
    /// The session the token was made from (revocable with
    /// [`SessionsClient::revoke`](super::SessionsClient::revoke)).
    #[serde(default)]
    pub sid: String,
    /// The tenant the user belongs to.
    #[serde(default)]
    pub tenant_id: String,
    /// Always `"user"`.
    #[serde(default)]
    pub schema_id: String,
    /// The user's email, when they have one.
    #[serde(default)]
    pub email: Option<String>,
    /// Always `"authenticated"` (the database role PostgREST-style consumers use).
    #[serde(default)]
    pub role: String,
    /// The user's organizations as of when the token was made. Always present for an end
    /// user, possibly empty.
    #[serde(default)]
    pub orgs: Vec<OrgClaim>,
    /// `Some(true)`, and only then, when `orgs` is incomplete; list them with
    /// [`IdentitiesClient::list_organizations`](super::IdentitiesClient::list_organizations).
    #[serde(default)]
    pub orgs_truncated: Option<bool>,
    /// Issued at, Unix seconds.
    #[serde(default)]
    pub iat: i64,
    /// Not valid before, Unix seconds.
    #[serde(default)]
    pub nbf: i64,
    /// Expires at, Unix seconds.
    pub exp: i64,
    /// Token id.
    #[serde(default)]
    pub jti: String,
    /// Any other claim.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

// ---------------------------------------------------------------------------
// Sessions
// ---------------------------------------------------------------------------

/// An end user's session, as the identity service returns it.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct Session {
    /// Session id.
    pub id: String,
    /// Whether the session is alive.
    #[serde(default)]
    pub active: Option<bool>,
    /// When the user authenticated.
    #[serde(default)]
    pub authenticated_at: Option<String>,
    /// When the session ends.
    #[serde(default)]
    pub expires_at: Option<String>,
    /// When the session was issued.
    #[serde(default)]
    pub issued_at: Option<String>,
    /// `"aal1"` or `"aal2"`.
    #[serde(default)]
    pub authenticator_assurance_level: Option<String>,
    /// How the user authenticated.
    #[serde(default)]
    pub authentication_methods: Vec<AuthenticationMethod>,
    /// The devices the session was used from.
    #[serde(default)]
    pub devices: Vec<SessionDevice>,
    /// The identity, when the answer embeds it.
    #[serde(default)]
    pub identity: Option<Value>,
    /// Every member not typed above.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// One authentication method a [`Session`] was established with.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct AuthenticationMethod {
    /// `"password"`, `"totp"`, `"oidc"`, …
    pub method: String,
    /// When it was completed.
    #[serde(default)]
    pub completed_at: Option<String>,
    /// Every member not typed above.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// A device a [`Session`] was used from.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct SessionDevice {
    /// Device id.
    #[serde(default)]
    pub id: Option<String>,
    /// IP address.
    #[serde(default)]
    pub ip_address: Option<String>,
    /// User agent.
    #[serde(default)]
    pub user_agent: Option<String>,
    /// Coarse location.
    #[serde(default)]
    pub location: Option<String>,
}

/// The answer of [`SessionsClient::check`](super::SessionsClient::check) — whether a
/// session is good enough to act on.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct SessionCheck {
    /// Whether the session is alive.
    pub active: bool,
    /// `"aal1"` or `"aal2"`.
    pub aal: String,
    /// Whether your tenant requires a second factor.
    pub mfa_required: bool,
    /// Whether this session meets what your tenant requires — the one field to gate on.
    pub mfa_satisfied: bool,
    /// The end user's id.
    #[serde(default)]
    pub identity_id: Option<String>,
    /// The user's organizations in the key's project, read now (first 100).
    #[serde(default)]
    pub orgs: Option<Vec<OrgClaim>>,
    /// `Some(true)`, and only then, when there are more than `orgs` shows.
    #[serde(default)]
    pub orgs_truncated: Option<bool>,
}

// ---------------------------------------------------------------------------
// Organizations, roles, members
// ---------------------------------------------------------------------------

/// A group of your end users (a customer's team, workspace, company).
#[derive(Debug, Clone, serde::Deserialize)]
pub struct Organization {
    /// Organization id (a UUID).
    pub id: String,
    /// Free text, 1–120 characters, not unique.
    pub name: String,
    /// ISO 8601 creation time.
    pub created_at: String,
    /// ISO 8601 time of the last change.
    pub updated_at: String,
}

/// Parameters for [`OrganizationsClient::create`](super::OrganizationsClient::create).
#[derive(Debug, Clone, serde::Serialize)]
pub struct CreateOrganizationParams {
    /// 1–120 characters, not unique.
    pub name: String,
}

/// Parameters for [`OrganizationsClient::update`](super::OrganizationsClient::update).
#[derive(Debug, Clone, serde::Serialize)]
pub struct UpdateOrganizationParams {
    /// The new name, 1–120 characters.
    pub name: String,
}

/// A named set of permissions inside one organization.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct Role {
    /// Role id (a UUID).
    pub id: String,
    /// The organization it belongs to.
    pub organization_id: String,
    /// Lowercase letters, digits, `_` and `-`; unique in the organization.
    pub name: String,
    /// Sorted and de-duplicated by the API.
    #[serde(default)]
    pub permissions: Vec<String>,
    /// ISO 8601 creation time.
    pub created_at: String,
    /// ISO 8601 time of the last change.
    pub updated_at: String,
}

/// Parameters for [`RolesClient::create`](super::RolesClient::create).
///
/// A role name is `^[a-z0-9][a-z0-9_-]{0,63}$`, unique in its organization (`409
/// conflict`), and an organization holds at most 100 roles (`409 limit_reached`).
/// Permissions are lowercase segments joined by `:` (`invoices:write`), at most 64
/// characters each and 100 per role; `*` and the `nautilus:` prefix are reserved.
#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct CreateRoleParams {
    /// The role's name.
    pub name: String,
    /// The permissions the role holds.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub permissions: Option<Vec<String>>,
}

/// Parameters for [`RolesClient::update`](super::RolesClient::update). Only the fields you
/// set are sent.
#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct UpdateRoleParams {
    /// A new name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// **Replaces** the whole set. Everyone holding the role has the new set at once.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub permissions: Option<Vec<String>>,
}

/// An end user's membership of an organization.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct Member {
    /// The organization.
    pub organization_id: String,
    /// The end user.
    pub identity_id: String,
    /// The role they hold.
    pub role_id: String,
    /// When the membership ends, or `None` for never.
    #[serde(default)]
    pub expires_at: Option<String>,
    /// ISO 8601 creation time.
    pub created_at: String,
    /// ISO 8601 time of the last change.
    pub updated_at: String,
}

/// Parameters for [`MembersClient::add`](super::MembersClient::add).
///
/// Adding an existing member is `409 conflict`; an identity that is not one of your end
/// users is `404`.
#[derive(Debug, Clone, serde::Serialize)]
pub struct AddMemberParams {
    /// One of your end users.
    pub identity_id: String,
    /// A role of this organization.
    pub role_id: String,
    /// A future time (RFC 3339, at most 10 years ahead) when the membership ends.
    /// `None` for never.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<String>,
}

/// Parameters for [`MembersClient::update`](super::MembersClient::update).
///
/// `expires_at` tells three things apart:
///
/// | value | sends | meaning |
/// |---|---|---|
/// | `None` | nothing | keep the deadline |
/// | `Some(None)` | `null` | remove the deadline |
/// | `Some(Some("2027-01-01T00:00:00Z".into()))` | the time | set it |
///
/// Sending neither `role_id` nor `expires_at` is a `400` from the server.
#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct UpdateMemberParams {
    /// Move the member to another role of this organization.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role_id: Option<String>,
    /// The membership deadline, as in the table above.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<Option<String>>,
}

// ---------------------------------------------------------------------------
// Audit log
// ---------------------------------------------------------------------------

/// Filters of [`AuditLogClient::list`](super::AuditLogClient::list). Unset ones are left
/// out of the query. Keep the same filters while paging with `cursor`.
#[derive(Debug, Clone, Default)]
pub struct ListAuditLogParams {
    /// Page size, 1–100 (default 20).
    pub limit: Option<u32>,
    /// The previous page's `next_cursor`.
    pub cursor: Option<String>,
    /// One action, e.g. `"key.created"`, `"identity.login"`,
    /// `"organization.member_added"`. An unknown action is `400 invalid_payload`.
    pub action: Option<String>,
    /// A UUID.
    pub actor_id: Option<String>,
    /// `"team_member"`, `"api_key"`, `"end_user"`, `"system"` or `"staff"`.
    pub actor_type: Option<String>,
    /// The id of what was acted on.
    pub target_id: Option<String>,
    /// Entries at or after this instant (RFC 3339).
    pub from: Option<String>,
    /// Entries before this instant (RFC 3339). Must be later than `from`.
    pub to: Option<String>,
}

/// One entry of the audit log.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct AuditLogEntry {
    /// Entry id.
    pub id: String,
    /// ISO 8601 time of the event.
    pub occurred_at: String,
    /// What happened. An open set: new actions are added over time.
    pub action: String,
    /// Who did it.
    pub actor: AuditActor,
    /// What it was done to, when there is a target.
    #[serde(default)]
    pub target: Option<AuditTarget>,
    /// The project it concerns, when it concerns one.
    #[serde(default)]
    pub project_id: Option<String>,
    /// Free-form facts about the event; never personal data.
    #[serde(default)]
    pub details: Map<String, Value>,
    /// The caller's IP address.
    #[serde(default)]
    pub ip: Option<String>,
    /// The caller's user agent.
    #[serde(default)]
    pub user_agent: Option<String>,
}

/// Who did something, in an [`AuditLogEntry`].
#[derive(Debug, Clone, serde::Deserialize)]
pub struct AuditActor {
    /// `"team_member"`, `"api_key"`, `"end_user"`, `"system"`, `"staff"` or `"unknown"`.
    /// Open set.
    #[serde(rename = "type")]
    pub kind: String,
    /// The actor's id, when there is one.
    #[serde(default)]
    pub id: Option<String>,
}

/// What something was done to, in an [`AuditLogEntry`].
#[derive(Debug, Clone, serde::Deserialize)]
pub struct AuditTarget {
    /// The kind of thing, e.g. `"identity"`.
    #[serde(rename = "type")]
    pub kind: String,
    /// Its id.
    pub id: String,
}

// ---------------------------------------------------------------------------
// Automatic deletion (lifecycle rules)
// ---------------------------------------------------------------------------

/// The four automatic-deletion rules.
///
/// Settings per kind (the `params` of
/// [`LifecycleClient::set_rule`](super::LifecycleClient::set_rule) and
/// [`preview_rule`](super::LifecycleClient::preview_rule)):
///
/// | kind | params |
/// |---|---|
/// | `DeleteUnverified` | `{"delete_after_days": 2..=365, "warn_before_days": 1..delete_after_days-1}` |
/// | `RemindUnverified` | `{"remind_after_hours": 1..=720}` |
/// | `ExpireRoles` | none (`{}` for a preview) |
/// | `DeleteInactive` | `{"inactive_after_months": 1..=120, "warn_before_days": 7..=90, "include_without_address": bool?}` |
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LifecycleRuleKind {
    /// Delete accounts that never verified their address.
    DeleteUnverified,
    /// Remind people who never verified their address.
    RemindUnverified,
    /// Remove expired organization roles.
    ExpireRoles,
    /// Delete accounts that have not signed in for a long time.
    DeleteInactive,
}

impl LifecycleRuleKind {
    /// The kind as it appears in paths and bodies.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::DeleteUnverified => "delete_unverified",
            Self::RemindUnverified => "remind_unverified",
            Self::ExpireRoles => "expire_roles",
            Self::DeleteInactive => "delete_inactive",
        }
    }
}

impl std::fmt::Display for LifecycleRuleKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// One rule, set up or not.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct LifecycleRule {
    /// Which rule.
    pub kind: LifecycleRuleKind,
    /// Whether it was ever set up.
    pub configured: bool,
    /// Whether it is on.
    pub enabled: bool,
    /// Its settings, `None` when not configured.
    #[serde(default)]
    pub params: Option<Value>,
    /// Whether your plan includes the rule.
    pub available: bool,
    /// `Some("plan")` when a plan change switched the rule off.
    #[serde(default)]
    pub disabled_reason: Option<String>,
    /// When it was switched on.
    #[serde(default)]
    pub enabled_at: Option<String>,
    /// When its settings last changed.
    #[serde(default)]
    pub updated_at: Option<String>,
    /// How many people have something scheduled by this rule.
    #[serde(default)]
    pub pending: u64,
}

/// All four rules of the key's project, and the plan.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct LifecycleRules {
    /// Your plan: `free`, `pro`, `advanced` or `contract`.
    pub tier: String,
    /// All four kinds, set up or not.
    pub rules: Vec<LifecycleRule>,
}

/// Parameters for [`LifecycleClient::set_rule`](super::LifecycleClient::set_rule).
///
/// Exactly `{enabled, params?}` is sent — the server refuses unknown fields.
#[derive(Debug, Clone, serde::Serialize)]
pub struct SetRuleParams {
    /// Switch the rule on or off.
    pub enabled: bool,
    /// The rule's settings (see [`LifecycleRuleKind`]). Leave out to keep the saved
    /// ones; required on a first write, except for `expire_roles`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<Value>,
}

/// What a rule would do right now, from
/// [`LifecycleClient::preview_rule`](super::LifecycleClient::preview_rule).
#[derive(Debug, Clone, serde::Deserialize)]
pub struct RulePreview {
    /// Which rule.
    pub kind: LifecycleRuleKind,
    /// Whether your plan includes it.
    pub available: bool,
    /// How many people the rule would schedule right now.
    pub matching_now: u64,
    /// `true` when the count stopped at 1,000.
    pub capped: bool,
}

/// Filters of [`LifecycleClient::list_actions`](super::LifecycleClient::list_actions).
#[derive(Debug, Clone, Default)]
pub struct ListLifecycleActionsParams {
    /// Page size, 1–100 (default 20).
    pub limit: Option<u32>,
    /// The previous page's `next_cursor`.
    pub cursor: Option<String>,
    /// `"pending"`, `"done"`, `"cancelled"` or `"failed"`.
    pub status: Option<String>,
    /// Only this rule's actions.
    pub kind: Option<LifecycleRuleKind>,
    /// Only this end user's actions.
    pub identity_id: Option<String>,
}

/// One thing a rule scheduled or did.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct LifecycleAction {
    /// Action id.
    pub id: String,
    /// The rule that made it.
    pub kind: LifecycleRuleKind,
    /// `"remind"`, `"warn"`, `"delete"` or `"revoke"`.
    pub step: String,
    /// The end user concerned.
    pub identity_id: String,
    /// `"pending"`, `"done"`, `"cancelled"` or `"failed"`.
    pub status: String,
    /// The moment the rule's clock started from.
    pub basis_at: String,
    /// When it is due.
    pub due_at: String,
    /// Why it was cancelled or failed (`rule_disabled`, `verified`, `signed_in`, …). An
    /// open string.
    #[serde(default)]
    pub reason: Option<String>,
    /// ISO 8601 creation time.
    pub created_at: String,
    /// When it finished.
    #[serde(default)]
    pub finished_at: Option<String>,
}
