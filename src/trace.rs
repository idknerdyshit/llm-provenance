//! Structured tracing helpers for provenance operations.

use std::time::{Duration, Instant};

use crate::Error;

pub(crate) const TRACE_TARGET: &str = "llm_provenance::trace";
#[cfg(feature = "sensitive-diagnostics")]
pub(crate) const SENSITIVE_TRACE_TARGET: &str = "llm_provenance::sensitive";
#[cfg(feature = "sensitive-diagnostics")]
const SENSITIVE_UNAVAILABLE: &str = "<unavailable>";

/// Controls whether a provenance operation emits only safe metadata or also
/// emits raw serialized values to the sensitive tracing target.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TraceOptions {
    sensitive: bool,
}

impl TraceOptions {
    /// Create redacted tracing options.
    #[must_use]
    pub const fn new() -> Self {
        Self { sensitive: false }
    }

    /// Create redacted tracing options explicitly.
    #[must_use]
    pub const fn redacted() -> Self {
        Self::new()
    }

    /// Enable intentionally sensitive tracing for this operation.
    ///
    /// This method is available only with the `sensitive-diagnostics` feature.
    /// It emits raw serialized values to the dedicated sensitive tracing
    /// target and is intended only for local protocol debugging.
    #[cfg(feature = "sensitive-diagnostics")]
    #[must_use]
    pub const fn with_sensitive_tracing(mut self) -> Self {
        self.sensitive = true;
        self
    }

    /// Return whether this operation is configured for sensitive tracing.
    #[must_use]
    pub const fn is_sensitive(self) -> bool {
        self.sensitive
    }
}

/// Safe, structured metadata shared by all operation events.
#[derive(Default)]
pub(crate) struct EventMetadata<'a> {
    pub(crate) schema_version: Option<u32>,
    pub(crate) digest_hex: Option<&'a str>,
    pub(crate) prompt_version: Option<u32>,
    pub(crate) context_changed: Option<bool>,
    pub(crate) cache_hit: Option<bool>,
    pub(crate) input_tokens: Option<u64>,
    pub(crate) output_tokens: Option<u64>,
    pub(crate) model_present: Option<bool>,
    pub(crate) provider_generation_id_present: Option<bool>,
    pub(crate) estimated_cost_present: Option<bool>,
}

pub(crate) struct OperationTrace {
    operation: &'static str,
    span: tracing::Span,
    start: Instant,
    #[cfg(feature = "sensitive-diagnostics")]
    sensitive: bool,
}

impl OperationTrace {
    pub(crate) fn new(
        operation: &'static str,
        schema_version: Option<u32>,
        sensitive: bool,
    ) -> Self {
        let span = match schema_version {
            Some(schema_version) => tracing::info_span!(
                target: TRACE_TARGET,
                "llm_provenance.operation",
                operation,
                schema_version,
                elapsed_us = tracing::field::Empty,
            ),
            None => tracing::info_span!(
                target: TRACE_TARGET,
                "llm_provenance.operation",
                operation,
                elapsed_us = tracing::field::Empty,
            ),
        };
        #[cfg(not(feature = "sensitive-diagnostics"))]
        let _ = sensitive;
        Self {
            operation,
            span,
            start: Instant::now(),
            #[cfg(feature = "sensitive-diagnostics")]
            sensitive,
        }
    }

    pub(crate) fn enter(&self) -> tracing::span::Entered<'_> {
        self.span.enter()
    }

    pub(crate) fn success(&self, metadata: &EventMetadata<'_>, raw_json: Option<&str>) {
        let elapsed_us = duration_us(self.start.elapsed());
        self.span.record("elapsed_us", elapsed_us);
        tracing::debug!(
            target: TRACE_TARGET,
            event = "llm_provenance.operation.success",
            operation = self.operation,
            elapsed_us,
            algorithm = "sha256",
            canonicalization = "rfc8785",
            schema_version = ?metadata.schema_version,
            digest_hex = ?metadata.digest_hex,
            prompt_version = ?metadata.prompt_version,
            context_changed = ?metadata.context_changed,
            cache_hit = ?metadata.cache_hit,
            input_tokens = ?metadata.input_tokens,
            output_tokens = ?metadata.output_tokens,
            model_present = ?metadata.model_present,
            provider_generation_id_present = ?metadata.provider_generation_id_present,
            estimated_cost_present = ?metadata.estimated_cost_present,
        );
        #[cfg(feature = "sensitive-diagnostics")]
        if self.sensitive {
            tracing::debug!(
                target: SENSITIVE_TRACE_TARGET,
                event = "llm_provenance.sensitive.operation.success",
                operation = self.operation,
                elapsed_us,
                algorithm = "sha256",
                canonicalization = "rfc8785",
                schema_version = ?metadata.schema_version,
                digest_hex = ?metadata.digest_hex,
                prompt_version = ?metadata.prompt_version,
                context_changed = ?metadata.context_changed,
                cache_hit = ?metadata.cache_hit,
                input_tokens = ?metadata.input_tokens,
                output_tokens = ?metadata.output_tokens,
                model_present = ?metadata.model_present,
                provider_generation_id_present = ?metadata.provider_generation_id_present,
                estimated_cost_present = ?metadata.estimated_cost_present,
                raw_json = raw_json.unwrap_or(SENSITIVE_UNAVAILABLE),
            );
        }
        #[cfg(not(feature = "sensitive-diagnostics"))]
        let _ = raw_json;
    }

    pub(crate) fn failure(&self, metadata: &EventMetadata<'_>, error: &Error) {
        let elapsed_us = duration_us(self.start.elapsed());
        self.span.record("elapsed_us", elapsed_us);
        tracing::warn!(
            target: TRACE_TARGET,
            event = "llm_provenance.operation.failure",
            operation = self.operation,
            elapsed_us,
            error_kind = error_kind(error),
            algorithm = "sha256",
            canonicalization = "rfc8785",
            schema_version = ?metadata.schema_version,
            digest_hex = ?metadata.digest_hex,
            prompt_version = ?metadata.prompt_version,
            context_changed = ?metadata.context_changed,
            cache_hit = ?metadata.cache_hit,
            input_tokens = ?metadata.input_tokens,
            output_tokens = ?metadata.output_tokens,
            model_present = ?metadata.model_present,
            provider_generation_id_present = ?metadata.provider_generation_id_present,
            estimated_cost_present = ?metadata.estimated_cost_present,
        );
        #[cfg(feature = "sensitive-diagnostics")]
        if self.sensitive {
            tracing::warn!(
                target: SENSITIVE_TRACE_TARGET,
                event = "llm_provenance.sensitive.operation.failure",
                operation = self.operation,
                elapsed_us,
                error_kind = error_kind(error),
                error = %error,
                algorithm = "sha256",
                canonicalization = "rfc8785",
                schema_version = ?metadata.schema_version,
                digest_hex = ?metadata.digest_hex,
                prompt_version = ?metadata.prompt_version,
                context_changed = ?metadata.context_changed,
                cache_hit = ?metadata.cache_hit,
                input_tokens = ?metadata.input_tokens,
                output_tokens = ?metadata.output_tokens,
                model_present = ?metadata.model_present,
                provider_generation_id_present = ?metadata.provider_generation_id_present,
                estimated_cost_present = ?metadata.estimated_cost_present,
            );
        }
    }
}

pub(crate) fn duration_us(duration: Duration) -> u64 {
    u64::try_from(duration.as_micros()).unwrap_or(u64::MAX)
}

pub(crate) fn error_kind(error: &Error) -> &'static str {
    match error {
        Error::InvalidSchemaId(_) => "invalid_schema_id",
        Error::InvalidSchemaVersion => "invalid_schema_version",
        Error::InvalidIJsonNumber(_) => "invalid_i_json_number",
        Error::Serialization(_) => "serialization",
        Error::InvalidDigest(_) => "invalid_digest",
        Error::InvalidPrompt(_) => "invalid_prompt",
        Error::InvalidCost(_) => "invalid_cost",
    }
}
