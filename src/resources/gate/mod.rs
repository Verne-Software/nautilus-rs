pub mod types;

use std::sync::Arc;

use crate::{error::Error, http::HttpClient};
use types::{
    AccessToken, AuthorizationDecision, AuthorizeParams, CreateIdentityParams, CreateTokenParams,
    Identity, JsonPatchOp, SecuritySettings, TokenInfo,
};

/// Gate service client — Auth-as-a-Service.
///
/// Provides identity management, short-lived access token issuance, and
/// policy-based authorization checks.
///
/// Obtain a `Gate` instance either as part of the unified [`Verne`] client or
/// standalone:
///
/// ```no_run
/// // Standalone
/// use nautilus::Gate;
/// let gate = Gate::new("vrn_gate_live_sk_…");
///
/// // Via unified client
/// use nautilus::Verne;
/// # fn run() -> Result<(), nautilus::Error> {
/// let verne = Verne::builder().gate("vrn_gate_live_sk_…").build()?;
/// let gate = verne.gate()?;
/// # Ok(())
/// # }
/// ```
///
/// [`Verne`]: crate::Verne
pub struct Gate {
    http: Arc<HttpClient>,
    api_key: String,
}

impl std::fmt::Debug for Gate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Gate").finish_non_exhaustive()
    }
}

impl Gate {
    /// Create a `Gate` client with default settings.
    ///
    /// Panics if the API key is empty or the HTTP client cannot be initialized.
    /// Use [`Gate::builder`] for fallible construction.
    pub fn new(api_key: impl Into<String>) -> Self {
        let key = api_key.into();
        Self::builder()
            .api_key(&key)
            .build()
            .expect("failed to build Gate client")
    }

    /// Return a [`GateBuilder`] for fine-grained configuration.
    pub fn builder() -> GateBuilder {
        GateBuilder::default()
    }

    pub(crate) fn from_http(http: Arc<HttpClient>, api_key: String) -> Self {
        Self { http, api_key }
    }

    /// Return an [`IdentitiesClient`] for CRUD operations on identities.
    pub fn identities(&self) -> IdentitiesClient {
        IdentitiesClient {
            http: Arc::clone(&self.http),
        }
    }

    /// Return a [`TokensClient`] for issuing and introspecting access tokens.
    pub fn tokens(&self) -> TokensClient {
        TokensClient {
            http: Arc::clone(&self.http),
            api_key: self.api_key.clone(),
        }
    }

    /// Return a [`SettingsClient`] for reading and updating tenant settings.
    pub fn settings(&self) -> SettingsClient {
        SettingsClient {
            http: Arc::clone(&self.http),
        }
    }

    /// Check whether a subject is allowed to perform an action on a resource.
    ///
    /// Maps to `POST /v1/gate/authorize`.
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
    ///     action: "read".into(),
    ///     resource: "report:rpt_456".into(),
    ///     context: None,
    /// }).await?;
    /// assert!(decision.allowed);
    /// # Ok(())
    /// # }
    /// ```
    pub async fn authorize(&self, params: AuthorizeParams) -> Result<AuthorizationDecision, Error> {
        self.http.post("/v1/gate/authorize", &params, false).await
    }
}

/// Builder for a standalone [`Gate`] client.
///
/// # Example
///
/// ```no_run
/// use nautilus::Gate;
///
/// let gate = Gate::builder()
///     .api_key("vrn_gate_live_sk_…")
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
    /// Set the Gate API key (**required**).
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
        let http = HttpClient::new(&key, self.base_url, self.timeout_secs)?;
        Ok(Gate {
            http: Arc::new(http),
            api_key: key,
        })
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
    /// Maps to `POST /v1/gate/identities`.
    pub async fn create(&self, params: CreateIdentityParams) -> Result<Identity, Error> {
        self.http.post("/v1/gate/identities", &params, false).await
    }

    /// Fetch a single identity by ID.
    ///
    /// Maps to `GET /v1/gate/identities/{id}`.
    pub async fn get(&self, identity_id: &str) -> Result<Identity, Error> {
        self.http
            .get(&format!("/v1/gate/identities/{identity_id}"))
            .await
    }

    /// Partially update an identity using [RFC 6902](https://datatracker.ietf.org/doc/html/rfc6902)
    /// JSON Patch operations.
    ///
    /// Maps to `PATCH /v1/gate/identities/{id}`.
    pub async fn patch(&self, identity_id: &str, ops: Vec<JsonPatchOp>) -> Result<Identity, Error> {
        self.http
            .patch(&format!("/v1/gate/identities/{identity_id}"), &ops)
            .await
    }

    /// Permanently delete an identity.
    ///
    /// Maps to `DELETE /v1/gate/identities/{id}`.
    pub async fn delete(&self, identity_id: &str) -> Result<(), Error> {
        self.http
            .delete(&format!("/v1/gate/identities/{identity_id}"))
            .await
    }

    /// Activate or deactivate an identity.
    ///
    /// An `"inactive"` identity cannot log in — Kratos rejects its credentials
    /// automatically — until it is reactivated. The identity is not deleted.
    /// Fires the `identity.state_changed` webhook event.
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
                &format!("/v1/gate/identities/{identity_id}/state"),
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
            .post_discard(&format!("/v1/gate/identities/{identity_id}/resend-verification"))
            .await
    }
}

/// Access to the `/v1/gate/tokens` endpoints.
///
/// Obtain via [`Gate::tokens`].
pub struct TokensClient {
    http: Arc<HttpClient>,
    api_key: String,
}

impl TokensClient {
    /// Issue a short-lived access token for a subject.
    ///
    /// Maps to `POST /v1/gate/tokens`. The API key is sent in the request body
    /// rather than the `Authorization` header.
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
    ///     scopes: Some(vec!["read:profile".into()]),
    ///     ttl_seconds: Some(900),
    /// }).await?;
    /// println!("expires at {}", token.expires_at);
    /// # Ok(())
    /// # }
    /// ```
    pub async fn create(&self, params: CreateTokenParams) -> Result<AccessToken, Error> {
        #[derive(serde::Serialize)]
        struct CreateTokenBody {
            api_key: String,
            subject: String,
            #[serde(skip_serializing_if = "Option::is_none")]
            scopes: Option<Vec<String>>,
            #[serde(skip_serializing_if = "Option::is_none")]
            ttl_seconds: Option<u64>,
        }

        let body = CreateTokenBody {
            api_key: self.api_key.clone(),
            subject: params.subject,
            scopes: params.scopes,
            ttl_seconds: params.ttl_seconds,
        };

        self.http.post("/v1/gate/tokens", &body, true).await
    }

    /// Validate an access token and retrieve its claims.
    ///
    /// Maps to `POST /v1/gate/tokens/introspect`. Check
    /// [`TokenInfo::active`] to determine whether the token is still valid.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use nautilus::Gate;
    ///
    /// # async fn run() -> Result<(), nautilus::Error> {
    /// let gate = Gate::new("vrn_gate_live_sk_…");
    /// let info = gate.tokens().introspect("eyJ…").await?;
    /// if info.active {
    ///     println!("valid token for {}", info.subject);
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
                false,
            )
            .await
    }
}

/// Access to the `/v1/gate/settings` endpoints.
///
/// Obtain via [`Gate::settings`].
pub struct SettingsClient {
    http: Arc<HttpClient>,
}

impl SettingsClient {
    /// Fetch the tenant's security settings (passwordless / MFA).
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
    /// use nautilus::{Gate, SecuritySettings};
    ///
    /// # async fn run() -> Result<(), nautilus::Error> {
    /// let gate = Gate::new("vrn_gate_live_sk_…");
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
}
