//! Official Rust SDK for the Verne Nautilus platform.
//!
//! Provides four services behind a single unified client:
//!
//! - **[Relay]** — Webhooks-as-a-Service: deliver events to subscribed HTTP endpoints.
//! - **[Gate]** — Auth-as-a-Service: end users, organizations with roles and members,
//!   short-lived access tokens and signed tokens (JWT), sessions, an audit log, automatic
//!   deletion rules, and authorization checks.
//! - **[Passepartout]** — Telegram Auth-as-a-Service: sign users in with Telegram and
//!   introspect the access tokens it issues.
//! - **[Clockwork]** — Cron-as-a-Service: schedule recurring cron jobs and one-off delayed
//!   jobs that invoke your HTTP endpoints, and inspect their execution history.
//!
//! # Quick start
//!
//! Add the crate to your `Cargo.toml`:
//!
//! ```toml
//! [dependencies]
//! nautilus-rs = "1.4"
//! tokio = { version = "1", features = ["full"] }
//! ```
//!
//! Create a key on Dashboard → Keys, choosing the scopes it needs, and pass it once. The
//! crate is `nautilus-rs`; in code it is `nautilus_rs`.
//!
//! ```no_run
//! use nautilus_rs::{Verne, SendMessageParams};
//! use serde_json::json;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), nautilus_rs::Error> {
//!     let verne = Verne::builder()
//!         .api_key(std::env::var("VERNE_API_KEY").expect("VERNE_API_KEY"))
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
//! You can also instantiate [`Relay`], [`Gate`], [`Passepartout`] or [`Clockwork`] on their
//! own:
//!
//! ```no_run
//! use nautilus_rs::Relay;
//!
//! let relay = Relay::new("vrn_9f8a7c1b2d3e4f50");
//! ```
//!
//! # API keys
//!
//! A key is `vrn_…` and carries a set of scopes (`relay`, `gate`, `clockwork`,
//! `passepartout`). One key can serve every service; per-service keys
//! ([`VerneBuilder::relay`] and friends) are for using different keys per service and
//! **override** [`VerneBuilder::api_key`] for that service. Older `vrn_<engine>_…` keys
//! still work and open that one engine. The SDK does not check scopes: a service the key
//! has no scope for is `403 forbidden` from the API. There is no test/live environment
//! and no sandbox — use a separate key per environment.
//!
//! # Error handling
//!
//! Every fallible call returns `Result<T, `[`Error`]`>`. Match on the variants to
//! handle specific failure modes, and on [`ApiError::status`] and [`ApiError::code`]
//! inside [`Error::Api`]:
//!
//! ```no_run
//! use nautilus_rs::{Error, Relay, SendMessageParams};
//! use serde_json::json;
//!
//! # async fn run() {
//! let relay = Relay::new("vrn_9f8a7c1b2d3e4f50");
//! match relay.messages().send(SendMessageParams {
//!     event_type: "ping".into(),
//!     payload: json!({}),
//!     ..Default::default()
//! }).await {
//!     Ok(msg) => println!("accepted: {}", msg.id),
//!     Err(Error::Api(e)) if e.status == 429 && e.code == "RELAY_MESSAGE_LIMIT_REACHED" => {
//!         // the monthly limit; e.retry_after is the seconds to the 1st of next month
//!         eprintln!("limit reached: {:?}", e.details);
//!     }
//!     Err(Error::Api(e)) => eprintln!("API {} {}: {}", e.status, e.code, e.message),
//!     Err(e) => eprintln!("unexpected: {e}"),
//! }
//! # }
//! ```
//!
//! # Features
//!
//! - `jwt` (off by default): `GateTokenVerifier` and `TokensClient::verify_signed` —
//!   offline ES256 verification of Gate's signed tokens. Minting a signed token
//!   ([`TokensClient::create_signed`]) needs no feature.

mod client;
mod error;
mod http;
mod resources;
mod types;

pub use client::{Verne, VerneBuilder};
pub use error::{ApiError, Error, TokenError, TokenErrorReason};
pub use types::{ListParams, Paginated};

pub use resources::relay::types::{ListMessagesParams, Message, SendMessageParams};
pub use resources::relay::{MessagesClient, Relay, RelayBuilder};

pub use resources::gate::types::{
    AccessToken, AddMemberParams, AuditActor, AuditLogEntry, AuditTarget, AuthenticationMethod,
    AuthorizationDecision, AuthorizeParams, AuthorizeReason, CreateIdentityParams,
    CreateOrganizationParams, CreateRoleParams, CreateTokenParams, Identity, IdentityTraits,
    IdentityTraitsInput, JsonPatchOp, LifecycleAction, LifecycleRule, LifecycleRuleKind,
    LifecycleRules, ListAuditLogParams, ListLifecycleActionsParams, LoginFlowParams, Member,
    OidcExchangeResult, OidcProvider, OrgClaim, Organization, RecoveryAddress,
    RegistrationFlowParams, Role, RulePreview, SecuritySettings, Session, SessionCheck,
    SessionDevice, SetRuleParams, SignedToken, SignedTokenClaims, TokenInfo, UpdateMemberParams,
    UpdateOrganizationParams, UpdateRoleParams, VerifiableAddress,
};
pub use resources::gate::{
    AuditLogClient, Gate, GateBuilder, IdentitiesClient, LifecycleClient, MembersClient,
    OrganizationsClient, RolesClient, SessionsClient, SettingsClient, TokensClient,
};
#[cfg(feature = "jwt")]
pub use resources::gate::{GateTokenVerifier, GateTokenVerifierBuilder};

pub use resources::passepartout::types::{
    LoginStart, LoginStatus, TelegramUser, TokenIntrospection,
};
pub use resources::passepartout::{Passepartout, PassepartoutBuilder};

pub use resources::clockwork::types::{
    ClockworkUsage, CreateCronJobParams, CreateDelayedJobParams, CronJob, CronUsage, DelayedJob,
    DelayedUsage, Execution, UpdateCronJobParams,
};
pub use resources::clockwork::{Clockwork, ClockworkBuilder, CronJobsClient, DelayedJobsClient};
