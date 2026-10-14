#![allow(dead_code)]

use mockito::{Server, ServerGuard};
use nautilus_rs::Gate;

/// The shape of key the docs use: `vrn_…`, no engine, no environment.
pub const KEY: &str = "vrn_9f8a7c1b2d3e4f50";

pub const ORG: &str = "550e8400-e29b-41d4-a716-446655440000";
pub const USER: &str = "f47ac10b-58cc-4372-a567-0e02b2c3d479";
pub const ROLE: &str = "8e4f0b2a-3c1d-4e5f-9a6b-7c8d9e0f1a2b";

/// A Gate client talking to a fresh mock server.
pub async fn gate() -> (ServerGuard, Gate) {
    let server = Server::new_async().await;
    let gate = Gate::builder()
        .api_key(KEY)
        .base_url(server.url())
        .build()
        .unwrap();
    (server, gate)
}

pub const JSON: (&str, &str) = ("content-type", "application/json");
