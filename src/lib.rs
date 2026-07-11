#![doc = include_str!("../README.md")]

mod artifact;
mod context;
mod digest;
mod error;
mod intent;
mod manifest;
mod provenance;
#[cfg(feature = "tracing")]
mod trace;
mod validate;

pub use artifact::{
    ARTIFACT_DIGEST_DOMAIN, ArtifactDigest, ArtifactLocator, ArtifactReference, OpaqueIdentifier,
};
pub use context::{
    CONTEXT_DIGEST_DOMAIN, Context, ContextVerification, DynamicContext, SchemaId, SchemaVersion,
};
pub use digest::{Canonicalization, ContextDigest, HashAlgorithm};
pub use error::{Error, Result};
pub use intent::{DynamicIntentRequest, DynamicIntentResponse, IntentRequest, IntentResponse};
pub use manifest::{ContextManifest, ManifestedPayload, SourceSnapshot, VersionedComponent};
pub use provenance::{
    AuditTimestamp, CacheProvenance, ExecutionEvidence, GENERATION_PROVENANCE_FINGERPRINT_DOMAIN,
    GenerationProvenance, GenerationProvenanceBuilder, GenerationProvenanceFingerprint,
    ModelIdentity, MonetaryCost, PromptEvidence, RetainedGenerationArtifacts, TokenUsage,
    VersionedPrompt,
};
#[cfg(feature = "tracing")]
pub use trace::TraceOptions;
