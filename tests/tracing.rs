#![cfg(feature = "tracing")]

use std::collections::BTreeMap;
use std::fmt;
use std::sync::{Arc, Mutex};

#[cfg(feature = "sensitive-diagnostics")]
use llm_provenance::TraceOptions;
use llm_provenance::{
    ArtifactDigest, ArtifactLocator, ArtifactReference, AuditTimestamp, CacheProvenance, Context,
    ExecutionEvidence, GenerationProvenance, IntentRequest, IntentResponse, ModelIdentity,
    MonetaryCost, PromptEvidence, RetainedGenerationArtifacts, SchemaId, SchemaVersion, TokenUsage,
    VersionedComponent, VersionedPrompt,
};
use serde::ser::Error as _;
use serde::{Serialize, Serializer};
use serde_json::json;
use tracing::field::{Field, Visit};
use tracing::{Event, Id, Subscriber};
use tracing_subscriber::Layer;
use tracing_subscriber::layer::{Context as LayerContext, SubscriberExt};
use tracing_subscriber::registry::LookupSpan;

const TRACE_TARGET: &str = "llm_provenance::trace";
const SENSITIVE_TARGET: &str = "llm_provenance::sensitive";

#[derive(Clone, Debug)]
struct CapturedEvent {
    target: String,
    fields: BTreeMap<String, String>,
}

#[derive(Clone, Debug)]
struct CapturedSpan {
    target: String,
    name: String,
}

#[derive(Clone, Default)]
struct Capture {
    events: Arc<Mutex<Vec<CapturedEvent>>>,
    spans: Arc<Mutex<Vec<CapturedSpan>>>,
}

impl Capture {
    fn has_event(&self, target: &str, event: &str) -> bool {
        self.events.lock().unwrap().iter().any(|captured| {
            captured.target == target
                && captured
                    .fields
                    .get("event")
                    .is_some_and(|value| value == event)
        })
    }

    fn has_span(&self, target: &str, name: &str) -> bool {
        self.spans
            .lock()
            .unwrap()
            .iter()
            .any(|span| span.target == target && span.name == name)
    }

    fn output(&self) -> String {
        format!(
            "events={:#?}\nspans={:#?}",
            self.events.lock().unwrap(),
            self.spans.lock().unwrap()
        )
    }
}

#[derive(Default)]
struct FieldVisitor {
    fields: BTreeMap<String, String>,
}

impl Visit for FieldVisitor {
    fn record_bool(&mut self, field: &Field, value: bool) {
        self.fields
            .insert(field.name().to_owned(), value.to_string());
    }

    fn record_i64(&mut self, field: &Field, value: i64) {
        self.fields
            .insert(field.name().to_owned(), value.to_string());
    }

    fn record_u64(&mut self, field: &Field, value: u64) {
        self.fields
            .insert(field.name().to_owned(), value.to_string());
    }

    fn record_str(&mut self, field: &Field, value: &str) {
        self.fields
            .insert(field.name().to_owned(), value.to_owned());
    }

    fn record_debug(&mut self, field: &Field, value: &dyn fmt::Debug) {
        self.fields
            .insert(field.name().to_owned(), format!("{value:?}"));
    }
}

struct CaptureLayer {
    capture: Capture,
}

impl<S> Layer<S> for CaptureLayer
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    fn on_new_span(
        &self,
        attrs: &tracing::span::Attributes<'_>,
        _id: &Id,
        _ctx: LayerContext<'_, S>,
    ) {
        let target = attrs.metadata().target();
        if target == TRACE_TARGET || target == SENSITIVE_TARGET {
            self.capture.spans.lock().unwrap().push(CapturedSpan {
                target: target.to_owned(),
                name: attrs.metadata().name().to_owned(),
            });
        }
    }

    fn on_event(&self, event: &Event<'_>, _ctx: LayerContext<'_, S>) {
        let target = event.metadata().target();
        if target != TRACE_TARGET && target != SENSITIVE_TARGET {
            return;
        }
        let mut visitor = FieldVisitor::default();
        event.record(&mut visitor);
        self.capture.events.lock().unwrap().push(CapturedEvent {
            target: target.to_owned(),
            fields: visitor.fields,
        });
    }
}

fn capture() -> (Capture, impl Drop) {
    let capture = Capture::default();
    let subscriber = tracing_subscriber::registry().with(CaptureLayer {
        capture: capture.clone(),
    });
    let guard = tracing::subscriber::set_default(subscriber);
    (capture, guard)
}

fn context(secret: &str) -> Context<serde_json::Value> {
    Context::new(
        SchemaId::new(format!("schema-{secret}")).unwrap(),
        SchemaVersion::new(3).unwrap(),
        json!({"message": format!("payload-{secret}")}),
    )
}

fn artifact(locator: &str, bytes: impl AsRef<[u8]>) -> ArtifactReference {
    ArtifactReference::new(
        ArtifactLocator::new(locator).unwrap(),
        ArtifactDigest::from_bytes(bytes),
    )
}

fn provenance(context: llm_provenance::ContextDigest, secret: &str) -> GenerationProvenance {
    let prompt = PromptEvidence::new(
        VersionedPrompt::new(format!("prompt-{secret}"), 4).unwrap(),
        artifact("archive/template", format!("template-{secret}")),
        VersionedComponent::new("renderer", "git:1").unwrap(),
        artifact("archive/request", format!("request-{secret}")),
    );
    let execution = ExecutionEvidence::new(
        VersionedComponent::new("app", "git:2").unwrap(),
        "operation-1",
        1,
        AuditTimestamp::parse_rfc3339("2026-07-11T20:30:00Z").unwrap(),
    )
    .unwrap();
    GenerationProvenance::builder(
        ModelIdentity::new("provider", format!("model-{secret}"), "revision-1").unwrap(),
        prompt,
        context,
        RetainedGenerationArtifacts::new(
            artifact("archive/context", format!("context-{secret}")),
            artifact("archive/config", format!("config-{secret}")),
            artifact("archive/response", format!("response-{secret}")),
            artifact("archive/output", format!("output-{secret}")),
        ),
        execution,
    )
    .cache(CacheProvenance::with_key(true, format!("cache-{secret}")).unwrap())
    .usage(TokenUsage {
        input_tokens: Some(12),
        output_tokens: Some(7),
    })
    .provider_generation_id(format!("generation-{secret}"))
    .unwrap()
    .estimated_cost(MonetaryCost::new("0.001", "USD").unwrap())
    .build()
    .expect("provenance")
}

#[test]
fn redacted_context_events_exclude_payload_and_identifiers() {
    let (capture, _guard) = capture();
    let digest = context("secret").digest().unwrap();
    let output = capture.output();

    assert!(capture.has_event(TRACE_TARGET, "llm_provenance.operation.success"));
    assert!(output.contains("context.digest"), "{output}");
    assert!(output.contains(&digest.hex()), "{output}");
    assert!(capture.has_span(TRACE_TARGET, "llm_provenance.operation"));
    assert!(
        !output.contains("secret"),
        "redacted trace leaked: {output}"
    );
    assert!(
        !output.contains("payload"),
        "redacted trace leaked payload: {output}"
    );
}

struct SerializationSecret;

impl Serialize for SerializationSecret {
    fn serialize<S: Serializer>(&self, _serializer: S) -> Result<S::Ok, S::Error> {
        Err(S::Error::custom("serialization-secret"))
    }
}

#[test]
fn redacted_failure_events_use_error_kind_only() {
    let (capture, _guard) = capture();
    let value = Context::new(
        SchemaId::new("safe-schema").unwrap(),
        SchemaVersion::new(1).unwrap(),
        SerializationSecret,
    );

    assert!(value.digest().is_err());
    let output = capture.output();
    assert!(
        output.contains("\"error_kind\": \"serialization\""),
        "{output}"
    );
    assert!(!output.contains("serialization-secret"), "{output}");
}

#[test]
fn intent_serialization_is_side_effect_free_and_explicit_trace_is_redacted() {
    let context = context("secret");
    let digest = context.digest().unwrap();
    let (capture, _guard) = capture();
    let request = IntentRequest {
        model: "model-secret".to_owned(),
        prompt: VersionedPrompt::new("prompt-secret", 1).unwrap(),
        context: digest.clone(),
        input: json!({"input": "input-secret"}),
    };
    let response = IntentResponse {
        classification: json!({"classification": "classification-secret"}),
        provenance: provenance(digest, "secret"),
    };

    let _ = serde_json::to_string(&request).unwrap();
    let _ = serde_json::to_string(&response).unwrap();
    assert!(capture.events.lock().unwrap().is_empty());

    request.emit_trace().unwrap();
    response.emit_trace().unwrap();
    let output = capture.output();
    assert!(output.contains("intent.request.emit"), "{output}");
    assert!(output.contains("intent.response.emit"), "{output}");
    assert!(
        !output.contains("secret"),
        "redacted trace leaked: {output}"
    );
}

#[cfg(feature = "sensitive-diagnostics")]
#[test]
fn sensitive_tracing_requires_explicit_opt_in_and_emits_raw_envelopes() {
    let (capture, _guard) = capture();
    let context = context("secret");
    let digest = context.digest().unwrap();
    let before = capture.output();
    assert!(!before.contains(SENSITIVE_TARGET), "{before}");
    assert!(!before.contains("payload-secret"), "{before}");

    let options = TraceOptions::new().with_sensitive_tracing();
    context.digest_with_options(options).unwrap();

    let request = IntentRequest {
        model: "model-secret".to_owned(),
        prompt: VersionedPrompt::new("prompt-secret", 1).unwrap(),
        context: digest.clone(),
        input: json!({"input": "input-secret"}),
    };
    let response = IntentResponse {
        classification: json!({"classification": "classification-secret"}),
        provenance: provenance(digest, "secret"),
    };
    request.emit_trace_with_options(options).unwrap();
    response.emit_sensitive_trace().unwrap();

    let output = capture.output();
    assert!(output.contains(SENSITIVE_TARGET), "{output}");
    assert!(output.contains("payload-secret"), "{output}");
    assert!(output.contains("input-secret"), "{output}");
    assert!(output.contains("classification-secret"), "{output}");
}

#[cfg(feature = "sensitive-diagnostics")]
#[test]
fn sensitive_convenience_methods_cover_context_conversions() {
    let (capture, _guard) = capture();
    let context = context("conversion-secret");
    let dynamic = context.to_dynamic_with_sensitive_tracing().unwrap();
    let _: Context<serde_json::Value> = dynamic.try_into_typed_with_sensitive_tracing().unwrap();
    context.canonical_bytes_with_sensitive_tracing().unwrap();

    let output = capture.output();
    assert!(output.contains("conversion-secret"), "{output}");
    assert!(output.contains("context.to_dynamic"), "{output}");
    assert!(output.contains("context.try_into_typed"), "{output}");
    assert!(output.contains("context.canonical_bytes"), "{output}");
}
