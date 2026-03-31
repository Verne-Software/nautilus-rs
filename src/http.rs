use std::time::Duration;

use serde::{de::DeserializeOwned, Serialize};

use crate::error::{ApiError, Error};

pub(crate) struct HttpClient {
    api_key: String,
    base_url: String,
    client: reqwest::Client,
}

#[derive(serde::Deserialize)]
struct ErrorEnvelope {
    error: ErrorBody,
}

#[derive(serde::Deserialize)]
struct ErrorBody {
    code: String,
    message: String,
    #[serde(default)]
    request_id: String,
}

impl HttpClient {
    pub fn new(
        api_key: impl Into<String>,
        base_url: Option<String>,
        timeout_secs: Option<u64>,
    ) -> Result<Self, Error> {
        let timeout = Duration::from_secs(timeout_secs.unwrap_or(30));
        let client = reqwest::ClientBuilder::new()
            .timeout(timeout)
            .build()
            .map_err(Error::Http)?;

        Ok(Self {
            api_key: api_key.into(),
            base_url: base_url.unwrap_or_else(|| "https://api.vernesoft.com".into()),
            client,
        })
    }

    pub async fn get<T: DeserializeOwned>(&self, path: &str) -> Result<T, Error> {
        let url = format!("{}{}", self.base_url, path);
        let resp = self
            .client
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .send()
            .await
            .map_err(Error::Http)?;

        self.parse_response(resp).await
    }

    pub async fn post<B: Serialize, T: DeserializeOwned>(
        &self,
        path: &str,
        body: &B,
        skip_auth: bool,
    ) -> Result<T, Error> {
        self.post_inner(path, body, skip_auth, false).await
    }

    async fn post_inner<B: Serialize, T: DeserializeOwned>(
        &self,
        path: &str,
        body: &B,
        skip_auth: bool,
        is_retry: bool,
    ) -> Result<T, Error> {
        let url = format!("{}{}", self.base_url, path);
        let mut req = self
            .client
            .post(&url)
            .header("Content-Type", "application/json");

        if !skip_auth {
            req = req.header("Authorization", format!("Bearer {}", self.api_key));
        }

        let resp = req.json(body).send().await.map_err(Error::Http)?;

        if resp.status().as_u16() == 429 && !is_retry {
            let wait_secs = resp
                .headers()
                .get("Retry-After")
                .and_then(|v| v.to_str().ok())
                .and_then(|s| s.parse::<f64>().ok())
                .unwrap_or(1.0);

            tokio::time::sleep(Duration::from_secs_f64(wait_secs)).await;
            return Box::pin(self.post_inner(path, body, skip_auth, true)).await;
        }

        self.parse_response(resp).await
    }

    pub async fn patch<B: Serialize, T: DeserializeOwned>(
        &self,
        path: &str,
        body: &B,
    ) -> Result<T, Error> {
        let url = format!("{}{}", self.base_url, path);
        let resp = self
            .client
            .patch(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(body)
            .send()
            .await
            .map_err(Error::Http)?;

        self.parse_response(resp).await
    }

    pub async fn delete(&self, path: &str) -> Result<(), Error> {
        let url = format!("{}{}", self.base_url, path);
        let resp = self
            .client
            .delete(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .send()
            .await
            .map_err(Error::Http)?;

        let status = resp.status();
        if status.as_u16() == 204 {
            return Ok(());
        }

        if !status.is_success() {
            let env: ErrorEnvelope = resp.json().await.map_err(Error::Http)?;
            return Err(Error::Api(ApiError {
                code: env.error.code,
                message: env.error.message,
                status: status.as_u16(),
                request_id: env.error.request_id,
            }));
        }

        Ok(())
    }

    async fn parse_response<T: DeserializeOwned>(
        &self,
        resp: reqwest::Response,
    ) -> Result<T, Error> {
        let status = resp.status();

        if !status.is_success() {
            let env: ErrorEnvelope = resp.json().await.map_err(Error::Http)?;
            return Err(Error::Api(ApiError {
                code: env.error.code,
                message: env.error.message,
                status: status.as_u16(),
                request_id: env.error.request_id,
            }));
        }

        let result = resp.json::<T>().await.map_err(Error::Http)?;
        Ok(result)
    }
}
