pub mod types;

use std::sync::Arc;

use crate::{error::Error, http::HttpClient};
use types::{
    AccessToken, AuthorizationDecision, AuthorizeParams, CreateIdentityParams, CreateTokenParams,
    Identity, JsonPatchOp, TokenInfo,
};

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
    pub fn new(api_key: impl Into<String>) -> Self {
        let key = api_key.into();
        Self::builder()
            .api_key(&key)
            .build()
            .expect("failed to build Gate client")
    }

    pub fn builder() -> GateBuilder {
        GateBuilder::default()
    }

    pub(crate) fn from_http(http: Arc<HttpClient>, api_key: String) -> Self {
        Self { http, api_key }
    }

    pub fn identities(&self) -> IdentitiesClient {
        IdentitiesClient {
            http: Arc::clone(&self.http),
        }
    }

    pub fn tokens(&self) -> TokensClient {
        TokensClient {
            http: Arc::clone(&self.http),
            api_key: self.api_key.clone(),
        }
    }

    pub async fn authorize(&self, params: AuthorizeParams) -> Result<AuthorizationDecision, Error> {
        self.http.post("/v1/gate/authorize", &params, false).await
    }
}

#[derive(Default)]
pub struct GateBuilder {
    api_key: Option<String>,
    base_url: Option<String>,
    timeout_secs: Option<u64>,
}

impl GateBuilder {
    pub fn api_key(mut self, key: impl Into<String>) -> Self {
        self.api_key = Some(key.into());
        self
    }

    pub fn base_url(mut self, url: impl Into<String>) -> Self {
        self.base_url = Some(url.into());
        self
    }

    pub fn timeout_secs(mut self, secs: u64) -> Self {
        self.timeout_secs = Some(secs);
        self
    }

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

pub struct IdentitiesClient {
    http: Arc<HttpClient>,
}

impl IdentitiesClient {
    pub async fn create(&self, params: CreateIdentityParams) -> Result<Identity, Error> {
        self.http.post("/v1/gate/identities", &params, false).await
    }

    pub async fn get(&self, identity_id: &str) -> Result<Identity, Error> {
        self.http
            .get(&format!("/v1/gate/identities/{identity_id}"))
            .await
    }

    pub async fn patch(&self, identity_id: &str, ops: Vec<JsonPatchOp>) -> Result<Identity, Error> {
        self.http
            .patch(&format!("/v1/gate/identities/{identity_id}"), &ops)
            .await
    }

    pub async fn delete(&self, identity_id: &str) -> Result<(), Error> {
        self.http
            .delete(&format!("/v1/gate/identities/{identity_id}"))
            .await
    }
}

pub struct TokensClient {
    http: Arc<HttpClient>,
    api_key: String,
}

impl TokensClient {
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
