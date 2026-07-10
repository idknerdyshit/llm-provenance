//! Typed context envelopes and RFC 8785 canonicalization.

use std::num::NonZeroU32;

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};

use crate::digest::ContextDigest;
use crate::validate::validate_i_json;
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
        let envelope = DigestEnvelope {
            domain: CONTEXT_DIGEST_DOMAIN,
            schema: self.schema.as_str(),
            schema_version: self.schema_version.get(),
            payload: &self.payload,
        };
        validate_i_json(&envelope)?;
        let value = serde_json::to_value(&envelope)
            .map_err(|error| Error::Serialization(error.to_string()))?;
        serde_json_canonicalizer::to_vec(&value)
            .map_err(|error| Error::Serialization(error.to_string()))
    }

    /// Compute a structured SHA-256 digest over the canonical context envelope.
    pub fn digest(&self) -> Result<ContextDigest> {
        let bytes = self.canonical_bytes()?;
        let digest: [u8; 32] = Sha256::digest(bytes).into();
        Ok(ContextDigest::from_parts(
            self.schema.clone(),
            self.schema_version,
            digest,
        ))
    }

    /// Convert a typed context into a dynamic JSON context.
    pub fn to_dynamic(&self) -> Result<DynamicContext> {
        validate_i_json(&self.payload)?;
        let payload = serde_json::to_value(&self.payload)
            .map_err(|error| Error::Serialization(error.to_string()))?;
        Ok(Context::new(
            self.schema.clone(),
            self.schema_version,
            payload,
        ))
    }
}

impl Context<serde_json::Value> {
    /// Deserialize a dynamic payload into an application-defined type while
    /// preserving the schema identifier and version.
    pub fn try_into_typed<T: DeserializeOwned>(self) -> Result<Context<T>> {
        let payload = serde_json::from_value(self.payload)
            .map_err(|error| Error::Serialization(error.to_string()))?;
        Ok(Context::new(self.schema, self.schema_version, payload))
    }
}

#[derive(Serialize)]
struct DigestEnvelope<'a, T> {
    domain: &'static str,
    schema: &'a str,
    schema_version: u32,
    payload: &'a T,
}
