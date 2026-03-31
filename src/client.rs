use std::sync::Arc;

use crate::{
    error::Error,
    http::HttpClient,
    resources::{gate::Gate, relay::Relay},
};

#[derive(Default)]
pub struct VerneBuilder {
    relay_key: Option<String>,
    gate_key: Option<String>,
    base_url: Option<String>,
    timeout_secs: Option<u64>,
}

impl VerneBuilder {
    pub fn relay(mut self, key: impl Into<String>) -> Self {
        self.relay_key = Some(key.into());
        self
    }

    pub fn gate(mut self, key: impl Into<String>) -> Self {
        self.gate_key = Some(key.into());
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

    pub fn build(self) -> Result<Verne, Error> {
        let relay = self
            .relay_key
            .map(|key| {
                let http = HttpClient::new(&key, self.base_url.clone(), self.timeout_secs)?;
                Ok::<_, Error>(Arc::new(Relay::from_http(Arc::new(http))))
            })
            .transpose()?;

        let gate = self
            .gate_key
            .map(|key| {
                let http = HttpClient::new(&key, self.base_url.clone(), self.timeout_secs)?;
                Ok::<_, Error>(Arc::new(Gate::from_http(Arc::new(http), key)))
            })
            .transpose()?;

        Ok(Verne { relay, gate })
    }
}

pub struct Verne {
    relay: Option<Arc<Relay>>,
    gate: Option<Arc<Gate>>,
}

impl Verne {
    pub fn builder() -> VerneBuilder {
        VerneBuilder::default()
    }

    pub fn relay(&self) -> Result<&Relay, Error> {
        self.relay
            .as_deref()
            .ok_or_else(|| Error::Config("no relay API key configured".into()))
    }

    pub fn gate(&self) -> Result<&Gate, Error> {
        self.gate
            .as_deref()
            .ok_or_else(|| Error::Config("no gate API key configured".into()))
    }
}
