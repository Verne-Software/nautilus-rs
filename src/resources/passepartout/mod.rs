pub mod types;

use std::sync::Arc;

use crate::{error::Error, http::HttpClient};
use types::{LoginStart, LoginStatus, TokenIntrospection};

/// Passepartout service client — Telegram Auth-as-a-Service.
///
/// Lets your users sign in with Telegram: start a login flow, poll for its
/// completion, and introspect the access tokens it issues.
///
/// Obtain a `Passepartout` instance either as part of the unified [`Verne`]
/// client or standalone:
///
/// ```no_run
/// // Standalone
/// use nautilus_rs::Passepartout;
/// let passepartout = Passepartout::new("vrn_passepartout_live_sk_…");
///
/// // Via unified client
/// use nautilus_rs::Verne;
/// # fn run() -> Result<(), nautilus_rs::Error> {
/// let verne = Verne::builder().passepartout("vrn_passepartout_live_sk_…").build()?;
/// let passepartout = verne.passepartout()?;
/// # Ok(())
/// # }
/// ```
///
/// [`Verne`]: crate::Verne
pub struct Passepartout {
    http: Arc<HttpClient>,
    // Retained for symmetry with the other service clients and future
    // body-authenticated endpoints; all current endpoints authenticate via the
    // `Authorization` header set on the shared `HttpClient`.
    #[allow(dead_code)]
    api_key: String,
}

impl std::fmt::Debug for Passepartout {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Passepartout").finish_non_exhaustive()
    }
}

impl Passepartout {
    /// Create a `Passepartout` client with default settings.
    ///
    /// Panics if the API key is empty or the HTTP client cannot be initialized.
    /// Use [`Passepartout::builder`] for fallible construction.
    pub fn new(api_key: impl Into<String>) -> Self {
        let key = api_key.into();
        Self::builder()
            .api_key(&key)
            .build()
            .expect("failed to build Passepartout client")
    }

    /// Return a [`PassepartoutBuilder`] for fine-grained configuration.
    pub fn builder() -> PassepartoutBuilder {
        PassepartoutBuilder::default()
    }

    pub(crate) fn from_http(http: Arc<HttpClient>, api_key: String) -> Self {
        Self { http, api_key }
    }

    /// Start a Telegram login flow.
    ///
    /// Returns a [`LoginStart`] containing the `deep_link` the end-user must
    /// open in Telegram and the `nonce` used to poll for completion. Maps to
    /// `POST /v1/passepartout/login/start`.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use nautilus_rs::Passepartout;
    ///
    /// # async fn run() -> Result<(), nautilus_rs::Error> {
    /// let passepartout = Passepartout::new("vrn_passepartout_live_sk_…");
    /// let flow = passepartout.login_start().await?;
    /// println!("open in Telegram: {}", flow.deep_link);
    /// # Ok(())
    /// # }
    /// ```
    pub async fn login_start(&self) -> Result<LoginStart, Error> {
        self.http
            .post("/v1/passepartout/login/start", &serde_json::json!({}), false)
            .await
    }

    /// Poll the status of a Telegram login flow.
    ///
    /// Call repeatedly with the `nonce` returned by
    /// [`login_start`](Self::login_start) until the flow reaches a terminal
    /// state. Maps to `GET /v1/passepartout/login/status?nonce=…`.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use nautilus_rs::Passepartout;
    ///
    /// # async fn run() -> Result<(), nautilus_rs::Error> {
    /// let passepartout = Passepartout::new("vrn_passepartout_live_sk_…");
    /// let status = passepartout.login_status("nonce_abc").await?;
    /// if status.status == "completed" {
    ///     println!("token: {:?}", status.access_token);
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub async fn login_status(&self, nonce: &str) -> Result<LoginStatus, Error> {
        let encoded = urlencode(nonce);
        self.http
            .get(&format!("/v1/passepartout/login/status?nonce={encoded}"))
            .await
    }

    /// Validate an access token and retrieve its claims.
    ///
    /// Maps to `POST /v1/passepartout/tokens/introspect`. Check
    /// [`TokenIntrospection::active`] to determine whether the token is still
    /// valid.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use nautilus_rs::Passepartout;
    ///
    /// # async fn run() -> Result<(), nautilus_rs::Error> {
    /// let passepartout = Passepartout::new("vrn_passepartout_live_sk_…");
    /// let info = passepartout.introspect("eyJ…").await?;
    /// if info.active {
    ///     println!("valid token for {:?}", info.subject);
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub async fn introspect(&self, access_token: &str) -> Result<TokenIntrospection, Error> {
        #[derive(serde::Serialize)]
        struct IntrospectBody<'a> {
            access_token: &'a str,
        }

        self.http
            .post(
                "/v1/passepartout/tokens/introspect",
                &IntrospectBody { access_token },
                false,
            )
            .await
    }
}

/// Percent-encode a query-string value, escaping every character that is not an
/// [RFC 3986](https://datatracker.ietf.org/doc/html/rfc3986) unreserved
/// character.
fn urlencode(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char);
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

/// Builder for a standalone [`Passepartout`] client.
///
/// # Example
///
/// ```no_run
/// use nautilus_rs::Passepartout;
///
/// let passepartout = Passepartout::builder()
///     .api_key("vrn_passepartout_live_sk_…")
///     .timeout_secs(15)
///     .build()
///     .expect("invalid configuration");
/// ```
#[derive(Default)]
pub struct PassepartoutBuilder {
    api_key: Option<String>,
    base_url: Option<String>,
    timeout_secs: Option<u64>,
}

impl PassepartoutBuilder {
    /// Set the Passepartout API key (**required**).
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

    /// Consume the builder and return a configured [`Passepartout`].
    ///
    /// # Errors
    ///
    /// Returns [`Error::Config`] if the API key was not set.
    pub fn build(self) -> Result<Passepartout, Error> {
        let key = self
            .api_key
            .ok_or_else(|| Error::Config("passepartout API key is required".into()))?;
        let http = HttpClient::new(&key, self.base_url, self.timeout_secs)?;
        Ok(Passepartout {
            http: Arc::new(http),
            api_key: key,
        })
    }
}
