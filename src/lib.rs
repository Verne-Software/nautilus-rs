//! Official Rust SDK for the Verne Nautilus platform.
//!
//! Provides access to two services:
//! - **Relay**: Webhooks-as-a-Service
//! - **Gate**: Auth-as-a-Service (identities, tokens, authorization)

mod client;
mod error;
mod http;
mod resources;
mod types;

pub use client::{Verne, VerneBuilder};
pub use error::{ApiError, Error};
pub use types::Paginated;

pub use resources::relay::types::{ListMessagesParams, Message, SendMessageParams};
pub use resources::relay::Relay;

pub use resources::gate::types::{
    AccessToken, AuthorizationDecision, AuthorizeParams, CreateIdentityParams, CreateTokenParams,
    Identity, IdentityTraits, IdentityTraitsInput, JsonPatchOp, TokenInfo,
};
pub use resources::gate::Gate;
