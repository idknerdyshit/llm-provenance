//! Versioned, audit-safe evidence for one LLM generation.

use std::num::NonZeroU32;
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};
use time::format_description::well_known::Rfc3339;
use time::{OffsetDateTime, UtcOffset};

use crate::validate::to_i_json_value;
use crate::{
    ArtifactReference, Context, ContextDigest, ContextVerification, Error, OpaqueIdentifier,
    Result, VersionedComponent,
};

/// Domain separator used to fingerprint the complete generation record.
pub const GENERATION_PROVENANCE_FINGERPRINT_DOMAIN: &str =
    "llm-provenance/generation-provenance-fingerprint/v1";

const FINGERPRINT_PREFIX: &str = "sha256:rfc8785:generation-provenance-v1:";

/// Stable identity of the prompt template selected by an application.
#[derive(Clone, PartialEq, Eq, Serialize)]
pub struct VersionedPrompt {
    id: OpaqueIdentifier,
    version: NonZeroU32,
}

impl VersionedPrompt {
    /// Construct a prompt identity with a one-based application version.
    pub fn new(id: impl Into<String>, version: u32) -> Result<Self> {
        Ok(Self {
            id: OpaqueIdentifier::new(id)?,
            version: NonZeroU32::new(version)
                .ok_or(Error::InvalidPrompt("version 0".to_owned()))?,
        })
    }

    /// Borrow the application-defined prompt identifier.
    #[must_use]
    pub fn id(&self) -> &str {
        self.id.as_str()
    }

    /// Return the one-based prompt version.
    #[must_use]
    pub fn version(&self) -> u32 {
        self.version.get()
    }
}

impl<'de> Deserialize<'de> for VersionedPrompt {
    fn deserialize<D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> std::result::Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Wire {
            id: OpaqueIdentifier,
            version: u32,
        }

        let wire = Wire::deserialize(deserializer)?;
        let version = NonZeroU32::new(wire.version)
            .ok_or_else(|| serde::de::Error::custom("prompt version must be greater than zero"))?;
        Ok(Self {
            id: wire.id,
            version,
        })
    }
}

/// Exact prompt evidence retained by the consuming application.
#[derive(Clone, PartialEq, Eq, Serialize)]
pub struct PromptEvidence {
    identity: VersionedPrompt,
    template: ArtifactReference,
    renderer: VersionedComponent,
    rendered_request: ArtifactReference,
}

impl PromptEvidence {
    /// Construct prompt-template and exact-rendered-request evidence.
    #[must_use]
    pub fn new(
        identity: VersionedPrompt,
        template: ArtifactReference,
        renderer: VersionedComponent,
        rendered_request: ArtifactReference,
    ) -> Self {
        Self {
            identity,
            template,
            renderer,
            rendered_request,
        }
    }

    /// Return the semantic prompt identity.
    #[must_use]
    pub fn identity(&self) -> &VersionedPrompt {
        &self.identity
    }

    /// Return retained prompt-template evidence.
    #[must_use]
    pub fn template(&self) -> &ArtifactReference {
        &self.template
    }

    /// Return the template-renderer identity.
    #[must_use]
    pub fn renderer(&self) -> &VersionedComponent {
        &self.renderer
    }

    /// Return the exact rendered provider-request evidence.
    #[must_use]
    pub fn rendered_request(&self) -> &ArtifactReference {
        &self.rendered_request
    }
}

impl<'de> Deserialize<'de> for PromptEvidence {
    fn deserialize<D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> std::result::Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Wire {
            identity: VersionedPrompt,
            template: ArtifactReference,
            renderer: VersionedComponent,
            rendered_request: ArtifactReference,
        }

        let wire = Wire::deserialize(deserializer)?;
        Ok(Self::new(
            wire.identity,
            wire.template,
            wire.renderer,
            wire.rendered_request,
        ))
    }
}

/// Provider, model, and immutable model revision actually used for execution.
#[derive(Clone, PartialEq, Eq, Serialize)]
pub struct ModelIdentity {
    provider: OpaqueIdentifier,
    model: OpaqueIdentifier,
    revision: OpaqueIdentifier,
}

impl ModelIdentity {
    /// Construct an immutable model identity.
    pub fn new(
        provider: impl Into<String>,
        model: impl Into<String>,
        revision: impl Into<String>,
    ) -> Result<Self> {
        Ok(Self {
            provider: OpaqueIdentifier::new(provider)?,
            model: OpaqueIdentifier::new(model)?,
            revision: OpaqueIdentifier::new(revision)?,
        })
    }

    /// Return the provider identifier.
    #[must_use]
    pub fn provider(&self) -> &OpaqueIdentifier {
        &self.provider
    }

    /// Return the model identifier.
    #[must_use]
    pub fn model(&self) -> &OpaqueIdentifier {
        &self.model
    }

    /// Return the immutable provider model revision.
    #[must_use]
    pub fn revision(&self) -> &OpaqueIdentifier {
        &self.revision
    }
}

impl<'de> Deserialize<'de> for ModelIdentity {
    fn deserialize<D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> std::result::Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Wire {
            provider: OpaqueIdentifier,
            model: OpaqueIdentifier,
            revision: OpaqueIdentifier,
        }

        let wire = Wire::deserialize(deserializer)?;
        Ok(Self {
            provider: wire.provider,
            model: wire.model,
            revision: wire.revision,
        })
    }
}

/// Canonical UTC RFC 3339 capture time for a generation attempt.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AuditTimestamp(String);

impl AuditTimestamp {
    /// Parse and normalize an RFC 3339 timestamp to UTC.
    pub fn parse_rfc3339(value: &str) -> Result<Self> {
        let parsed = OffsetDateTime::parse(value, &Rfc3339).map_err(|_| Error::InvalidTimestamp)?;
        let normalized = parsed
            .to_offset(UtcOffset::UTC)
            .format(&Rfc3339)
            .map_err(|_| Error::InvalidTimestamp)?;
        Ok(Self(normalized))
    }

    /// Borrow canonical UTC RFC 3339 text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl FromStr for AuditTimestamp {
    type Err = Error;

    fn from_str(value: &str) -> Result<Self> {
        Self::parse_rfc3339(value)
    }
}

impl Serialize for AuditTimestamp {
    fn serialize<S: serde::Serializer>(
        &self,
        serializer: S,
    ) -> std::result::Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for AuditTimestamp {
    fn deserialize<D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> std::result::Result<Self, D::Error> {
        Self::parse_rfc3339(&String::deserialize(deserializer)?).map_err(serde::de::Error::custom)
    }
}

/// Application execution identity for one generation attempt.
#[derive(Clone, PartialEq, Eq, Serialize)]
pub struct ExecutionEvidence {
    application: VersionedComponent,
    operation_id: OpaqueIdentifier,
    attempt: NonZeroU32,
    captured_at: AuditTimestamp,
}

impl ExecutionEvidence {
    /// Construct an execution identity with a one-based attempt number.
    pub fn new(
        application: VersionedComponent,
        operation_id: impl Into<String>,
        attempt: u32,
        captured_at: AuditTimestamp,
    ) -> Result<Self> {
        Ok(Self {
            application,
            operation_id: OpaqueIdentifier::new(operation_id)?,
            attempt: NonZeroU32::new(attempt).ok_or(Error::InvalidAttempt)?,
            captured_at,
        })
    }

    /// Return the application build identity.
    #[must_use]
    pub fn application(&self) -> &VersionedComponent {
        &self.application
    }

    /// Return the opaque application operation identifier.
    #[must_use]
    pub fn operation_id(&self) -> &OpaqueIdentifier {
        &self.operation_id
    }

    /// Return the one-based attempt number.
    #[must_use]
    pub fn attempt(&self) -> u32 {
        self.attempt.get()
    }

    /// Return capture time in canonical UTC RFC 3339 form.
    #[must_use]
    pub fn captured_at(&self) -> &AuditTimestamp {
        &self.captured_at
    }
}

impl<'de> Deserialize<'de> for ExecutionEvidence {
    fn deserialize<D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> std::result::Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Wire {
            application: VersionedComponent,
            operation_id: OpaqueIdentifier,
            attempt: u32,
            captured_at: AuditTimestamp,
        }

        let wire = Wire::deserialize(deserializer)?;
        let attempt = NonZeroU32::new(wire.attempt).ok_or_else(|| {
            serde::de::Error::custom("generation attempt must be greater than zero")
        })?;
        Ok(Self {
            application: wire.application,
            operation_id: wire.operation_id,
            attempt,
            captured_at: wire.captured_at,
        })
    }
}

/// Cache behavior observed for a generation.
#[derive(Clone, PartialEq, Eq, Serialize)]
pub struct CacheProvenance {
    key: Option<OpaqueIdentifier>,
    hit: bool,
}

impl CacheProvenance {
    /// Construct cache metadata when no stable cache key is retained.
    #[must_use]
    pub fn new(hit: bool) -> Self {
        Self { key: None, hit }
    }

    /// Construct cache metadata with a validated opaque cache key.
    pub fn with_key(hit: bool, key: impl Into<String>) -> Result<Self> {
        Ok(Self {
            key: Some(OpaqueIdentifier::new(key)?),
            hit,
        })
    }

    /// Return the opaque cache key, if retained.
    #[must_use]
    pub fn key(&self) -> Option<&OpaqueIdentifier> {
        self.key.as_ref()
    }

    /// Return whether the response was served from cache.
    #[must_use]
    pub fn hit(&self) -> bool {
        self.hit
    }
}

impl Default for CacheProvenance {
    fn default() -> Self {
        Self::new(false)
    }
}

impl<'de> Deserialize<'de> for CacheProvenance {
    fn deserialize<D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> std::result::Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Wire {
            key: Option<OpaqueIdentifier>,
            hit: bool,
        }

        let wire = Wire::deserialize(deserializer)?;
        Ok(Self {
            key: wire.key,
            hit: wire.hit,
        })
    }
}

/// Provider-reported token accounting.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct TokenUsage {
    /// Prompt/input token count when reported.
    pub input_tokens: Option<u64>,
    /// Completion/output token count when reported.
    pub output_tokens: Option<u64>,
}

/// Exact decimal monetary estimate associated with a generation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MonetaryCost {
    amount: String,
    currency: String,
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
    #[must_use]
    pub fn amount(&self) -> &str {
        &self.amount
    }

    /// Borrow the uppercase three-letter currency code.
    #[must_use]
    pub fn currency(&self) -> &str {
        &self.currency
    }
}

impl<'de> Deserialize<'de> for MonetaryCost {
    fn deserialize<D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> std::result::Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Wire {
            amount: String,
            currency: String,
        }

        let wire = Wire::deserialize(deserializer)?;
        Self::new(wire.amount, wire.currency).map_err(serde::de::Error::custom)
    }
}

/// Stable fingerprint of a complete generation-provenance record.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GenerationProvenanceFingerprint([u8; 32]);

impl GenerationProvenanceFingerprint {
    /// Return raw SHA-256 fingerprint bytes.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    /// Return lowercase hexadecimal fingerprint bytes.
    #[must_use]
    pub fn hex(&self) -> String {
        hex::encode(self.0)
    }
}

impl std::fmt::Display for GenerationProvenanceFingerprint {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{FINGERPRINT_PREFIX}{}", self.hex())
    }
}

impl FromStr for GenerationProvenanceFingerprint {
    type Err = Error;

    fn from_str(value: &str) -> Result<Self> {
        let raw_digest = value
            .strip_prefix(FINGERPRINT_PREFIX)
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

impl Serialize for GenerationProvenanceFingerprint {
    fn serialize<S: serde::Serializer>(
        &self,
        serializer: S,
    ) -> std::result::Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for GenerationProvenanceFingerprint {
    fn deserialize<D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> std::result::Result<Self, D::Error> {
        String::deserialize(deserializer)?
            .parse()
            .map_err(serde::de::Error::custom)
    }
}

/// Required external artifacts for a replayable generation record.
///
/// This construction helper is not serialized as a nested wire object; the
/// final [`GenerationProvenance`] keeps its stable top-level field names.
#[derive(Clone, PartialEq, Eq)]
pub struct RetainedGenerationArtifacts {
    context_preimage: ArtifactReference,
    decoding_config: ArtifactReference,
    provider_response: ArtifactReference,
    output: ArtifactReference,
}

impl RetainedGenerationArtifacts {
    /// Collect the required retained artifacts for one generation record.
    #[must_use]
    pub fn new(
        context_preimage: ArtifactReference,
        decoding_config: ArtifactReference,
        provider_response: ArtifactReference,
        output: ArtifactReference,
    ) -> Self {
        Self {
            context_preimage,
            decoding_config,
            provider_response,
            output,
        }
    }
}

/// Complete, versioned audit evidence for one LLM generation.
///
/// This record contains only references and commitments. Applications retain
/// raw context, rendered requests, provider responses, and outputs elsewhere
/// under their own encryption, access-control, retention, and deletion policy.
#[derive(Clone, PartialEq, Eq, Serialize)]
pub struct GenerationProvenance {
    format_version: u32,
    model: ModelIdentity,
    prompt: PromptEvidence,
    context: ContextDigest,
    observed_context: Option<ContextDigest>,
    context_preimage: ArtifactReference,
    decoding_config: ArtifactReference,
    tool_config: Option<ArtifactReference>,
    structured_output_schema: Option<ArtifactReference>,
    seed: Option<u64>,
    provider_response: ArtifactReference,
    output: ArtifactReference,
    execution: ExecutionEvidence,
    cache: CacheProvenance,
    usage: TokenUsage,
    provider_generation_id: Option<OpaqueIdentifier>,
    estimated_cost: Option<MonetaryCost>,
}

impl GenerationProvenance {
    /// Current stable wire-format version for generation provenance.
    pub const FORMAT_VERSION: u32 = 1;

    /// Begin constructing strict generation evidence. Every argument is
    /// required for a replayable record; optional evidence is set on the
    /// returned builder.
    #[must_use]
    pub fn builder(
        model: ModelIdentity,
        prompt: PromptEvidence,
        context: ContextDigest,
        artifacts: RetainedGenerationArtifacts,
        execution: ExecutionEvidence,
    ) -> GenerationProvenanceBuilder {
        GenerationProvenanceBuilder {
            model,
            prompt,
            context,
            context_preimage: artifacts.context_preimage,
            decoding_config: artifacts.decoding_config,
            provider_response: artifacts.provider_response,
            output: artifacts.output,
            execution,
            observed_context: None,
            tool_config: None,
            structured_output_schema: None,
            seed: None,
            cache: CacheProvenance::default(),
            usage: TokenUsage::default(),
            provider_generation_id: None,
            estimated_cost: None,
        }
    }

    /// Return the persisted provenance format version.
    #[must_use]
    pub fn format_version(&self) -> u32 {
        self.format_version
    }

    /// Return immutable provider/model evidence.
    #[must_use]
    pub fn model(&self) -> &ModelIdentity {
        &self.model
    }

    /// Return prompt template and exact rendered-request evidence.
    #[must_use]
    pub fn prompt(&self) -> &PromptEvidence {
        &self.prompt
    }

    /// Return the final context digest used for the persisted artifact.
    #[must_use]
    pub fn context(&self) -> &ContextDigest {
        &self.context
    }

    /// Return a context digest observed before generation, if the application
    /// rechecked context before persistence.
    #[must_use]
    pub fn observed_context(&self) -> Option<&ContextDigest> {
        self.observed_context.as_ref()
    }

    /// Return archived canonical-context-preimage evidence.
    #[must_use]
    pub fn context_preimage(&self) -> &ArtifactReference {
        &self.context_preimage
    }

    /// Return required decoding configuration evidence.
    #[must_use]
    pub fn decoding_config(&self) -> &ArtifactReference {
        &self.decoding_config
    }

    /// Return tool configuration evidence, if no-tools was not used.
    #[must_use]
    pub fn tool_config(&self) -> Option<&ArtifactReference> {
        self.tool_config.as_ref()
    }

    /// Return structured-output schema evidence, if one was requested.
    #[must_use]
    pub fn structured_output_schema(&self) -> Option<&ArtifactReference> {
        self.structured_output_schema.as_ref()
    }

    /// Return a provider-supported seed, if supplied.
    #[must_use]
    pub fn seed(&self) -> Option<u64> {
        self.seed
    }

    /// Return raw provider-response evidence.
    #[must_use]
    pub fn provider_response(&self) -> &ArtifactReference {
        &self.provider_response
    }

    /// Return normalized persisted output evidence.
    #[must_use]
    pub fn output(&self) -> &ArtifactReference {
        &self.output
    }

    /// Return application execution identity.
    #[must_use]
    pub fn execution(&self) -> &ExecutionEvidence {
        &self.execution
    }

    /// Return cache evidence.
    #[must_use]
    pub fn cache(&self) -> &CacheProvenance {
        &self.cache
    }

    /// Return token accounting.
    #[must_use]
    pub fn usage(&self) -> TokenUsage {
        self.usage
    }

    /// Return an opaque provider generation identifier, if reported.
    #[must_use]
    pub fn provider_generation_id(&self) -> Option<&OpaqueIdentifier> {
        self.provider_generation_id.as_ref()
    }

    /// Return exact estimated cost, if retained.
    #[must_use]
    pub fn estimated_cost(&self) -> Option<&MonetaryCost> {
        self.estimated_cost.as_ref()
    }

    /// Whether the originally observed context differs from final context.
    #[must_use]
    pub fn context_changed(&self) -> bool {
        self.observed_context
            .as_ref()
            .is_some_and(|observed| observed != &self.context)
    }

    /// Verify a rebuilt context against the final persisted context digest.
    pub fn verify_context<T: Serialize>(
        &self,
        context: &Context<T>,
    ) -> Result<ContextVerification> {
        context.verify_digest(&self.context)
    }

    /// Return the exact canonical bytes fingerprinted for this record.
    pub fn canonical_bytes(&self) -> Result<Vec<u8>> {
        let envelope = FingerprintEnvelope {
            domain: GENERATION_PROVENANCE_FINGERPRINT_DOMAIN,
            record: self,
        };
        let value = to_i_json_value(&envelope)?;
        serde_json_canonicalizer::to_vec(&value)
            .map_err(|error| Error::Serialization(error.to_string()))
    }

    /// Compute a stable, domain-separated fingerprint suitable for application
    /// managed signatures or append-only ledgers.
    pub fn fingerprint(&self) -> Result<GenerationProvenanceFingerprint> {
        Ok(GenerationProvenanceFingerprint(
            Sha256::digest(self.canonical_bytes()?).into(),
        ))
    }
}

impl<'de> Deserialize<'de> for GenerationProvenance {
    fn deserialize<D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> std::result::Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Wire {
            format_version: u32,
            model: ModelIdentity,
            prompt: PromptEvidence,
            context: ContextDigest,
            observed_context: Option<ContextDigest>,
            context_preimage: ArtifactReference,
            decoding_config: ArtifactReference,
            tool_config: Option<ArtifactReference>,
            structured_output_schema: Option<ArtifactReference>,
            seed: Option<u64>,
            provider_response: ArtifactReference,
            output: ArtifactReference,
            execution: ExecutionEvidence,
            cache: CacheProvenance,
            usage: TokenUsage,
            provider_generation_id: Option<OpaqueIdentifier>,
            estimated_cost: Option<MonetaryCost>,
        }

        let wire = Wire::deserialize(deserializer)?;
        if wire.format_version != Self::FORMAT_VERSION {
            return Err(serde::de::Error::custom(format!(
                "unsupported provenance record format version: {}",
                wire.format_version
            )));
        }
        let provenance = Self {
            format_version: wire.format_version,
            model: wire.model,
            prompt: wire.prompt,
            context: wire.context,
            observed_context: wire.observed_context,
            context_preimage: wire.context_preimage,
            decoding_config: wire.decoding_config,
            tool_config: wire.tool_config,
            structured_output_schema: wire.structured_output_schema,
            seed: wire.seed,
            provider_response: wire.provider_response,
            output: wire.output,
            execution: wire.execution,
            cache: wire.cache,
            usage: wire.usage,
            provider_generation_id: wire.provider_generation_id,
            estimated_cost: wire.estimated_cost,
        };
        provenance
            .canonical_bytes()
            .map_err(serde::de::Error::custom)?;
        Ok(provenance)
    }
}

/// Builder for a strict version-1 generation-provenance record.
pub struct GenerationProvenanceBuilder {
    model: ModelIdentity,
    prompt: PromptEvidence,
    context: ContextDigest,
    context_preimage: ArtifactReference,
    decoding_config: ArtifactReference,
    provider_response: ArtifactReference,
    output: ArtifactReference,
    execution: ExecutionEvidence,
    observed_context: Option<ContextDigest>,
    tool_config: Option<ArtifactReference>,
    structured_output_schema: Option<ArtifactReference>,
    seed: Option<u64>,
    cache: CacheProvenance,
    usage: TokenUsage,
    provider_generation_id: Option<OpaqueIdentifier>,
    estimated_cost: Option<MonetaryCost>,
}

impl GenerationProvenanceBuilder {
    /// Record a context digest observed before generation.
    #[must_use]
    pub fn observed_context(mut self, context: ContextDigest) -> Self {
        self.observed_context = Some(context);
        self
    }

    /// Record retained tool configuration evidence.
    #[must_use]
    pub fn tool_config(mut self, configuration: ArtifactReference) -> Self {
        self.tool_config = Some(configuration);
        self
    }

    /// Record retained structured-output schema evidence.
    #[must_use]
    pub fn structured_output_schema(mut self, schema: ArtifactReference) -> Self {
        self.structured_output_schema = Some(schema);
        self
    }

    /// Record a provider-supported seed.
    #[must_use]
    pub fn seed(mut self, seed: u64) -> Self {
        self.seed = Some(seed);
        self
    }

    /// Set cache metadata.
    #[must_use]
    pub fn cache(mut self, cache: CacheProvenance) -> Self {
        self.cache = cache;
        self
    }

    /// Set provider-reported token accounting.
    #[must_use]
    pub fn usage(mut self, usage: TokenUsage) -> Self {
        self.usage = usage;
        self
    }

    /// Record an opaque provider generation identifier.
    pub fn provider_generation_id(mut self, value: impl Into<String>) -> Result<Self> {
        self.provider_generation_id = Some(OpaqueIdentifier::new(value)?);
        Ok(self)
    }

    /// Record an exact estimated cost.
    #[must_use]
    pub fn estimated_cost(mut self, cost: MonetaryCost) -> Self {
        self.estimated_cost = Some(cost);
        self
    }

    /// Finalize a fully populated, canonicalizable version-1 record.
    pub fn build(self) -> Result<GenerationProvenance> {
        let provenance = GenerationProvenance {
            format_version: GenerationProvenance::FORMAT_VERSION,
            model: self.model,
            prompt: self.prompt,
            context: self.context,
            observed_context: self.observed_context,
            context_preimage: self.context_preimage,
            decoding_config: self.decoding_config,
            tool_config: self.tool_config,
            structured_output_schema: self.structured_output_schema,
            seed: self.seed,
            provider_response: self.provider_response,
            output: self.output,
            execution: self.execution,
            cache: self.cache,
            usage: self.usage,
            provider_generation_id: self.provider_generation_id,
            estimated_cost: self.estimated_cost,
        };
        provenance.canonical_bytes()?;
        Ok(provenance)
    }
}

#[derive(Serialize)]
struct FingerprintEnvelope<'a> {
    domain: &'static str,
    record: &'a GenerationProvenance,
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
