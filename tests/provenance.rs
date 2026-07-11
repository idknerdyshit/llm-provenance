use std::str::FromStr;

use llm_provenance::{
    ArtifactDigest, ArtifactLocator, ArtifactReference, AuditTimestamp, CacheProvenance, Context,
    ContextVerification, ExecutionEvidence, GenerationProvenance, GenerationProvenanceFingerprint,
    ModelIdentity, MonetaryCost, PromptEvidence, RetainedGenerationArtifacts, SchemaId,
    SchemaVersion, TokenUsage, VersionedComponent, VersionedPrompt,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

fn digest(value: &str) -> llm_provenance::ContextDigest {
    Context::new(
        SchemaId::new("example.intent").expect("schema"),
        SchemaVersion::new(1).expect("version"),
        json!({"input": value}),
    )
    .digest()
    .expect("digest")
}

fn artifact(locator: &str, bytes: impl AsRef<[u8]>) -> ArtifactReference {
    ArtifactReference::new(
        ArtifactLocator::new(locator).expect("locator"),
        ArtifactDigest::from_bytes(bytes),
    )
}

#[derive(Clone, Copy)]
struct EvidenceInputs {
    template: &'static [u8],
    request: &'static [u8],
    model_revision: &'static str,
    decoding_config: &'static [u8],
    tool_config: &'static [u8],
    output_schema: &'static [u8],
    seed: u64,
    provider_response: &'static [u8],
    output: &'static [u8],
    attempt: u32,
}

impl Default for EvidenceInputs {
    fn default() -> Self {
        Self {
            template: b"raw template",
            request: b"raw rendered request",
            model_revision: "2026-07-01",
            decoding_config: b"{\"temperature\":0}",
            tool_config: b"tool configuration",
            output_schema: b"structured output schema",
            seed: 17,
            provider_response: b"raw provider response",
            output: b"normalized output",
            attempt: 1,
        }
    }
}

fn provenance(context: llm_provenance::ContextDigest) -> GenerationProvenance {
    provenance_with(context, EvidenceInputs::default())
}

fn provenance_with(
    context: llm_provenance::ContextDigest,
    inputs: EvidenceInputs,
) -> GenerationProvenance {
    let prompt = PromptEvidence::new(
        VersionedPrompt::new("intent-classifier", 3).expect("prompt"),
        artifact("archive/prompt-template", inputs.template),
        VersionedComponent::new("prompt-renderer", "git:abc123").expect("renderer"),
        artifact("archive/rendered-request", inputs.request),
    );
    let execution = ExecutionEvidence::new(
        VersionedComponent::new("support-service", "git:def456").expect("application"),
        "operation-123",
        inputs.attempt,
        AuditTimestamp::parse_rfc3339("2026-07-11T16:30:00-04:00").expect("timestamp"),
    )
    .expect("execution");

    GenerationProvenance::builder(
        ModelIdentity::new("provider", "model", inputs.model_revision).expect("model"),
        prompt,
        context,
        RetainedGenerationArtifacts::new(
            artifact("archive/context-preimage", b"raw canonical context"),
            artifact("archive/decoding-config", inputs.decoding_config),
            artifact("archive/provider-response", inputs.provider_response),
            artifact("archive/output", inputs.output),
        ),
        execution,
    )
    .tool_config(artifact("archive/tool-config", inputs.tool_config))
    .structured_output_schema(artifact("archive/output-schema", inputs.output_schema))
    .seed(inputs.seed)
    .cache(CacheProvenance::with_key(false, "cache-commitment").expect("cache"))
    .usage(TokenUsage {
        input_tokens: Some(42),
        output_tokens: Some(9),
    })
    .provider_generation_id("generation-123")
    .expect("generation ID")
    .estimated_cost(MonetaryCost::new("0.00125", "USD").expect("cost"))
    .build()
    .expect("provenance")
}

#[derive(Clone, PartialEq, Serialize, Deserialize)]
struct ClassifierInput {
    body: String,
}

#[derive(Clone, PartialEq, Serialize, Deserialize)]
struct Classification {
    label: String,
    confidence: f64,
}

#[test]
fn typed_intent_envelopes_round_trip() {
    let context = digest("I need help");
    let request = llm_provenance::IntentRequest {
        model: "provider/model".to_owned(),
        prompt: VersionedPrompt::new("intent-classifier", 3).expect("prompt"),
        context: context.clone(),
        input: ClassifierInput {
            body: "I need help".to_owned(),
        },
    };
    let request_json = serde_json::to_string(&request).expect("request JSON");
    let request_round_trip: llm_provenance::IntentRequest<ClassifierInput> =
        serde_json::from_str(&request_json).expect("request");
    assert!(request_round_trip == request);

    let response = llm_provenance::IntentResponse {
        classification: Classification {
            label: "support".to_owned(),
            confidence: 0.98,
        },
        provenance: provenance(context),
    };
    let response_json = serde_json::to_string(&response).expect("response JSON");
    let response_round_trip: llm_provenance::IntentResponse<Classification> =
        serde_json::from_str(&response_json).expect("response");
    assert!(response_round_trip == response);
}

#[test]
fn provenance_detects_context_changes_and_verifies_rebuilds() {
    let first = digest("first");
    let second = digest("second");
    let value = provenance(second.clone());
    assert!(!value.context_changed());

    let changed = GenerationProvenance::builder(
        value.model().clone(),
        value.prompt().clone(),
        second.clone(),
        RetainedGenerationArtifacts::new(
            value.context_preimage().clone(),
            value.decoding_config().clone(),
            value.provider_response().clone(),
            value.output().clone(),
        ),
        value.execution().clone(),
    )
    .observed_context(first)
    .build()
    .expect("changed provenance");
    assert!(changed.context_changed());

    let rebuilt = Context::new(
        SchemaId::new("example.intent").expect("schema"),
        SchemaVersion::new(1).expect("version"),
        json!({"input": "second"}),
    );
    assert!(matches!(
        changed.verify_context(&rebuilt).expect("verify"),
        ContextVerification::Match
    ));
}

#[test]
fn provenance_serialization_contains_references_not_raw_evidence() {
    let raw_context = "private customer message";
    let raw_prompt = "system secret prompt";
    let raw_output = "private generated output";
    let context = digest(raw_context);
    let value = provenance(context);
    let encoded = serde_json::to_string(&value).expect("provenance JSON");

    for forbidden in [raw_context, raw_prompt, raw_output, "raw provider response"] {
        assert!(!encoded.contains(forbidden), "found {forbidden}");
    }
    assert!(encoded.contains("\"format_version\":1"));
    assert!(encoded.contains("archive/context-preimage"));
    assert!(value.output().verify_bytes(b"normalized output"));
    assert!(!value.output().verify_bytes(b"modified output"));
}

#[test]
fn provenance_fingerprint_is_stable_and_input_sensitive() {
    let context = digest("I need help");
    let value = provenance(context.clone());
    let first = value.fingerprint().expect("fingerprint");
    assert_eq!(
        first.to_string(),
        "sha256:rfc8785:generation-provenance-v1:12d4a22ebe784359ac76ea04532ca58d4ef3eb71f1203d9b3427911bd5081f0e"
    );
    let wire = serde_json::to_string(&value).expect("wire");
    let decoded: GenerationProvenance = serde_json::from_str(&wire).expect("decode");
    assert_eq!(
        decoded.fingerprint().expect("round-trip fingerprint"),
        first
    );
    assert_eq!(
        GenerationProvenanceFingerprint::from_str(&first.to_string()).expect("text fingerprint"),
        first
    );

    let baseline = EvidenceInputs::default();
    for inputs in [
        EvidenceInputs {
            template: b"changed template",
            ..baseline
        },
        EvidenceInputs {
            request: b"changed rendered request",
            ..baseline
        },
        EvidenceInputs {
            model_revision: "2026-08-01",
            ..baseline
        },
        EvidenceInputs {
            decoding_config: b"{\"temperature\":1}",
            ..baseline
        },
        EvidenceInputs {
            tool_config: b"changed tool configuration",
            ..baseline
        },
        EvidenceInputs {
            output_schema: b"changed structured output schema",
            ..baseline
        },
        EvidenceInputs {
            seed: 18,
            ..baseline
        },
        EvidenceInputs {
            provider_response: b"changed raw provider response",
            ..baseline
        },
        EvidenceInputs {
            output: b"changed normalized output",
            ..baseline
        },
        EvidenceInputs {
            attempt: 2,
            ..baseline
        },
    ] {
        let changed = provenance_with(context.clone(), inputs);
        assert_ne!(changed.fingerprint().expect("changed fingerprint"), first);
    }
}

#[test]
fn provenance_wire_format_is_closed_and_strict() {
    let value = provenance(digest("I need help"));
    let mut wire = serde_json::to_value(value).expect("wire value");
    let object = wire.as_object_mut().expect("record object");
    object.insert("unexpected".to_owned(), json!(true));
    assert!(serde_json::from_value::<GenerationProvenance>(wire).is_err());

    let mut unsupported = serde_json::to_value(provenance(digest("I need help"))).expect("wire");
    unsupported["format_version"] = json!(2);
    assert!(serde_json::from_value::<GenerationProvenance>(unsupported).is_err());

    let mut malformed = serde_json::to_value(provenance(digest("I need help"))).expect("wire");
    malformed["execution"]["captured_at"] = json!("not-a-time");
    assert!(serde_json::from_value::<GenerationProvenance>(malformed).is_err());

    let mut empty = serde_json::to_value(provenance(digest("I need help"))).expect("wire");
    empty["execution"]["operation_id"] = Value::String(" ".to_owned());
    assert!(serde_json::from_value::<GenerationProvenance>(empty).is_err());

    let mut nested_unknown = serde_json::to_value(provenance(digest("I need help"))).expect("wire");
    nested_unknown["context"]["unexpected"] = json!(true);
    assert!(serde_json::from_value::<GenerationProvenance>(nested_unknown).is_err());

    let mut partial = serde_json::to_value(provenance(digest("I need help"))).expect("wire");
    partial
        .as_object_mut()
        .expect("record object")
        .remove("output");
    assert!(serde_json::from_value::<GenerationProvenance>(partial).is_err());
}

#[test]
fn provenance_rejects_non_i_json_numbers_before_fingerprinting() {
    let value = provenance(digest("I need help"));
    let unsafe_seed = GenerationProvenance::builder(
        value.model().clone(),
        value.prompt().clone(),
        value.context().clone(),
        RetainedGenerationArtifacts::new(
            value.context_preimage().clone(),
            value.decoding_config().clone(),
            value.provider_response().clone(),
            value.output().clone(),
        ),
        value.execution().clone(),
    )
    .seed(9_007_199_254_740_992)
    .build();
    assert!(unsafe_seed.is_err());

    let mut unsafe_usage = serde_json::to_value(value).expect("wire");
    unsafe_usage["usage"]["input_tokens"] = json!(9_007_199_254_740_992_u64);
    assert!(serde_json::from_value::<GenerationProvenance>(unsafe_usage).is_err());
}

#[test]
fn prompt_cost_and_timestamp_validation_is_enforced() {
    assert!(VersionedPrompt::new("", 1).is_err());
    assert!(VersionedPrompt::new("valid", 0).is_err());
    assert!(AuditTimestamp::parse_rfc3339("not-a-time").is_err());
    assert_eq!(
        AuditTimestamp::parse_rfc3339("2026-07-11T16:30:00-04:00")
            .expect("timestamp")
            .as_str(),
        "2026-07-11T20:30:00Z"
    );

    assert!(MonetaryCost::new("1.25", "USD").is_ok());
    for (amount, currency) in [
        ("1e-3", "USD"),
        ("+1", "USD"),
        ("01.0", "USD"),
        ("1.", "USD"),
        ("1", "usd"),
    ] {
        assert!(MonetaryCost::new(amount, currency).is_err());
    }
}
