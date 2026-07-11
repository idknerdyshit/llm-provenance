//! Provider-neutral typed envelopes for intent classification.

use serde::{Deserialize, Serialize};

use crate::{ContextDigest, GenerationProvenance, VersionedPrompt};
#[cfg(feature = "tracing")]
use crate::{Error, Result};

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

#[cfg(feature = "tracing")]
impl<T: Serialize> IntentRequest<T> {
    /// Emit a redacted structured tracing event for this request envelope.
    ///
    /// Intent envelopes are inert data structures, so tracing is explicit and
    /// does not run during construction or serde serialization.
    pub fn emit_trace(&self) -> Result<()> {
        self.emit_trace_with_options(crate::trace::TraceOptions::default())
    }

    /// Emit a structured tracing event using the requested tracing mode.
    pub fn emit_trace_with_options(&self, options: crate::trace::TraceOptions) -> Result<()> {
        let digest_hex = self.context.hex();
        let metadata = crate::trace::EventMetadata {
            schema_version: Some(self.context.schema_version().get()),
            digest_hex: Some(&digest_hex),
            prompt_version: Some(self.prompt.version()),
            model_present: Some(!self.model.is_empty()),
            ..Default::default()
        };
        let trace = crate::trace::OperationTrace::new(
            "intent.request.emit",
            Some(self.context.schema_version().get()),
            options.is_sensitive(),
        );
        let _entered = trace.enter();
        let raw_json = if options.is_sensitive() {
            match serde_json::to_string(self) {
                Ok(raw_json) => Some(raw_json),
                Err(error) => {
                    let error = Error::Serialization(error.to_string());
                    trace.failure(&metadata, &error);
                    return Err(error);
                }
            }
        } else {
            None
        };
        trace.success(&metadata, raw_json.as_deref());
        Ok(())
    }

    /// Emit this request envelope to the sensitive tracing target.
    ///
    /// This is intended only for local protocol debugging.
    #[cfg(feature = "sensitive-diagnostics")]
    pub fn emit_sensitive_trace(&self) -> Result<()> {
        self.emit_trace_with_options(crate::trace::TraceOptions::new().with_sensitive_tracing())
    }
}

#[cfg(feature = "tracing")]
impl<T: Serialize> IntentResponse<T> {
    /// Emit a redacted structured tracing event for this response envelope.
    ///
    /// Intent envelopes are inert data structures, so tracing is explicit and
    /// does not run during construction or serde serialization.
    pub fn emit_trace(&self) -> Result<()> {
        self.emit_trace_with_options(crate::trace::TraceOptions::default())
    }

    /// Emit a structured tracing event using the requested tracing mode.
    pub fn emit_trace_with_options(&self, options: crate::trace::TraceOptions) -> Result<()> {
        let context = self.provenance.context();
        let usage = self.provenance.usage();
        let digest_hex = context.hex();
        let metadata = crate::trace::EventMetadata {
            schema_version: Some(context.schema_version().get()),
            digest_hex: Some(&digest_hex),
            prompt_version: Some(self.provenance.prompt().identity().version()),
            context_changed: Some(self.provenance.context_changed()),
            cache_hit: Some(self.provenance.cache().hit()),
            input_tokens: usage.input_tokens,
            output_tokens: usage.output_tokens,
            model_present: Some(true),
            provider_generation_id_present: Some(
                self.provenance.provider_generation_id().is_some(),
            ),
            estimated_cost_present: Some(self.provenance.estimated_cost().is_some()),
        };
        let trace = crate::trace::OperationTrace::new(
            "intent.response.emit",
            Some(context.schema_version().get()),
            options.is_sensitive(),
        );
        let _entered = trace.enter();
        let raw_json = if options.is_sensitive() {
            match serde_json::to_string(self) {
                Ok(raw_json) => Some(raw_json),
                Err(error) => {
                    let error = Error::Serialization(error.to_string());
                    trace.failure(&metadata, &error);
                    return Err(error);
                }
            }
        } else {
            None
        };
        trace.success(&metadata, raw_json.as_deref());
        Ok(())
    }

    /// Emit this response envelope to the sensitive tracing target.
    ///
    /// This is intended only for local protocol debugging.
    #[cfg(feature = "sensitive-diagnostics")]
    pub fn emit_sensitive_trace(&self) -> Result<()> {
        self.emit_trace_with_options(crate::trace::TraceOptions::new().with_sensitive_tracing())
    }
}
