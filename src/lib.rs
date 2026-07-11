#![doc = include_str!("../README.md")]

mod context;
mod digest;
mod error;
mod intent;
mod provenance;
#[cfg(feature = "tracing")]
mod trace;
mod validate;

pub use context::{CONTEXT_DIGEST_DOMAIN, Context, DynamicContext, SchemaId, SchemaVersion};
pub use digest::{Canonicalization, ContextDigest, HashAlgorithm};
pub use error::{Error, Result};
pub use intent::{DynamicIntentRequest, DynamicIntentResponse, IntentRequest, IntentResponse};
pub use provenance::{
    CacheProvenance, GenerationProvenance, MonetaryCost, TokenUsage, VersionedPrompt,
};
#[cfg(feature = "tracing")]
pub use trace::TraceOptions;
