//! Official Rust SDK for the Verne Nautilus platform.
//!
//! Provides two services behind a single unified client:
//!
//! - **[Relay]** — Webhooks-as-a-Service: deliver events to subscribed HTTP endpoints.
//! - **[Gate]** — Auth-as-a-Service: manage identities, issue short-lived access tokens,
//!   and enforce authorization policies.
//! - **[Passepartout]** — Telegram Auth-as-a-Service: sign users in with Telegram and
//!   introspect the access tokens it issues.
//!
//! # Quick start
//!
//! Add the crate to your `Cargo.toml`:
//!
//! ```toml
//! [dependencies]
//! nautilus-rs = "1.2"
//! tokio = { version = "1", features = ["full"] }
//! ```
//!
//! ## Using both services together
//!
//! ```no_run
//! use nautilus_rs::{Verne, SendMessageParams};
//! use serde_json::json;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), nautilus_rs::Error> {
//!     let verne = Verne::builder()
//!         .relay("vrn_relay_live_sk_…")
//!         .gate("vrn_gate_live_sk_…")
//!         .build()?;
//!
//!     // Send a webhook event
//!     let msg = verne.relay()?.messages().send(SendMessageParams {
//!         event_type: "order.placed".into(),
//!         payload: json!({ "order_id": "ord_123" }),
//!         ..Default::default()
//!     }).await?;
//!     println!("sent: {}", msg.id);
//!
//!     Ok(())
//! }
//! ```
//!
//! ## Using a single service
//!
//! You can also instantiate [`Relay`] or [`Gate`] on their own:
//!
//! ```no_run
//! use nautilus_rs::Relay;
//!
//! let relay = Relay::new("vrn_relay_live_sk_…");
//! ```
//!
//! # Error handling
//!
//! Every fallible call returns `Result<T, `[`Error`]`>`. Match on the variants to
//! handle specific failure modes:
//!
//! ```no_run
//! use nautilus_rs::{Error, Relay, SendMessageParams};
//! use serde_json::json;
//!
//! # async fn run() {
//! let relay = Relay::new("vrn_relay_live_sk_…");
//! match relay.messages().send(SendMessageParams {
//!     event_type: "ping".into(),
//!     payload: json!({}),
//!     ..Default::default()
//! }).await {
//!     Ok(msg) => println!("delivered: {}", msg.id),
//!     Err(Error::Api(e)) => eprintln!("API {}: {}", e.status, e.message),
//!     Err(e) => eprintln!("unexpected: {e}"),
//! }
//! # }
//! ```
//!
//! # API key format
//!
//! Keys follow the pattern `vrn_<service>_<env>_sk_…`, for example:
//! - `vrn_relay_live_sk_…`
//! - `vrn_gate_test_sk_…`

mod client;
mod error;
mod http;
mod resources;
mod types;

pub use client::{Verne, VerneBuilder};
pub use error::{ApiError, Error};
pub use types::Paginated;

pub use resources::relay::types::{ListMessagesParams, Message, SendMessageParams};
pub use resources::relay::{MessagesClient, Relay, RelayBuilder};

pub use resources::gate::types::{
    AccessToken, AuthorizationDecision, AuthorizeParams, CreateIdentityParams, CreateTokenParams,
    Identity, IdentityTraits, IdentityTraitsInput, JsonPatchOp, OidcProvider, SecuritySettings,
    TokenInfo,
};
pub use resources::gate::{Gate, GateBuilder, IdentitiesClient, SettingsClient, TokensClient};

pub use resources::passepartout::types::{
    LoginStart, LoginStatus, TelegramUser, TokenIntrospection,
};
pub use resources::passepartout::{Passepartout, PassepartoutBuilder};
