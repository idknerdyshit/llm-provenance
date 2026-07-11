//! Public error type.

/// Errors produced while validating, canonicalizing, or decoding provenance.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
#[non_exhaustive]
pub enum Error {
    /// A schema identifier is empty, too long, or contains unsupported bytes.
    #[error("invalid schema identifier: {0}")]
    InvalidSchemaId(String),

    /// Schema versions are one-based; zero is never valid.
    #[error("schema version must be greater than zero")]
    InvalidSchemaVersion,

    /// The value cannot be represented safely by RFC 8785/I-JSON.
    #[error("value is not safely representable as I-JSON: {0}")]
    InvalidIJsonNumber(String),

    /// Serde could not serialize or deserialize the supplied value.
    #[error("serialization failed: {0}")]
    Serialization(String),

    /// A digest string or serialized digest has an unsupported shape.
    #[error("invalid context digest: {0}")]
    InvalidDigest(String),

    /// A versioned prompt identifier is invalid.
    #[error("invalid prompt identity: {0}")]
    InvalidPrompt(String),

    /// A monetary value is not an exact decimal string plus ISO-style currency.
    #[error("invalid monetary cost: {0}")]
    InvalidCost(String),

    /// An opaque identifier or locator is empty, too long, or contains a
    /// control character. The rejected value is intentionally not retained.
    #[error("invalid opaque identifier")]
    InvalidOpaqueIdentifier,

    /// A one-based generation attempt was zero.
    #[error("generation attempt must be greater than zero")]
    InvalidAttempt,

    /// A persisted capture timestamp is not a valid RFC 3339 timestamp.
    #[error("invalid audit timestamp")]
    InvalidTimestamp,

    /// A provenance record uses an unsupported wire-format version.
    #[error("unsupported provenance record format version: {0}")]
    UnsupportedProvenanceFormat(u32),

    /// Stored bytes are not a valid canonical context preimage.
    #[error("invalid canonical context preimage")]
    InvalidCanonicalContext,
}

/// Convenience result alias for this crate.
pub type Result<T> = std::result::Result<T, Error>;
