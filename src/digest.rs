//! Structured context digests and stable text/JSON encodings.

use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::{Error, Result, SchemaId, SchemaVersion};

/// Hash algorithm used for context digests.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum HashAlgorithm {
    /// SHA-256.
    Sha256,
}

/// Canonical representation hashed to produce a context digest.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Canonicalization {
    /// RFC 8785 JSON Canonicalization Scheme.
    Rfc8785,
}

/// A self-describing digest of a versioned context.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ContextDigest {
    schema: SchemaId,
    schema_version: SchemaVersion,
    algorithm: HashAlgorithm,
    canonicalization: Canonicalization,
    #[serde(with = "hex::serde")]
    digest: [u8; 32],
}

impl ContextDigest {
    pub(crate) fn from_parts(
        schema: SchemaId,
        schema_version: SchemaVersion,
        digest: [u8; 32],
    ) -> Self {
        Self {
            schema,
            schema_version,
            algorithm: HashAlgorithm::Sha256,
            canonicalization: Canonicalization::Rfc8785,
            digest,
        }
    }

    /// Return the application schema identifier carried by the digest.
    pub fn schema(&self) -> &SchemaId {
        &self.schema
    }

    /// Return the application schema version carried by the digest.
    pub fn schema_version(&self) -> SchemaVersion {
        self.schema_version
    }

    /// Return the hash algorithm.
    pub fn algorithm(&self) -> HashAlgorithm {
        self.algorithm
    }

    /// Return the canonicalization scheme.
    pub fn canonicalization(&self) -> Canonicalization {
        self.canonicalization
    }

    /// Return the raw SHA-256 bytes.
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.digest
    }

    /// Return the lowercase hexadecimal digest for compatibility with stores
    /// that already persist an opaque `context_hash` string.
    pub fn hex(&self) -> String {
        hex::encode(self.digest)
    }
}

impl std::fmt::Display for ContextDigest {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "sha256:rfc8785:{}:{}:{}",
            self.schema,
            self.schema_version.get(),
            self.hex()
        )
    }
}

impl FromStr for ContextDigest {
    type Err = Error;

    fn from_str(value: &str) -> Result<Self> {
        let mut parts = value.split(':');
        let algorithm = parts.next();
        let canonicalization = parts.next();
        let schema = parts.next();
        let version = parts.next();
        let digest = parts.next();
        if algorithm != Some("sha256")
            || canonicalization != Some("rfc8785")
            || parts.next().is_some()
        {
            return Err(Error::InvalidDigest(value.to_owned()));
        }
        let schema = SchemaId::new(schema.unwrap_or_default())
            .map_err(|_| Error::InvalidDigest(value.to_owned()))?;
        let raw_version = version.unwrap_or_default();
        let version = raw_version
            .parse::<u32>()
            .ok()
            .filter(|parsed| parsed.to_string() == raw_version)
            .and_then(|raw| SchemaVersion::new(raw).ok())
            .ok_or_else(|| Error::InvalidDigest(value.to_owned()))?;
        let raw_digest = digest.unwrap_or_default();
        if raw_digest.len() != 64
            || !raw_digest
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(Error::InvalidDigest(value.to_owned()));
        }
        let bytes = hex::decode(raw_digest).map_err(|_| Error::InvalidDigest(value.to_owned()))?;
        let digest: [u8; 32] = bytes
            .try_into()
            .map_err(|_| Error::InvalidDigest(value.to_owned()))?;
        Ok(Self::from_parts(schema, version, digest))
    }
}

impl<'de> Deserialize<'de> for ContextDigest {
    fn deserialize<D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> std::result::Result<Self, D::Error> {
        #[derive(Deserialize)]
        struct Wire {
            schema: SchemaId,
            schema_version: SchemaVersion,
            algorithm: HashAlgorithm,
            canonicalization: Canonicalization,
            #[serde(with = "hex::serde")]
            digest: Vec<u8>,
        }

        let wire = Wire::deserialize(deserializer)?;
        if wire.algorithm != HashAlgorithm::Sha256
            || wire.canonicalization != Canonicalization::Rfc8785
        {
            return Err(serde::de::Error::custom("unsupported digest metadata"));
        }
        let digest = wire
            .digest
            .try_into()
            .map_err(|_| serde::de::Error::custom("SHA-256 digest must contain 32 bytes"))?;
        Ok(Self::from_parts(wire.schema, wire.schema_version, digest))
    }
}
