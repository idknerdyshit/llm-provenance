//! Provider-neutral typed envelopes for intent classification.

use serde::{Deserialize, Serialize};

use crate::{ContextDigest, GenerationProvenance, VersionedPrompt};

/// Typed request for an intent-classification operation.
///
/// This type intentionally omits `Debug` so classifier input cannot enter logs
/// through routine diagnostic formatting.
#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub struct IntentRequest<T> {
    /// Provider/model identifier requested for classification.
    pub model: String,
    /// Versioned classifier prompt identity; rendered prompts stay outside
    /// provenance and this request envelope.
    pub prompt: VersionedPrompt,
    /// Digest of the context associated with the classifier input.
    pub context: ContextDigest,
    /// Application-defined classifier input.
    pub input: T,
}

/// Typed response from an intent-classification operation.
///
/// Applications own the classification shape; this crate only couples it to
/// generation provenance.
#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub struct IntentResponse<T> {
    /// Application-defined classification result.
    pub classification: T,
    /// Provenance for the classifier generation.
    pub provenance: GenerationProvenance,
}

/// Dynamically typed JSON intent request.
pub type DynamicIntentRequest = IntentRequest<serde_json::Value>;

/// Dynamically typed JSON intent response.
pub type DynamicIntentResponse = IntentResponse<serde_json::Value>;
