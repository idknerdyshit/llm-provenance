//! Audit-safe metadata describing an LLM generation.

use serde::{Deserialize, Serialize};

use crate::{ContextDigest, Error, Result};

/// Stable identity of the prompt template used for a generation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct VersionedPrompt {
    /// Application-defined prompt identifier; rendered prompt text does not
    /// belong in provenance.
    id: String,
    /// One-based prompt version.
    version: u32,
}

impl<'de> Deserialize<'de> for VersionedPrompt {
    fn deserialize<D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> std::result::Result<Self, D::Error> {
        #[derive(Deserialize)]
        struct Wire {
            id: String,
            version: u32,
        }

        let wire = Wire::deserialize(deserializer)?;
        Self::new(wire.id, wire.version).map_err(serde::de::Error::custom)
    }
}

impl VersionedPrompt {
    /// Construct a validated prompt identity.
    pub fn new(id: impl Into<String>, version: u32) -> Result<Self> {
        let id = id.into();
        if id.trim().is_empty() || id.len() > 128 || version == 0 {
            return Err(Error::InvalidPrompt(format!("{id}@{version}")));
        }
        Ok(Self { id, version })
    }

    /// Borrow the application-defined prompt identifier.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Return the one-based prompt version.
    pub fn version(&self) -> u32 {
        self.version
    }
}

/// Cache behavior observed for a generation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct CacheProvenance {
    /// Opaque cache key or digest; never a credential.
    pub key: Option<String>,
    /// Whether the response was served from cache.
    pub hit: bool,
}

/// Provider-reported token accounting.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct TokenUsage {
    /// Prompt/input token count when reported.
    pub input_tokens: Option<u64>,
    /// Completion/output token count when reported.
    pub output_tokens: Option<u64>,
}

/// Exact decimal monetary estimate associated with a generation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MonetaryCost {
    /// Exact base-10 decimal without exponent notation.
    amount: String,
    /// Three-letter uppercase currency code, such as `USD`.
    currency: String,
}

impl<'de> Deserialize<'de> for MonetaryCost {
    fn deserialize<D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> std::result::Result<Self, D::Error> {
        #[derive(Deserialize)]
        struct Wire {
            amount: String,
            currency: String,
        }

        let wire = Wire::deserialize(deserializer)?;
        Self::new(wire.amount, wire.currency).map_err(serde::de::Error::custom)
    }
}

impl MonetaryCost {
    /// Construct a validated exact monetary value.
    pub fn new(amount: impl Into<String>, currency: impl Into<String>) -> Result<Self> {
        let amount = amount.into();
        let currency = currency.into();
        if !valid_decimal(&amount)
            || currency.len() != 3
            || !currency.bytes().all(|byte| byte.is_ascii_uppercase())
        {
            return Err(Error::InvalidCost(format!("{amount} {currency}")));
        }
        Ok(Self { amount, currency })
    }

    /// Borrow the exact base-10 amount.
    pub fn amount(&self) -> &str {
        &self.amount
    }

    /// Borrow the uppercase three-letter currency code.
    pub fn currency(&self) -> &str {
        &self.currency
    }
}

/// Provider-neutral provenance for one generated artifact.
///
/// This structure intentionally stores only references and accounting metadata:
/// no rendered prompts, raw context, generated body, or credentials.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GenerationProvenance {
    /// Provider/model identifier used for generation.
    pub model: String,
    /// Versioned prompt template identity.
    pub prompt: VersionedPrompt,
    /// Digest of the final context used for the persisted artifact.
    pub context: ContextDigest,
    /// Digest observed before generation, when the application rechecked
    /// context before persistence.
    pub observed_context: Option<ContextDigest>,
    /// Cache metadata.
    pub cache: CacheProvenance,
    /// Token accounting.
    pub usage: TokenUsage,
    /// Opaque provider generation identifier.
    pub provider_generation_id: Option<String>,
    /// Optional exact estimated cost.
    pub estimated_cost: Option<MonetaryCost>,
}

impl GenerationProvenance {
    /// Whether the originally observed context differs from the final context.
    pub fn context_changed(&self) -> bool {
        self.observed_context
            .as_ref()
            .is_some_and(|observed| observed != &self.context)
    }
}

fn valid_decimal(value: &str) -> bool {
    let value = value.strip_prefix('-').unwrap_or(value);
    if value.is_empty() || value.starts_with('+') {
        return false;
    }
    let mut parts = value.split('.');
    let integer = parts.next().unwrap_or_default();
    let fraction = parts.next();
    if parts.next().is_some()
        || integer.is_empty()
        || !integer.bytes().all(|byte| byte.is_ascii_digit())
        || (integer.len() > 1 && integer.starts_with('0'))
    {
        return false;
    }
    fraction
        .is_none_or(|digits| !digits.is_empty() && digits.bytes().all(|byte| byte.is_ascii_digit()))
}
