use llm_provenance::{
    CacheProvenance, Context, GenerationProvenance, IntentRequest, IntentResponse, MonetaryCost,
    SchemaId, SchemaVersion, TokenUsage, VersionedPrompt,
};
use serde::{Deserialize, Serialize};
use serde_json::json;

fn digest(value: &str) -> llm_provenance::ContextDigest {
    Context::new(
        SchemaId::new("example.intent").expect("schema"),
        SchemaVersion::new(1).expect("version"),
        json!({"input": value}),
    )
    .digest()
    .expect("digest")
}

fn provenance(context: llm_provenance::ContextDigest) -> GenerationProvenance {
    GenerationProvenance {
        model: "provider/model".to_owned(),
        prompt: VersionedPrompt::new("intent-classifier", 3).expect("prompt"),
        context,
        observed_context: None,
        cache: CacheProvenance {
            key: Some("cache-digest".to_owned()),
            hit: false,
        },
        usage: TokenUsage {
            input_tokens: Some(42),
            output_tokens: Some(9),
        },
        provider_generation_id: Some("generation-123".to_owned()),
        estimated_cost: Some(MonetaryCost::new("0.00125", "USD").expect("cost")),
    }
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
    let request = IntentRequest {
        model: "provider/model".to_owned(),
        prompt: VersionedPrompt::new("intent-classifier", 3).expect("prompt"),
        context: context.clone(),
        input: ClassifierInput {
            body: "I need help".to_owned(),
        },
    };
    let request_json = serde_json::to_string(&request).expect("request JSON");
    let request_round_trip: IntentRequest<ClassifierInput> =
        serde_json::from_str(&request_json).expect("request");
    assert!(request_round_trip == request);

    let response = IntentResponse {
        classification: Classification {
            label: "support".to_owned(),
            confidence: 0.98,
        },
        provenance: provenance(context),
    };
    let response_json = serde_json::to_string(&response).expect("response JSON");
    let response_round_trip: IntentResponse<Classification> =
        serde_json::from_str(&response_json).expect("response");
    assert!(response_round_trip == response);
}

#[test]
fn dynamic_intent_aliases_round_trip() {
    let request: llm_provenance::DynamicIntentRequest = IntentRequest {
        model: "provider/model".to_owned(),
        prompt: VersionedPrompt::new("intent-classifier", 1).expect("prompt"),
        context: digest("hello"),
        input: json!({"body": "hello"}),
    };
    let encoded = serde_json::to_vec(&request).expect("dynamic request JSON");
    let decoded: llm_provenance::DynamicIntentRequest =
        serde_json::from_slice(&encoded).expect("dynamic request");
    assert!(decoded == request);
}

#[test]
fn provenance_detects_context_changes() {
    let first = digest("first");
    let second = digest("second");
    let mut value = provenance(second.clone());
    assert!(!value.context_changed());
    value.observed_context = Some(second);
    assert!(!value.context_changed());
    value.observed_context = Some(first);
    assert!(value.context_changed());
}

#[test]
fn provenance_serialization_excludes_sensitive_payloads() {
    let encoded = serde_json::to_string(&provenance(digest("private customer message")))
        .expect("provenance JSON");
    for forbidden in [
        "private customer message",
        "system_prompt",
        "user_prompt",
        "api_key",
        "credential",
        "payload",
    ] {
        assert!(!encoded.contains(forbidden), "found {forbidden}");
    }
}

#[test]
fn prompt_and_cost_validation_is_enforced_during_decode() {
    assert!(VersionedPrompt::new("", 1).is_err());
    assert!(VersionedPrompt::new("valid", 0).is_err());
    assert!(serde_json::from_str::<VersionedPrompt>(r#"{"id":"","version":1}"#).is_err());

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
    assert!(serde_json::from_str::<MonetaryCost>(r#"{"amount":"1e-3","currency":"USD"}"#).is_err());
}
