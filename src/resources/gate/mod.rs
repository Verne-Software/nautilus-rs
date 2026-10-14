mod audit_log;
mod lifecycle;
mod organizations;
mod sessions;
pub mod types;
#[cfg(feature = "jwt")]
mod verifier;

use std::sync::Arc;

use reqwest::Method;
use serde_json::{Map, Value};

use crate::{
    error::Error,
    http::{urlencode as seg, Call, HttpClient},
    types::{ListParams, Paginated},
};
use types::{
    AccessToken, AuthorizationDecision, AuthorizeParams, CreateIdentityParams, CreateTokenParams,
    Identity, JsonPatchOp, LoginFlowParams, OidcExchangeResult, OidcProvider, OrgClaim,
    RegistrationFlowParams, SecuritySettings, Session, SignedToken, TokenInfo,
};

pub use audit_log::AuditLogClient;
pub use lifecycle::LifecycleClient;
pub use organizations::{MembersClient, OrganizationsClient, RolesClient};
pub use sessions::SessionsClient;
#[cfg(feature = "jwt")]
use verifier::VerifierCache;
#[cfg(feature = "jwt")]
pub use verifier::{GateTokenVerifier, GateTokenVerifierBuilder};

/// Gate service client — Auth-as-a-Service.
///
/// Provides identity management, organizations with roles and members, short-lived access
/// tokens and signed tokens (JWT), sessions and the session check, an audit log, automatic
/// deletion rules, and authorization checks.
///
/// Obtain a `Gate` instance either as part of the unified [`Verne`] client or
/// standalone:
///
/// ```no_run
/// // Standalone
/// use nautilus_rs::Gate;
/// let gate = Gate::new("vrn_9f8a7c1b2d3e4f50");
///
/// // Via unified client
/// use nautilus_rs::Verne;
/// # fn run() -> Result<(), nautilus_rs::Error> {
/// let verne = Verne::builder().api_key("vrn_9f8a7c1b2d3e4f50").build()?;
/// let gate = verne.gate()?;
/// # Ok(())
/// # }
/// ```
///
/// The key needs the `gate` scope; without it every call is `403 forbidden`.
///
/// [`Verne`]: crate::Verne
pub struct Gate {
    http: Arc<HttpClient>,
    #[cfg(feature = "jwt")]
    verifiers: VerifierCache,
}

impl std::fmt::Debug for Gate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Gate").finish_non_exhaustive()
    }
}

impl Gate {
    /// Create a `Gate` client with default settings.
    ///
    /// Panics if the HTTP client cannot be initialized. Use [`Gate::builder`] for
    /// fallible construction.
    pub fn new(api_key: impl Into<String>) -> Self {
        Self::builder()
            .api_key(api_key)
            .build()
            .expect("failed to build Gate client")
    }

    /// Return a [`GateBuilder`] for fine-grained configuration.
    pub fn builder() -> GateBuilder {
        GateBuilder::default()
    }

    pub(crate) fn from_http(http: Arc<HttpClient>) -> Self {
        Self {
            http,
            #[cfg(feature = "jwt")]
            verifiers: Default::default(),
        }
    }

    /// Return an [`IdentitiesClient`] for CRUD operations on identities.
    pub fn identities(&self) -> IdentitiesClient {
        IdentitiesClient {
            http: Arc::clone(&self.http),
        }
    }

    /// Return a [`TokensClient`] for issuing and introspecting access tokens and
    /// minting and verifying signed tokens.
    pub fn tokens(&self) -> TokensClient {
        TokensClient {
            http: Arc::clone(&self.http),
            #[cfg(feature = "jwt")]
            verifiers: Arc::clone(&self.verifiers),
        }
    }

    /// Return a [`SettingsClient`] for reading and updating tenant settings.
    pub fn settings(&self) -> SettingsClient {
        SettingsClient {
            http: Arc::clone(&self.http),
        }
    }

    /// Return an [`OrganizationsClient`] for organizations, their roles and members, and
    /// permission checks.
    pub fn organizations(&self) -> OrganizationsClient {
        OrganizationsClient {
            http: Arc::clone(&self.http),
        }
    }

    /// Return a [`SessionsClient`] for end users' sessions and the session check.
    pub fn sessions(&self) -> SessionsClient {
        SessionsClient {
            http: Arc::clone(&self.http),
        }
    }

    /// Return an [`AuditLogClient`] for reading the audit log.
    pub fn audit_log(&self) -> AuditLogClient {
        AuditLogClient {
            http: Arc::clone(&self.http),
        }
    }

    /// Return a [`LifecycleClient`] for the automatic-deletion rules.
    pub fn lifecycle(&self) -> LifecycleClient {
        LifecycleClient {
            http: Arc::clone(&self.http),
        }
    }

    /// Ask whether a subject may perform an action on a resource. The answer is a
    /// decision; your service enforces it.
    ///
    /// - `resource: "organization:<id>"` — may this end user do `action` (a permission) in
    ///   that organization? [`reason`](AuthorizationDecision::reason) is one of a fixed set
    ///   and [`role`](AuthorizationDecision::role) is the role they hold. Prefer
    ///   [`organizations().authorize()`](OrganizationsClient::authorize) /
    ///   [`can()`](OrganizationsClient::can), which build the resource.
    /// - any other resource — only *is `subject` one of your active end users?*; `reason`
    ///   is [`OwnedByTenant`](crate::AuthorizeReason::OwnedByTenant) (allowed),
    ///   [`SubjectInactive`](crate::AuthorizeReason::SubjectInactive),
    ///   [`NotOwnedByTenant`](crate::AuthorizeReason::NotOwnedByTenant) or
    ///   [`NotAKnownIdentity`](crate::AuthorizeReason::NotAKnownIdentity), and
    ///   [`message`](AuthorizationDecision::message) says it in a sentence for your logs.
    ///
    /// Maps to `POST /v1/gate/authorize`.
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
    /// assert!(decision.allowed);
    /// # Ok(())
    /// # }
    /// ```
    pub async fn authorize(&self, params: AuthorizeParams) -> Result<AuthorizationDecision, Error> {
        self.http.post("/v1/gate/authorize", &params).await
    }

    /// List the social login providers currently enabled for a tenant.
    ///
    /// This is a **public, unauthenticated** endpoint — call it from your
    /// login / registration page to decide which social buttons to render.
    /// Maps to `GET /public/gate/providers/{tenant_id}`.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use nautilus_rs::Gate;
    ///
    /// # async fn run() -> Result<(), nautilus_rs::Error> {
    /// let gate = Gate::new("vrn_9f8a7c1b2d3e4f50");
    /// let providers = gate.get_enabled_providers("550e8400-e29b-41d4-a716-446655440000").await?;
    /// // → ["github", "google"]
    /// # Ok(())
    /// # }
    /// ```
    pub async fn get_enabled_providers(&self, tenant_id: &str) -> Result<Vec<String>, Error> {
        #[derive(serde::Deserialize)]
        struct Wrapper {
            providers: Vec<String>,
        }

        let wrapped: Wrapper = self
            .http
            .get(&format!("/public/gate/providers/{}", seg(tenant_id)))
            .await?;
        Ok(wrapped.providers)
    }

    /// Initialize a Kratos login flow using your Gate API key.
    ///
    /// Call this from your server and pass the returned flow JSON to your
    /// browser-side code to render social login buttons. The flow already
    /// contains only the providers your tenant has enabled. Maps to
    /// `GET /v1/gate/auth/login`.
    ///
    /// The response mirrors the raw Ory Kratos flow JSON, so it is returned as
    /// an untyped [`serde_json::Value`]. For a return URL, a `state`, or a step-up flow,
    /// use [`create_login_flow_with`](Self::create_login_flow_with).
    pub async fn create_login_flow(&self) -> Result<Value, Error> {
        self.create_login_flow_with(LoginFlowParams::default())
            .await
    }

    /// Initialize a login flow with a return URL, a `state`, or as a step-up (second
    /// factor) flow.
    ///
    /// With a return URL registered ([`SettingsClient::update_return_urls`]), pass
    /// `return_to` and always a `state` for social login. A step-up is
    /// `aal: Some("aal2")` with the user's `session_token`, which travels in the
    /// `X-Session-Token` header.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use nautilus_rs::{Gate, LoginFlowParams};
    ///
    /// # async fn run() -> Result<(), nautilus_rs::Error> {
    /// let gate = Gate::new("vrn_9f8a7c1b2d3e4f50");
    /// let flow = gate.create_login_flow_with(LoginFlowParams {
    ///     return_to: Some("https://app.example.com/auth/callback".into()),
    ///     state: Some("c0ffee-1234".into()),
    ///     ..Default::default()
    /// }).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn create_login_flow_with(&self, params: LoginFlowParams) -> Result<Value, Error> {
        let mut call = Call::new(Method::GET, "/v1/gate/auth/login")
            .query("return_to", params.return_to)
            .query("state", params.state)
            .query("aal", params.aal);
        if let Some(token) = params.session_token {
            call = call.session_token(token);
        }
        self.http.send(call).await
    }

    /// Initialize a Kratos registration flow. Maps to `GET /v1/gate/auth/registration`.
    ///
    /// Refused with `402 ACTIVE_USER_LIMIT_REACHED` when the account is past its
    /// active-user limit and the month to move up has ended.
    pub async fn create_registration_flow(
        &self,
        params: RegistrationFlowParams,
    ) -> Result<Value, Error> {
        let call = Call::new(Method::GET, "/v1/gate/auth/registration")
            .query("return_to", params.return_to)
            .query("state", params.state);
        self.http.send(call).await
    }

    /// Exchange the one-time `code` your return URL received after a social login for the
    /// user's session.
    ///
    /// The code works once, for about 60 seconds, and only with a key of the project that
    /// started the login (otherwise `404`). Keep the session token server-side. Maps to
    /// `POST /v1/gate/auth/oidc/exchange`.
    pub async fn exchange_oidc_code(&self, code: &str) -> Result<OidcExchangeResult, Error> {
        self.http
            .post(
                "/v1/gate/auth/oidc/exchange",
                &serde_json::json!({ "code": code }),
            )
            .await
    }
}

/// Builder for a standalone [`Gate`] client.
///
/// # Example
///
/// ```no_run
/// use nautilus_rs::Gate;
///
/// let gate = Gate::builder()
///     .api_key("vrn_9f8a7c1b2d3e4f50")
///     .timeout_secs(15)
///     .build()
///     .expect("invalid configuration");
/// ```
#[derive(Default)]
pub struct GateBuilder {
    api_key: Option<String>,
    base_url: Option<String>,
    timeout_secs: Option<u64>,
}

impl GateBuilder {
    /// Set the API key (**required**). It needs the `gate` scope.
    pub fn api_key(mut self, key: impl Into<String>) -> Self {
        self.api_key = Some(key.into());
        self
    }

    /// Override the API base URL (default: `https://api.vernesoft.com`).
    pub fn base_url(mut self, url: impl Into<String>) -> Self {
        self.base_url = Some(url.into());
        self
    }

    /// Set the HTTP request timeout in seconds (default: `30`).
    pub fn timeout_secs(mut self, secs: u64) -> Self {
        self.timeout_secs = Some(secs);
        self
    }

    /// Consume the builder and return a configured [`Gate`].
    ///
    /// # Errors
    ///
    /// Returns [`Error::Config`] if the API key was not set.
    pub fn build(self) -> Result<Gate, Error> {
        let key = self
            .api_key
            .ok_or_else(|| Error::Config("gate API key is required".into()))?;
        let http = HttpClient::new(key, self.base_url, self.timeout_secs)?;
        Ok(Gate::from_http(Arc::new(http)))
    }
}

/// Access to the `/v1/gate/identities` endpoints.
///
/// Obtain via [`Gate::identities`].
pub struct IdentitiesClient {
    http: Arc<HttpClient>,
}

impl IdentitiesClient {
    /// Create a new identity.
    ///
    /// Maps to `POST /v1/gate/identities`. A second end user with the same address in
    /// your account is `409 conflict`.
    pub async fn create(&self, params: CreateIdentityParams) -> Result<Identity, Error> {
        self.http.post("/v1/gate/identities", &params).await
    }

    /// Fetch a single identity by ID.
    ///
    /// Maps to `GET /v1/gate/identities/{id}`.
    pub async fn get(&self, identity_id: &str) -> Result<Identity, Error> {
        self.http
            .get(&format!("/v1/gate/identities/{}", seg(identity_id)))
            .await
    }

    /// Partially update an identity using [RFC 6902](https://datatracker.ietf.org/doc/html/rfc6902)
    /// JSON Patch operations.
    ///
    /// Only `/traits/email` and `/traits/locale` exist to patch; put your own data in
    /// [custom metadata](Self::update_metadata). Maps to `PATCH /v1/gate/identities/{id}`.
    pub async fn patch(&self, identity_id: &str, ops: Vec<JsonPatchOp>) -> Result<Identity, Error> {
        self.http
            .patch(&format!("/v1/gate/identities/{}", seg(identity_id)), &ops)
            .await
    }

    /// Permanently delete an identity.
    ///
    /// Maps to `DELETE /v1/gate/identities/{id}`.
    pub async fn delete(&self, identity_id: &str) -> Result<(), Error> {
        self.http
            .delete(&format!("/v1/gate/identities/{}", seg(identity_id)))
            .await
    }

    /// Activate or deactivate an identity.
    ///
    /// An `"inactive"` identity cannot log in — Kratos rejects its credentials
    /// automatically — until it is reactivated. The identity is not deleted. Its
    /// sessions are suspended, not revoked; introspection of its access tokens answers
    /// `active: false`; and no signed tokens are issued for it. Fires the
    /// `identity.state_changed` webhook event.
    ///
    /// Maps to `PATCH /v1/gate/identities/{id}/state`. `state` must be
    /// `"active"` or `"inactive"`.
    pub async fn set_state(&self, identity_id: &str, state: &str) -> Result<Identity, Error> {
        #[derive(serde::Serialize)]
        struct StateBody<'a> {
            state: &'a str,
        }

        self.http
            .patch(
                &format!("/v1/gate/identities/{}/state", seg(identity_id)),
                &StateBody { state },
            )
            .await
    }

    /// Activate an identity — convenience wrapper for [`set_state`](Self::set_state)
    /// with `"active"`.
    pub async fn activate(&self, identity_id: &str) -> Result<Identity, Error> {
        self.set_state(identity_id, "active").await
    }

    /// Deactivate an identity — convenience wrapper for [`set_state`](Self::set_state)
    /// with `"inactive"`.
    pub async fn deactivate(&self, identity_id: &str) -> Result<Identity, Error> {
        self.set_state(identity_id, "inactive").await
    }

    /// Trigger a new email verification flow for an identity.
    ///
    /// Useful when the original verification email expired or was never
    /// received; the user receives a fresh verification email.
    ///
    /// Maps to `POST /v1/gate/identities/{id}/resend-verification`.
    pub async fn resend_verification(&self, identity_id: &str) -> Result<(), Error> {
        self.http
            .post_discard(&format!(
                "/v1/gate/identities/{}/resend-verification",
                seg(identity_id)
            ))
            .await
    }

    /// Read an identity's custom metadata (`metadata_public`) — the place for your own
    /// fields about a user. Empty when nothing is set.
    ///
    /// Maps to `GET /v1/gate/identities/{id}/metadata`. An identity that is not your end
    /// user is `403 forbidden`, not `404`.
    pub async fn get_metadata(&self, identity_id: &str) -> Result<Map<String, Value>, Error> {
        #[derive(serde::Deserialize)]
        struct Wrapper {
            metadata_public: Option<Map<String, Value>>,
        }

        let wrapped: Wrapper = self
            .http
            .get(&format!(
                "/v1/gate/identities/{}/metadata",
                seg(identity_id)
            ))
            .await?;
        Ok(wrapped.metadata_public.unwrap_or_default())
    }

    /// Merge `patch` into an identity's custom metadata and return the result. Keys you
    /// leave out are kept; a key set to `null` is removed.
    ///
    /// Maps to `PATCH /v1/gate/identities/{id}/metadata`.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use nautilus_rs::Gate;
    /// use serde_json::json;
    ///
    /// # async fn run() -> Result<(), nautilus_rs::Error> {
    /// let gate = Gate::new("vrn_9f8a7c1b2d3e4f50");
    /// // sets `plan`, removes `seats`
    /// let merged = gate.identities()
    ///     .update_metadata("f47ac10b-58cc-4372-a567-0e02b2c3d479", json!({ "plan": "pro", "seats": null }))
    ///     .await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn update_metadata<P: serde::Serialize>(
        &self,
        identity_id: &str,
        patch: P,
    ) -> Result<Map<String, Value>, Error> {
        #[derive(serde::Deserialize)]
        struct Wrapper {
            metadata_public: Option<Map<String, Value>>,
        }

        let wrapped: Wrapper = self
            .http
            .patch(
                &format!("/v1/gate/identities/{}/metadata", seg(identity_id)),
                &patch,
            )
            .await?;
        Ok(wrapped.metadata_public.unwrap_or_default())
    }

    /// The organizations of the key's project that an end user belongs to, with the role
    /// held in each and its permissions — live, oldest membership first, cursor-paged.
    /// This is the complete list that `orgs_truncated` (in an introspection, a session
    /// check or a signed token) points to.
    ///
    /// Maps to `GET /v1/gate/identities/{id}/organizations`. Another account's user is
    /// `403 forbidden`.
    pub async fn list_organizations(
        &self,
        identity_id: &str,
        params: ListParams,
    ) -> Result<Paginated<OrgClaim>, Error> {
        let call = Call::new(
            Method::GET,
            format!("/v1/gate/identities/{}/organizations", seg(identity_id)),
        )
        .query("limit", params.limit)
        .query("cursor", params.cursor);
        self.http.send(call).await
    }
}

/// Access to the `/v1/gate/tokens` endpoints, and to signed tokens.
///
/// Obtain via [`Gate::tokens`].
pub struct TokensClient {
    http: Arc<HttpClient>,
    #[cfg(feature = "jwt")]
    verifiers: VerifierCache,
}

impl TokensClient {
    /// Issue a short-lived access token (`gat_live_…`) for a subject — a user or a
    /// service account.
    ///
    /// Maps to `POST /v1/gate/tokens`, answered `201`. The key travels in the
    /// `Authorization` header like on every other call. (Before 1.4.0 this SDK sent it
    /// in the body without the header, which the API refuses with `401`.)
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
    ///     scopes: Some(vec!["read:profile".into()]),
    ///     ttl_seconds: Some(900),
    /// }).await?;
    /// println!("expires at {}", token.expires_at);
    /// # Ok(())
    /// # }
    /// ```
    pub async fn create(&self, params: CreateTokenParams) -> Result<AccessToken, Error> {
        self.http.post("/v1/gate/tokens", &params).await
    }

    /// Validate an access token and retrieve its claims.
    ///
    /// Maps to `POST /v1/gate/tokens/introspect`. Check
    /// [`TokenInfo::active`] to determine whether the token is still valid; an
    /// unknown, expired or foreign token, or one whose user you deactivated or deleted,
    /// is `active: false` with every other field `None`. When the subject is an end
    /// user, [`orgs`](TokenInfo::orgs) lists their organizations, read at the moment you
    /// ask.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use nautilus_rs::Gate;
    ///
    /// # async fn run() -> Result<(), nautilus_rs::Error> {
    /// let gate = Gate::new("vrn_9f8a7c1b2d3e4f50");
    /// let info = gate.tokens().introspect("gat_live_…").await?;
    /// if info.active {
    ///     println!("valid token for {:?}", info.subject);
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub async fn introspect(&self, access_token: &str) -> Result<TokenInfo, Error> {
        #[derive(serde::Serialize)]
        struct IntrospectBody<'a> {
            access_token: &'a str,
        }

        self.http
            .post(
                "/v1/gate/tokens/introspect",
                &IntrospectBody { access_token },
            )
            .await
    }

    /// Get a **signed JWT** (ES256) for a signed-in end user's session, for a platform
    /// that verifies signatures instead of calling Gate (Supabase, an API gateway).
    ///
    /// The session token is the credential: **no API key is sent**. The token is valid
    /// for 15 minutes and **cannot be revoked**; its `orgs` claim is as old as the token.
    /// Every call mints a new one — ask again before the one you hold expires. A revoked
    /// or expired session, or a deactivated user, is `401`; a dead identity service is
    /// `502 upstream_unavailable`.
    ///
    /// Maps to `GET /auth/sessions/whoami?tokenize_as=gate_jwt`.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use nautilus_rs::Gate;
    ///
    /// # async fn run(session_token: &str) -> Result<(), nautilus_rs::Error> {
    /// let gate = Gate::new("vrn_9f8a7c1b2d3e4f50");
    /// let signed = gate.tokens().create_signed(session_token).await?;
    /// println!("{}", signed.token);
    /// # Ok(())
    /// # }
    /// ```
    pub async fn create_signed(&self, session_token: &str) -> Result<SignedToken, Error> {
        let call = Call::new(Method::GET, "/auth/sessions/whoami")
            .query("tokenize_as", Some("gate_jwt"))
            .session_token(session_token)
            .skip_auth();
        let mut body: Map<String, Value> = self.http.send(call).await?;
        let token = match body.remove("tokenized") {
            Some(Value::String(token)) => token,
            _ => {
                return Err(Error::Config(
                    "the whoami answer carried no `tokenized` token".into(),
                ))
            }
        };
        let session: Session = serde_json::from_value(Value::Object(body))?;
        Ok(SignedToken { token, session })
    }

    /// Verify a Gate signed token offline against your tenant's public keys (fetched once
    /// and cached) and return its claims. Needs the `jwt` feature.
    ///
    /// Returns [`Error::Token`] when the token must not be trusted — wrong issuer
    /// (another tenant), bad signature, expired, any algorithm but ES256. The issuer is
    /// `<this client's base URL>/auth/<tenant_id>`. Use
    /// [`GateTokenVerifier`](crate::GateTokenVerifier) to verify without a Gate client, or
    /// to change the clock leeway.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # #[cfg(feature = "jwt")]
    /// # async fn run(token: &str) -> Result<(), nautilus_rs::Error> {
    /// use nautilus_rs::Gate;
    ///
    /// let gate = Gate::new("vrn_9f8a7c1b2d3e4f50");
    /// let claims = gate.tokens()
    ///     .verify_signed(token, "550e8400-e29b-41d4-a716-446655440000")
    ///     .await?;
    /// println!("user {}", claims.sub);
    /// # Ok(())
    /// # }
    /// ```
    #[cfg(feature = "jwt")]
    pub async fn verify_signed(
        &self,
        token: &str,
        tenant_id: &str,
    ) -> Result<types::SignedTokenClaims, Error> {
        let verifier = {
            // A poisoned lock only means another verify panicked; the map is still a map.
            let mut cache = self.verifiers.lock().unwrap_or_else(|e| e.into_inner());
            match cache.get(tenant_id) {
                Some(verifier) => Arc::clone(verifier),
                None => {
                    let verifier = Arc::new(
                        GateTokenVerifier::builder()
                            .tenant_id(tenant_id)
                            .base_url(self.http.base_url())
                            .build()?,
                    );
                    cache.insert(tenant_id.to_owned(), Arc::clone(&verifier));
                    verifier
                }
            }
        };
        verifier.verify(token).await
    }
}

/// Access to the `/v1/gate/settings` endpoints.
///
/// Obtain via [`Gate::settings`].
pub struct SettingsClient {
    http: Arc<HttpClient>,
}

impl SettingsClient {
    /// Fetch the tenant's security settings (passwordless / MFA). With `mfa_enabled`,
    /// [`SessionsClient::check`] refuses a session without a second factor
    /// (`403 mfa_required`).
    ///
    /// Maps to `GET /v1/gate/settings/security`.
    pub async fn get_security(&self) -> Result<SecuritySettings, Error> {
        self.http.get("/v1/gate/settings/security").await
    }

    /// Replace the tenant's security settings.
    ///
    /// Both fields are always sent — the update is a full replacement, not a
    /// merge. Maps to `PUT /v1/gate/settings/security`.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use nautilus_rs::{Gate, SecuritySettings};
    ///
    /// # async fn run() -> Result<(), nautilus_rs::Error> {
    /// let gate = Gate::new("vrn_9f8a7c1b2d3e4f50");
    /// gate.settings().update_security(SecuritySettings {
    ///     passwordless_enabled: true,
    ///     mfa_enabled: false,
    /// }).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn update_security(&self, settings: SecuritySettings) -> Result<(), Error> {
        self.http
            .put_discard("/v1/gate/settings/security", &settings)
            .await
    }

    /// List the tenant's social login (OIDC) providers and whether each is
    /// enabled — covering every provider Gate supports, regardless of state.
    ///
    /// Maps to `GET /v1/gate/settings/oidc-providers`.
    pub async fn get_oidc_providers(&self) -> Result<Vec<OidcProvider>, Error> {
        #[derive(serde::Deserialize)]
        struct Wrapper {
            providers: Vec<OidcProvider>,
        }

        let wrapped: Wrapper = self.http.get("/v1/gate/settings/oidc-providers").await?;
        Ok(wrapped.providers)
    }

    /// Set the `enabled` flag for one or more social login providers.
    ///
    /// Any provider omitted from `providers` is left unchanged. Returns the
    /// full, updated provider list. Maps to
    /// `PUT /v1/gate/settings/oidc-providers`.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use nautilus_rs::{Gate, OidcProvider};
    ///
    /// # async fn run() -> Result<(), nautilus_rs::Error> {
    /// let gate = Gate::new("vrn_9f8a7c1b2d3e4f50");
    /// let providers = gate.settings().update_oidc_providers(vec![
    ///     OidcProvider { provider: "github".into(), enabled: true },
    ///     OidcProvider { provider: "google".into(), enabled: true },
    /// ]).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn update_oidc_providers(
        &self,
        providers: Vec<OidcProvider>,
    ) -> Result<Vec<OidcProvider>, Error> {
        #[derive(serde::Serialize)]
        struct Body {
            providers: Vec<OidcProvider>,
        }

        #[derive(serde::Deserialize)]
        struct Wrapper {
            providers: Vec<OidcProvider>,
        }

        let wrapped: Wrapper = self
            .http
            .put("/v1/gate/settings/oidc-providers", &Body { providers })
            .await?;
        Ok(wrapped.providers)
    }

    /// The project's social-login return URLs — the addresses on your backend that
    /// receive the user (with a one-time `code`) after a social login. The first is the
    /// default.
    ///
    /// Maps to `GET /v1/gate/settings/return-urls`.
    pub async fn get_return_urls(&self) -> Result<Vec<String>, Error> {
        #[derive(serde::Deserialize)]
        struct Wrapper {
            return_urls: Vec<String>,
        }

        let wrapped: Wrapper = self.http.get("/v1/gate/settings/return-urls").await?;
        Ok(wrapped.return_urls)
    }

    /// Replace the project's return URLs (at most 10; an empty list switches the feature
    /// off) and return them as they will be matched — exactly, never by prefix.
    ///
    /// `https` only, except `http://localhost` and `http://127.0.0.1`; no query,
    /// fragment, `@` or wildcard (`400 invalid_payload` names the entry). Maps to
    /// `PUT /v1/gate/settings/return-urls`.
    pub async fn update_return_urls(&self, urls: Vec<String>) -> Result<Vec<String>, Error> {
        #[derive(serde::Serialize)]
        struct Body {
            return_urls: Vec<String>,
        }

        #[derive(serde::Deserialize)]
        struct Wrapper {
            return_urls: Vec<String>,
        }

        let wrapped: Wrapper = self
            .http
            .put("/v1/gate/settings/return-urls", &Body { return_urls: urls })
            .await?;
        Ok(wrapped.return_urls)
    }
}
