//! Typed context envelopes and RFC 8785 canonicalization.

use std::num::NonZeroU32;

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};

use crate::digest::ContextDigest;
use crate::validate::to_i_json_value;
use crate::{Error, Result};

/// Domain separator included in every context hash preimage.
///
/// Changing this value changes every digest and therefore requires a new major
/// version plus an explicit migration strategy.
pub const CONTEXT_DIGEST_DOMAIN: &str = "llm-provenance/context-digest/v1";

/// Stable identifier for an application's context schema.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(transparent)]
pub struct SchemaId(String);

impl SchemaId {
    /// Validate and construct a schema identifier.
    ///
    /// Identifiers are 1-128 ASCII characters and may contain letters, digits,
    /// `.`, `_`, `/`, and `-`. Colons are excluded because they delimit the
    /// textual [`ContextDigest`] representation.
    pub fn new(value: impl Into<String>) -> Result<Self> {
        let value = value.into();
        let valid = !value.is_empty()
            && value.len() <= 128
            && value.bytes().all(|byte| {
                byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'/' | b'-')
            });
        if valid {
            Ok(Self(value))
        } else {
            Err(Error::InvalidSchemaId(value))
        }
    }

    /// Borrow the validated identifier.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Debug for SchemaId {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_tuple("SchemaId").field(&self.0).finish()
    }
}

impl std::fmt::Display for SchemaId {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::str::FromStr for SchemaId {
    type Err = Error;
    fn from_str(value: &str) -> Result<Self> {
        Self::new(value)
    }
}

impl<'de> Deserialize<'de> for SchemaId {
    fn deserialize<D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> std::result::Result<Self, D::Error> {
        let value = String::deserialize(deserializer)?;
        Self::new(value).map_err(serde::de::Error::custom)
    }
}

/// One-based version of an application context schema.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(transparent)]
pub struct SchemaVersion(NonZeroU32);

impl SchemaVersion {
    /// Construct a non-zero schema version.
    pub fn new(value: u32) -> Result<Self> {
        NonZeroU32::new(value)
            .map(Self)
            .ok_or(Error::InvalidSchemaVersion)
    }

    /// Return the numeric schema version.
    pub fn get(self) -> u32 {
        self.0.get()
    }
}

impl<'de> Deserialize<'de> for SchemaVersion {
    fn deserialize<D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> std::result::Result<Self, D::Error> {
        let value = u32::deserialize(deserializer)?;
        Self::new(value).map_err(serde::de::Error::custom)
    }
}

/// Versioned, typed input context for an LLM operation.
///
/// This type intentionally omits `Debug`: payloads commonly contain personal or
/// commercially sensitive data and should not enter logs accidentally.
#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub struct Context<T> {
    schema: SchemaId,
    schema_version: SchemaVersion,
    payload: T,
}

/// Context whose application payload is dynamically typed JSON.
pub type DynamicContext = Context<serde_json::Value>;

/// Redacted result of comparing a rebuilt context with an expected digest.
///
/// This type intentionally contains only self-describing digest references.
/// It never contains application payload values or canonical preimage bytes.
#[derive(Clone, PartialEq, Eq)]
pub enum ContextVerification {
    /// The rebuilt context exactly matches the expected digest.
    Match,
    /// The rebuilt context is valid but does not match the expected digest.
    Mismatch {
        /// Digest supplied by the persisted audit record.
        expected: ContextDigest,
        /// Digest computed from the rebuilt context.
        actual: ContextDigest,
    },
}

impl ContextVerification {
    /// Return whether verification succeeded.
    #[must_use]
    pub fn is_match(&self) -> bool {
        matches!(self, Self::Match)
    }
}

impl<T> Context<T> {
    /// Construct a context from already validated schema metadata.
    pub fn new(schema: SchemaId, schema_version: SchemaVersion, payload: T) -> Self {
        Self {
            schema,
            schema_version,
            payload,
        }
    }

    /// Return the application schema identifier.
    pub fn schema(&self) -> &SchemaId {
        &self.schema
    }

    /// Return the application schema version.
    pub fn schema_version(&self) -> SchemaVersion {
        self.schema_version
    }

    /// Borrow the application payload.
    pub fn payload(&self) -> &T {
        &self.payload
    }

    /// Consume the envelope and return its application payload.
    pub fn into_payload(self) -> T {
        self.payload
    }
}

impl<T: Serialize> Context<T> {
    /// Return the exact RFC 8785 bytes used as the SHA-256 preimage.
    pub fn canonical_bytes(&self) -> Result<Vec<u8>> {
        #[cfg(feature = "tracing")]
        {
            self.canonical_bytes_with_options(crate::trace::TraceOptions::default())
        }
        #[cfg(not(feature = "tracing"))]
        {
            self.canonical_bytes_impl()
        }
    }

    /// Return the canonical bytes while applying the requested tracing mode.
    #[cfg(feature = "tracing")]
    pub fn canonical_bytes_with_options(
        &self,
        options: crate::trace::TraceOptions,
    ) -> Result<Vec<u8>> {
        let trace = crate::trace::OperationTrace::new(
            "context.canonical_bytes",
            Some(self.schema_version.get()),
            options.is_sensitive(),
        );
        let metadata = crate::trace::EventMetadata {
            schema_version: Some(self.schema_version.get()),
            ..Default::default()
        };
        let _entered = trace.enter();
        match self.canonical_bytes_impl() {
            Ok(bytes) => {
                let raw_json = options
                    .is_sensitive()
                    .then(|| String::from_utf8_lossy(&bytes).into_owned());
                trace.success(&metadata, raw_json.as_deref());
                Ok(bytes)
            }
            Err(error) => {
                trace.failure(&metadata, &error);
                Err(error)
            }
        }
    }

    /// Return the canonical bytes and emit them to the sensitive tracing
    /// target. This is intended only for local protocol debugging.
    #[cfg(feature = "sensitive-diagnostics")]
    pub fn canonical_bytes_with_sensitive_tracing(&self) -> Result<Vec<u8>> {
        self.canonical_bytes_with_options(
            crate::trace::TraceOptions::new().with_sensitive_tracing(),
        )
    }

    fn canonical_bytes_impl(&self) -> Result<Vec<u8>> {
        let envelope = DigestEnvelope {
            domain: CONTEXT_DIGEST_DOMAIN,
            schema: self.schema.as_str(),
            schema_version: self.schema_version.get(),
            payload: &self.payload,
        };
        let value = to_i_json_value(&envelope)?;
        serde_json_canonicalizer::to_vec(&value)
            .map_err(|error| Error::Serialization(error.to_string()))
    }

    /// Compute a structured SHA-256 digest over the canonical context envelope.
    pub fn digest(&self) -> Result<ContextDigest> {
        #[cfg(feature = "tracing")]
        {
            self.digest_with_options(crate::trace::TraceOptions::default())
        }
        #[cfg(not(feature = "tracing"))]
        {
            self.digest_impl()
        }
    }

    /// Recompute this context digest and compare it with persisted evidence.
    ///
    /// The returned result is intentionally redacted: it reports only digest
    /// references and never includes payload values or canonical bytes.
    pub fn verify_digest(&self, expected: &ContextDigest) -> Result<ContextVerification> {
        let actual = self.digest()?;
        Ok(verification(expected, actual))
    }

    /// Compute a structured SHA-256 digest while applying the requested
    /// tracing mode.
    #[cfg(feature = "tracing")]
    pub fn digest_with_options(
        &self,
        options: crate::trace::TraceOptions,
    ) -> Result<ContextDigest> {
        let trace = crate::trace::OperationTrace::new(
            "context.digest",
            Some(self.schema_version.get()),
            options.is_sensitive(),
        );
        let metadata = crate::trace::EventMetadata {
            schema_version: Some(self.schema_version.get()),
            ..Default::default()
        };
        let _entered = trace.enter();
        let result = self.canonical_bytes_impl().map(|bytes| {
            let raw_json = options
                .is_sensitive()
                .then(|| String::from_utf8_lossy(&bytes).into_owned());
            let digest: [u8; 32] = Sha256::digest(bytes).into();
            (digest, raw_json)
        });
        match result {
            Ok((digest, raw_json)) => {
                let digest_hex = hex::encode(digest);
                let metadata = crate::trace::EventMetadata {
                    digest_hex: Some(&digest_hex),
                    ..metadata
                };
                trace.success(&metadata, raw_json.as_deref());
                Ok(ContextDigest::from_parts(
                    self.schema.clone(),
                    self.schema_version,
                    digest,
                ))
            }
            Err(error) => {
                trace.failure(&metadata, &error);
                Err(error)
            }
        }
    }

    /// Compute a digest and emit the canonical context JSON to the sensitive
    /// tracing target. This is intended only for local protocol debugging.
    #[cfg(feature = "sensitive-diagnostics")]
    pub fn digest_with_sensitive_tracing(&self) -> Result<ContextDigest> {
        self.digest_with_options(crate::trace::TraceOptions::new().with_sensitive_tracing())
    }

    #[cfg(not(feature = "tracing"))]
    fn digest_impl(&self) -> Result<ContextDigest> {
        let bytes = self.canonical_bytes_impl()?;
        let digest: [u8; 32] = Sha256::digest(bytes).into();
        Ok(ContextDigest::from_parts(
            self.schema.clone(),
            self.schema_version,
            digest,
        ))
    }

    /// Convert a typed context into a dynamic JSON context.
    pub fn to_dynamic(&self) -> Result<DynamicContext> {
        #[cfg(feature = "tracing")]
        {
            self.to_dynamic_with_options(crate::trace::TraceOptions::default())
        }
        #[cfg(not(feature = "tracing"))]
        {
            self.to_dynamic_impl()
        }
    }

    /// Convert a typed context into a dynamic JSON context while applying the
    /// requested tracing mode.
    #[cfg(feature = "tracing")]
    pub fn to_dynamic_with_options(
        &self,
        options: crate::trace::TraceOptions,
    ) -> Result<DynamicContext> {
        let trace = crate::trace::OperationTrace::new(
            "context.to_dynamic",
            Some(self.schema_version.get()),
            options.is_sensitive(),
        );
        let metadata = crate::trace::EventMetadata {
            schema_version: Some(self.schema_version.get()),
            ..Default::default()
        };
        let _entered = trace.enter();
        match self.to_dynamic_payload() {
            Ok(payload) => {
                let raw_json = options
                    .is_sensitive()
                    .then(|| serde_json::to_string(&payload).ok())
                    .flatten();
                let context = Context::new(self.schema.clone(), self.schema_version, payload);
                trace.success(&metadata, raw_json.as_deref());
                Ok(context)
            }
            Err(error) => {
                trace.failure(&metadata, &error);
                Err(error)
            }
        }
    }

    /// Convert a context and emit its dynamic payload to the sensitive
    /// tracing target. This is intended only for local protocol debugging.
    #[cfg(feature = "sensitive-diagnostics")]
    pub fn to_dynamic_with_sensitive_tracing(&self) -> Result<DynamicContext> {
        self.to_dynamic_with_options(crate::trace::TraceOptions::new().with_sensitive_tracing())
    }

    #[cfg(not(feature = "tracing"))]
    fn to_dynamic_impl(&self) -> Result<DynamicContext> {
        let payload = self.to_dynamic_payload()?;
        Ok(Context::new(
            self.schema.clone(),
            self.schema_version,
            payload,
        ))
    }

    fn to_dynamic_payload(&self) -> Result<serde_json::Value> {
        let payload = to_i_json_value(&self.payload)?;
        Ok(payload)
    }
}

impl Context<serde_json::Value> {
    /// Deserialize a dynamic payload into an application-defined type while
    /// preserving the schema identifier and version.
    pub fn try_into_typed<T: DeserializeOwned>(self) -> Result<Context<T>> {
        #[cfg(feature = "tracing")]
        {
            self.try_into_typed_with_options(crate::trace::TraceOptions::default())
        }
        #[cfg(not(feature = "tracing"))]
        {
            self.try_into_typed_impl()
        }
    }

    /// Deserialize a dynamic context while applying the requested tracing
    /// mode.
    #[cfg(feature = "tracing")]
    pub fn try_into_typed_with_options<T: DeserializeOwned>(
        self,
        options: crate::trace::TraceOptions,
    ) -> Result<Context<T>> {
        let Context {
            schema,
            schema_version,
            payload,
        } = self;
        let trace = crate::trace::OperationTrace::new(
            "context.try_into_typed",
            Some(schema_version.get()),
            options.is_sensitive(),
        );
        let metadata = crate::trace::EventMetadata {
            schema_version: Some(schema_version.get()),
            ..Default::default()
        };
        let _entered = trace.enter();
        let raw_json = options
            .is_sensitive()
            .then(|| serde_json::to_string(&payload).ok())
            .flatten();
        let result = serde_json::from_value(payload)
            .map_err(|error| Error::Serialization(error.to_string()))
            .map(|payload| Context::new(schema, schema_version, payload));
        match result {
            Ok(context) => {
                trace.success(&metadata, raw_json.as_deref());
                Ok(context)
            }
            Err(error) => {
                trace.failure(&metadata, &error);
                Err(error)
            }
        }
    }

    /// Deserialize a dynamic context and emit its payload to the sensitive
    /// tracing target. This is intended only for local protocol debugging.
    #[cfg(feature = "sensitive-diagnostics")]
    pub fn try_into_typed_with_sensitive_tracing<T: DeserializeOwned>(self) -> Result<Context<T>> {
        self.try_into_typed_with_options(crate::trace::TraceOptions::new().with_sensitive_tracing())
    }

    #[cfg(not(feature = "tracing"))]
    fn try_into_typed_impl<T: DeserializeOwned>(self) -> Result<Context<T>> {
        let Context {
            schema,
            schema_version,
            payload,
        } = self;
        let payload = serde_json::from_value(payload)
            .map_err(|error| Error::Serialization(error.to_string()))?;
        Ok(Context::new(schema, schema_version, payload))
    }
}

#[derive(Serialize)]
struct DigestEnvelope<'a, T> {
    domain: &'static str,
    schema: &'a str,
    schema_version: u32,
    payload: &'a T,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DynamicDigestEnvelope {
    domain: String,
    schema: SchemaId,
    schema_version: SchemaVersion,
    payload: serde_json::Value,
}

pub(crate) fn verify_canonical_bytes(
    expected: &ContextDigest,
    bytes: &[u8],
) -> Result<ContextVerification> {
    let envelope: DynamicDigestEnvelope =
        serde_json::from_slice(bytes).map_err(|_| Error::InvalidCanonicalContext)?;
    if envelope.domain != CONTEXT_DIGEST_DOMAIN {
        return Err(Error::InvalidCanonicalContext);
    }

    let canonical_envelope = DigestEnvelope {
        domain: CONTEXT_DIGEST_DOMAIN,
        schema: envelope.schema.as_str(),
        schema_version: envelope.schema_version.get(),
        payload: &envelope.payload,
    };
    let value = to_i_json_value(&canonical_envelope).map_err(|_| Error::InvalidCanonicalContext)?;
    let canonical =
        serde_json_canonicalizer::to_vec(&value).map_err(|_| Error::InvalidCanonicalContext)?;
    if canonical != bytes {
        return Err(Error::InvalidCanonicalContext);
    }

    let digest: [u8; 32] = Sha256::digest(bytes).into();
    let actual = ContextDigest::from_parts(envelope.schema, envelope.schema_version, digest);
    Ok(verification(expected, actual))
}

fn verification(expected: &ContextDigest, actual: ContextDigest) -> ContextVerification {
    if expected == &actual {
        ContextVerification::Match
    } else {
        ContextVerification::Mismatch {
            expected: expected.clone(),
            actual,
        }
    }
}
