pub mod types;

use std::sync::Arc;

use crate::{error::Error, http::HttpClient, types::Paginated};
use types::{ListMessagesParams, Message, SendMessageParams};

pub struct Relay {
    http: Arc<HttpClient>,
}

impl std::fmt::Debug for Relay {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Relay").finish_non_exhaustive()
    }
}

impl Relay {
    pub fn new(api_key: impl Into<String>) -> Self {
        Self::builder()
            .api_key(api_key)
            .build()
            .expect("failed to build Relay client")
    }

    pub fn builder() -> RelayBuilder {
        RelayBuilder::default()
    }

    pub(crate) fn from_http(http: Arc<HttpClient>) -> Self {
        Self { http }
    }

    pub fn messages(&self) -> MessagesClient {
        MessagesClient {
            http: Arc::clone(&self.http),
        }
    }
}

#[derive(Default)]
pub struct RelayBuilder {
    api_key: Option<String>,
    base_url: Option<String>,
    timeout_secs: Option<u64>,
}

impl RelayBuilder {
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

    pub fn build(self) -> Result<Relay, Error> {
        let key = self
            .api_key
            .ok_or_else(|| Error::Config("relay API key is required".into()))?;
        let http = HttpClient::new(key, self.base_url, self.timeout_secs)?;
        Ok(Relay {
            http: Arc::new(http),
        })
    }
}

pub struct MessagesClient {
    http: Arc<HttpClient>,
}

impl MessagesClient {
    pub async fn send(&self, params: SendMessageParams) -> Result<Message, Error> {
        self.http.post("/v1/relay/messages", &params, false).await
    }

    pub async fn list(&self, params: ListMessagesParams) -> Result<Paginated<Message>, Error> {
        let mut query = vec![];
        if let Some(limit) = params.limit {
            query.push(format!("limit={limit}"));
        }
        if let Some(cursor) = &params.cursor {
            query.push(format!("cursor={cursor}"));
        }
        if let Some(event_type) = &params.event_type {
            query.push(format!("event_type={event_type}"));
        }

        let path = if query.is_empty() {
            "/v1/relay/messages".to_string()
        } else {
            format!("/v1/relay/messages?{}", query.join("&"))
        };

        self.http.get(&path).await
    }
}
