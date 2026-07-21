use std::time::Duration;

use serde::{de::DeserializeOwned, Serialize};

use crate::error::{ApiError, Error};

const API_URL: &str = "https://api.vernesoft.com";

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
            base_url: base_url.unwrap_or_else(|| API_URL.into()),
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

    /// Send a `PUT` request and deserialize the JSON response body.
    pub async fn put<B: Serialize, T: DeserializeOwned>(
        &self,
        path: &str,
        body: &B,
    ) -> Result<T, Error> {
        let url = format!("{}{}", self.base_url, path);
        let resp = self
            .client
            .put(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(body)
            .send()
            .await
            .map_err(Error::Http)?;

        self.parse_response(resp).await
    }

    /// Send a `PUT` request, discarding the (successful) response body.
    ///
    /// Used by endpoints that reply with a bare `{"status":"ok"}` acknowledgement.
    pub async fn put_discard<B: Serialize>(&self, path: &str, body: &B) -> Result<(), Error> {
        let url = format!("{}{}", self.base_url, path);
        let resp = self
            .client
            .put(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(body)
            .send()
            .await
            .map_err(Error::Http)?;

        self.check_empty(resp).await
    }

    /// Send a bodyless `POST` request, discarding the (successful) response body.
    ///
    /// Used by action endpoints that reply with `204 No Content`.
    pub async fn post_discard(&self, path: &str) -> Result<(), Error> {
        let url = format!("{}{}", self.base_url, path);
        let resp = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .send()
            .await
            .map_err(Error::Http)?;

        self.check_empty(resp).await
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

        self.check_empty(resp).await
    }

    /// Validate a response status without deserializing the body — for endpoints
    /// that return no meaningful content (`204`, `{"status":"ok"}`, …).
    async fn check_empty(&self, resp: reqwest::Response) -> Result<(), Error> {
        let status = resp.status();
        if status.is_success() {
            return Ok(());
        }

        let env: ErrorEnvelope = resp.json().await.map_err(Error::Http)?;
        Err(Error::Api(ApiError {
            code: env.error.code,
            message: env.error.message,
            status: status.as_u16(),
            request_id: env.error.request_id,
        }))
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
