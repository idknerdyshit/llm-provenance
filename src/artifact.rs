//! Content commitments and opaque references for application-retained evidence.

use std::str::FromStr;

use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};

use crate::{Error, Result};

/// Domain separator for raw application artifact commitments.
pub const ARTIFACT_DIGEST_DOMAIN: &str = "llm-provenance/artifact-digest/v1";

const ARTIFACT_DIGEST_PREFIX: &str = "sha256:bytes:v1:";
const MAX_OPAQUE_VALUE_LENGTH: usize = 2_048;

/// SHA-256 commitment to exact application-retained bytes.
///
/// The digest is computed over `ARTIFACT_DIGEST_DOMAIN`, a zero byte, and the
/// exact supplied bytes. It does not normalize JSON, Unicode, or line endings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ArtifactDigest([u8; 32]);

impl ArtifactDigest {
    /// Commit to exact bytes retained by the consuming application.
    #[must_use]
    pub fn from_bytes(bytes: impl AsRef<[u8]>) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(ARTIFACT_DIGEST_DOMAIN.as_bytes());
        hasher.update([0]);
        hasher.update(bytes.as_ref());
        Self(hasher.finalize().into())
    }

    /// Return whether bytes match this artifact commitment.
    #[must_use]
    pub fn verify_bytes(&self, bytes: impl AsRef<[u8]>) -> bool {
        Self::from_bytes(bytes) == *self
    }

    /// Return the raw SHA-256 bytes.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    /// Return lowercase hexadecimal encoding of the digest bytes.
    #[must_use]
    pub fn hex(&self) -> String {
        hex::encode(self.0)
    }
}

impl std::fmt::Display for ArtifactDigest {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{ARTIFACT_DIGEST_PREFIX}{}", self.hex())
    }
}

impl FromStr for ArtifactDigest {
    type Err = Error;

    fn from_str(value: &str) -> Result<Self> {
        let raw_digest = value
            .strip_prefix(ARTIFACT_DIGEST_PREFIX)
            .ok_or_else(|| Error::InvalidDigest(value.to_owned()))?;
        if raw_digest.len() != 64
            || !raw_digest
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(Error::InvalidDigest(value.to_owned()));
        }
        let bytes = hex::decode(raw_digest).map_err(|_| Error::InvalidDigest(value.to_owned()))?;
        let digest = bytes
            .try_into()
            .map_err(|_| Error::InvalidDigest(value.to_owned()))?;
        Ok(Self(digest))
    }
}

impl Serialize for ArtifactDigest {
    fn serialize<S: serde::Serializer>(
        &self,
        serializer: S,
    ) -> std::result::Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for ArtifactDigest {
    fn deserialize<D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> std::result::Result<Self, D::Error> {
        let value = String::deserialize(deserializer)?;
        value.parse().map_err(serde::de::Error::custom)
    }
}

/// Application-defined stable identifier whose contents are not printed by
/// this crate's normal diagnostics or tracing.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(transparent)]
pub struct OpaqueIdentifier(String);

impl OpaqueIdentifier {
    /// Construct a non-empty, control-character-free opaque identifier.
    pub fn new(value: impl Into<String>) -> Result<Self> {
        let value = value.into();
        if valid_opaque_value(&value) {
            Ok(Self(value))
        } else {
            Err(Error::InvalidOpaqueIdentifier)
        }
    }

    /// Borrow the opaque identifier for application-owned resolution.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for OpaqueIdentifier {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl FromStr for OpaqueIdentifier {
    type Err = Error;

    fn from_str(value: &str) -> Result<Self> {
        Self::new(value)
    }
}

impl<'de> Deserialize<'de> for OpaqueIdentifier {
    fn deserialize<D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> std::result::Result<Self, D::Error> {
        Self::new(String::deserialize(deserializer)?).map_err(serde::de::Error::custom)
    }
}

/// Opaque application-owned locator for retained evidence.
///
/// Locators must not contain credentials. This type deliberately does not
/// impose URL semantics because applications may use database, object-store,
/// or archival identifiers.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(transparent)]
pub struct ArtifactLocator(String);

impl ArtifactLocator {
    /// Construct a non-empty, control-character-free evidence locator.
    pub fn new(value: impl Into<String>) -> Result<Self> {
        let value = value.into();
        if valid_opaque_value(&value) {
            Ok(Self(value))
        } else {
            Err(Error::InvalidOpaqueIdentifier)
        }
    }

    /// Borrow the locator for application-owned evidence resolution.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl FromStr for ArtifactLocator {
    type Err = Error;

    fn from_str(value: &str) -> Result<Self> {
        Self::new(value)
    }
}

impl<'de> Deserialize<'de> for ArtifactLocator {
    fn deserialize<D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> std::result::Result<Self, D::Error> {
        Self::new(String::deserialize(deserializer)?).map_err(serde::de::Error::custom)
    }
}

/// An external evidence location paired with its exact byte commitment.
#[derive(Clone, PartialEq, Eq, Serialize)]
pub struct ArtifactReference {
    locator: ArtifactLocator,
    digest: ArtifactDigest,
}

impl ArtifactReference {
    /// Pair an application-owned locator with a commitment to retained bytes.
    #[must_use]
    pub fn new(locator: ArtifactLocator, digest: ArtifactDigest) -> Self {
        Self { locator, digest }
    }

    /// Return the external evidence locator.
    #[must_use]
    pub fn locator(&self) -> &ArtifactLocator {
        &self.locator
    }

    /// Return the exact-byte commitment.
    #[must_use]
    pub fn digest(&self) -> ArtifactDigest {
        self.digest
    }

    /// Return whether resolved bytes match the retained commitment.
    #[must_use]
    pub fn verify_bytes(&self, bytes: impl AsRef<[u8]>) -> bool {
        self.digest.verify_bytes(bytes)
    }
}

impl<'de> Deserialize<'de> for ArtifactReference {
    fn deserialize<D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> std::result::Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Wire {
            locator: ArtifactLocator,
            digest: ArtifactDigest,
        }

        let wire = Wire::deserialize(deserializer)?;
        Ok(Self::new(wire.locator, wire.digest))
    }
}

fn valid_opaque_value(value: &str) -> bool {
    !value.trim().is_empty()
        && value.len() <= MAX_OPAQUE_VALUE_LENGTH
        && !value.chars().any(char::is_control)
}
