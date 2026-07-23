# Verne Software Rust SDK

[![Crates.io](https://img.shields.io/crates/v/nautilus-rs.svg)](https://crates.io/crates/nautilus-rs)
[![docs.rs](https://img.shields.io/docsrs/nautilus-rs)](https://docs.rs/nautilus-rs)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.75%2B-orange.svg)](#)

The official Rust SDK for the [Verne Nautilus](https://vernesoft.com) platform.

> **Server-side only.** API keys carry full service access and must never be used in WASM or client-side contexts.

## Requirements

Rust 1.75 or later.

## Installation

```toml
[dependencies]
nautilus-rs = "1.1"
```

## Quick Start

```rust
use nautilus_rs::Verne;

#[tokio::main]
async fn main() -> Result<(), nautilus_rs::Error> {
    let verne = Verne::builder()
        .relay(std::env::var("VERNE_RELAY_KEY").unwrap())
        .gate(std::env::var("VERNE_GATE_KEY").unwrap())
        .build()?;

    Ok(())
}
```

You can also instantiate services independently if you only need one:

```rust
use nautilus_rs::{Relay, Gate};

let relay = Relay::new("vrn_relay_live_sk_...");
let gate  = Gate::new("vrn_gate_live_sk_...");
```

## Relay — Webhooks-as-a-Service

Send events to all subscribed endpoints:

```rust
verne.relay()?.messages().send(nautilus_rs::SendMessageParams {
    event_type: "user.created".into(),
    payload: serde_json::json!({ "id": "usr_123" }),
    ..Default::default()
}).await?;
```

Optional parameters:

```rust
verne.relay()?.messages().send(nautilus_rs::SendMessageParams {
    event_type: "order.placed".into(),
    payload: serde_json::json!({ "order_id": "999" }),
    idempotency_key: Some("evt_abc".into()), // prevent duplicate delivery within 24h
    channels: Some(vec!["team-a".into()]),   // restrict to specific endpoint channels
}).await?;
```

List previously sent events:

```rust
let page = verne.relay()?.messages().list(nautilus_rs::ListMessagesParams {
    limit: Some(20),
    event_type: Some("user.created".into()),
    cursor: None,
}).await?;

println!("{:?}", page.data);        // Vec<Message>
println!("{}", page.has_more);      // bool
println!("{:?}", page.next_cursor); // pass to the next call to paginate
```

## Gate — Auth-as-a-Service

### Identity Management

Manage your end-users. The `tenant_id` is automatically scoped to your API key.

```rust
// Create a user
let identity = verne.gate()?.identities().create(nautilus_rs::CreateIdentityParams {
    schema_id: "user".into(),
    traits: nautilus_rs::IdentityTraitsInput {
        email: "user@example.com".into(),
        custom_data: Some(serde_json::json!({ "role": "editor" })),
    },
    credentials: Some(serde_json::json!({
        "password": { "config": { "password": "StrongPassword123!" } }
    })),
    state: Some("active".into()),
}).await?;

// Get a user
verne.gate()?.identities().get(&identity.id).await?;

// Update a user (JSON Patch — RFC 6902)
verne.gate()?.identities().patch(&identity.id, vec![
    nautilus_rs::JsonPatchOp {
        op: "replace".into(),
        path: "/traits/custom_data/role".into(),
        value: Some(serde_json::json!("admin")),
        from: None,
    },
]).await?;

// Delete a user
verne.gate()?.identities().delete(&identity.id).await?;

// Activate / deactivate a user (an inactive user cannot log in)
verne.gate()?.identities().deactivate(&identity.id).await?;
verne.gate()?.identities().activate(&identity.id).await?;
// …or set the state explicitly:
verne.gate()?.identities().set_state(&identity.id, "inactive").await?;

// Resend the email verification link
verne.gate()?.identities().resend_verification(&identity.id).await?;
```

### Security Settings

Read or replace the tenant's security settings (passwordless login, TOTP MFA):

```rust
let security = verne.gate()?.settings().get_security().await?;
// security.passwordless_enabled, security.mfa_enabled

// Both fields are always sent — the update is a full replacement, not a merge.
verne.gate()?.settings().update_security(nautilus_rs::SecuritySettings {
    passwordless_enabled: true,
    mfa_enabled: false,
}).await?;
```

### Access Tokens

Exchange your long-lived API key for a short-lived access token:

```rust
let token = verne.gate()?.tokens().create(nautilus_rs::CreateTokenParams {
    subject: "usr_123".into(),
    scopes: Some(vec!["gate.tokens.read".into()]), // optional
    ttl_seconds: Some(3600),                       // optional, default 3600, max 86400
}).await?;

// token.access_token — attach to downstream requests
// token.expires_at   — ISO 8601 expiry
```

Validate a token:

```rust
let info = verne.gate()?.tokens().introspect(&token.access_token).await?;

if !info.active {
    // token is expired or invalid
}
```

### Authorization

Check whether a subject is allowed to perform an action:

```rust
let decision = verne.gate()?.authorize(nautilus_rs::AuthorizeParams {
    subject: "usr_123".into(),
    action: "relay.messages.read".into(),
    resource: "tenant:ten_001".into(),
    context: None,
}).await?;

if !decision.allowed {
    eprintln!("Forbidden: {}", decision.reason);
}
```

## Error Handling

All errors are returned as `nautilus_rs::Error`:

```rust
use nautilus_rs::Error;

match verne.relay()?.messages().send(params).await {
    Ok(msg) => println!("{}", msg.id),
    Err(Error::Api(e)) => {
        eprintln!("code: {}", e.code);       // e.g. "invalid_payload", "unauthorized"
        eprintln!("status: {}", e.status);   // HTTP status code
        eprintln!("request_id: {}", e.request_id); // include in support requests
    }
    Err(Error::Http(e)) => eprintln!("network error: {e}"),
    Err(Error::Config(msg)) => eprintln!("config error: {msg}"),
    Err(e) => eprintln!("error: {e}"),
}
```

## Configuration

Both `Verne` and the per-service builders accept an optional timeout (default 30 seconds):

```rust
let verne = Verne::builder()
    .relay(std::env::var("VERNE_RELAY_KEY").unwrap())
    .timeout_secs(10)
    .build()?;
```

## License

[MIT](LICENSE)
