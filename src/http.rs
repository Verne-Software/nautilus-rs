use std::time::Duration;

use reqwest::Method;
use serde::{de::DeserializeOwned, Serialize};

use crate::error::{ApiError, Error};

pub(crate) const API_URL: &str = "https://api.vernesoft.com";

/// The longest `Retry-After` the client waits out on its own before retrying a `429`.
///
/// Anything longer is raised at once, with [`ApiError::retry_after`] set, for the caller to
/// decide: Relay's monthly limit answers with the seconds left until the 1st of next month,
/// and a request that blocked for two weeks would be worse than the error.
const MAX_AUTO_RETRY_AFTER_SECS: u64 = 60;

/// `429` codes that waiting a few seconds does not lift.
const NON_RETRYABLE_429_CODES: &[&str] = &["RELAY_MESSAGE_LIMIT_REACHED"];

pub(crate) struct HttpClient {
    api_key: String,
    base_url: String,
    client: reqwest::Client,
}

/// One request, described before it is sent so a `429` can send it again.
pub(crate) struct Call {
    method: Method,
    path: String,
    query: Vec<(&'static str, String)>,
    body: Option<Vec<u8>>,
    skip_auth: bool,
    session_token: Option<String>,
}

impl Call {
    pub fn new(method: Method, path: impl Into<String>) -> Self {
        Self {
            method,
            path: path.into(),
            query: Vec::new(),
            body: None,
            skip_auth: false,
            session_token: None,
        }
    }

    /// Add a query parameter; `None` is dropped.
    pub fn query(mut self, key: &'static str, value: Option<impl ToString>) -> Self {
        if let Some(value) = value {
            self.query.push((key, value.to_string()));
        }
        self
    }

    /// Set the JSON body.
    pub fn json<B: Serialize + ?Sized>(mut self, body: &B) -> Result<Self, Error> {
        self.body = Some(serde_json::to_vec(body)?);
        Ok(self)
    }

    /// Omit the `Authorization` header: the call is authenticated by something else.
    pub fn skip_auth(mut self) -> Self {
        self.skip_auth = true;
        self
    }

    /// Send an end user's session token as `X-Session-Token`.
    pub fn session_token(mut self, token: impl Into<String>) -> Self {
        self.session_token = Some(token.into());
        self
    }
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
            base_url: normalize_base_url(base_url),
            client,
        })
    }

    /// The base URL requests go to, without a trailing slash.
    #[cfg_attr(not(feature = "jwt"), allow(dead_code))]
    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    pub async fn get<T: DeserializeOwned>(&self, path: &str) -> Result<T, Error> {
        self.send(Call::new(Method::GET, path)).await
    }

    pub async fn post<B: Serialize, T: DeserializeOwned>(
        &self,
        path: &str,
        body: &B,
    ) -> Result<T, Error> {
        self.send(Call::new(Method::POST, path).json(body)?).await
    }

    pub async fn patch<B: Serialize, T: DeserializeOwned>(
        &self,
        path: &str,
        body: &B,
    ) -> Result<T, Error> {
        self.send(Call::new(Method::PATCH, path).json(body)?).await
    }

    /// Send a `PUT` request and deserialize the JSON response body.
    pub async fn put<B: Serialize, T: DeserializeOwned>(
        &self,
        path: &str,
        body: &B,
    ) -> Result<T, Error> {
        self.send(Call::new(Method::PUT, path).json(body)?).await
    }

    /// Send a `PUT` request, discarding the (successful) response body.
    ///
    /// Used by endpoints that reply with a bare `{"status":"ok"}` acknowledgement.
    pub async fn put_discard<B: Serialize>(&self, path: &str, body: &B) -> Result<(), Error> {
        self.send_empty(Call::new(Method::PUT, path).json(body)?)
            .await
    }

    /// Send a bodyless `POST` request, discarding the (successful) response body.
    ///
    /// Used by action endpoints that reply with `204 No Content`.
    pub async fn post_discard(&self, path: &str) -> Result<(), Error> {
        self.send_empty(Call::new(Method::POST, path)).await
    }

    pub async fn delete(&self, path: &str) -> Result<(), Error> {
        self.send_empty(Call::new(Method::DELETE, path)).await
    }

    /// Send a call and deserialize the JSON response body.
    pub async fn send<T: DeserializeOwned>(&self, call: Call) -> Result<T, Error> {
        let resp = self.execute(call).await?;
        let bytes = resp.bytes().await.map_err(Error::Http)?;
        Ok(serde_json::from_slice(&bytes)?)
    }

    /// Send a call and check its status without reading the body — for endpoints that
    /// return no meaningful content (`204`, `{"status":"ok"}`, …).
    pub async fn send_empty(&self, call: Call) -> Result<(), Error> {
        self.execute(call).await.map(drop)
    }

    /// Send a call; a non-2xx answer becomes [`Error::Api`]. A `POST` that hits a short
    /// `429` is sent once more after the wait.
    async fn execute(&self, call: Call) -> Result<reqwest::Response, Error> {
        let mut attempt = 1;
        loop {
            let resp = self.build(&call).send().await.map_err(Error::Http)?;
            let status = resp.status();
            if status.is_success() {
                return Ok(resp);
            }

            let retry_after = parse_retry_after(
                resp.headers()
                    .get("Retry-After")
                    .and_then(|v| v.to_str().ok()),
            );
            let bytes = resp.bytes().await.map_err(Error::Http)?;
            let error = ApiError::from_response(status.as_u16(), &bytes, retry_after);

            // One retry of a POST on a short 429 (a per-minute burst ceiling). Never on a
            // limit that lifts at the end of the month, and never on a 402.
            let wait = retry_after.unwrap_or(1);
            if status.as_u16() == 429
                && attempt == 1
                && call.method == Method::POST
                && wait <= MAX_AUTO_RETRY_AFTER_SECS
                && !NON_RETRYABLE_429_CODES.contains(&error.code.as_str())
            {
                tokio::time::sleep(Duration::from_secs(wait)).await;
                attempt = 2;
                continue;
            }
            return Err(error.into());
        }
    }

    fn build(&self, call: &Call) -> reqwest::RequestBuilder {
        let mut url = format!("{}{}", self.base_url, call.path);
        if !call.query.is_empty() {
            url.push(if call.path.contains('?') { '&' } else { '?' });
            url.push_str(&encode_query(&call.query));
        }

        let mut req = self
            .client
            .request(call.method.clone(), &url)
            .header("Content-Type", "application/json")
            .header("Accept", "application/json");

        if !call.skip_auth {
            req = req.header("Authorization", format!("Bearer {}", self.api_key));
        }
        if let Some(token) = &call.session_token {
            req = req.header("X-Session-Token", token);
        }
        if let Some(body) = &call.body {
            req = req.body(body.clone());
        }
        req
    }
}

pub(crate) fn normalize_base_url(base_url: Option<String>) -> String {
    base_url
        .unwrap_or_else(|| API_URL.into())
        .trim_end_matches('/')
        .to_string()
}

/// Seconds from a `Retry-After` header given as delta-seconds (fractions round up).
///
/// An HTTP-date is not understood and reads as absent; the gateway sends seconds.
fn parse_retry_after(raw: Option<&str>) -> Option<u64> {
    let seconds: f64 = raw?.trim().parse().ok()?;
    (seconds.is_finite() && seconds >= 0.0).then(|| seconds.ceil() as u64)
}

/// `a=1&b=2` with both sides percent-encoded.
fn encode_query(pairs: &[(&'static str, String)]) -> String {
    pairs
        .iter()
        .map(|(k, v)| format!("{}={}", urlencode(k), urlencode(v)))
        .collect::<Vec<_>>()
        .join("&")
}

/// Percent-encode a path segment or query value, escaping every character that is not an
/// [RFC 3986](https://datatracker.ietf.org/doc/html/rfc3986) unreserved character.
///
/// Ids come from callers and must not be able to reshape a path.
pub(crate) fn urlencode(value: &str) -> String {
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
