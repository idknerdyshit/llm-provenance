# llm-provenance

`llm-provenance` provides provider-neutral building blocks for reproducible LLM
context hashes and audit-safe generation provenance. Applications keep ownership
of their context, evidence archive, and classifier schemas; this crate supplies
typed envelopes, RFC 8785 canonicalization, SHA-256 commitments, replay
verification, and versioned execution metadata.

The crate is synchronous and pure. It has no HTTP client, async runtime,
database, provider SDK, or application-specific schema. It emits optional
`tracing` events but never installs a subscriber; consuming applications own
subscriber setup and filtering.

## Observability

The `tracing` feature is enabled by default. Context operations emit structured
redacted events to the `llm_provenance::trace` target. Intent envelopes are
inert data structures, so callers explicitly invoke `emit_trace()` when they
want an intent request or response event.

Applications configure one app-wide subscriber. No subscriber is required
specifically for this crate:

```rust,no_run
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

tracing_subscriber::registry()
    .with(EnvFilter::new(
        "info,llm_provenance::trace=debug",
    ))
    .with(tracing_subscriber::fmt::layer())
    .init();
```

Normal events contain operation outcomes, elapsed time, schema versions,
opaque digest references, token accounting, cache state, and presence flags.
They do not contain context or intent payloads, canonical preimages, raw
application identifiers, or full error strings.

The non-default `sensitive-diagnostics` feature enables an explicit local
debugging escape hatch. It implies `tracing`, but raw events are still emitted
only when an operation receives `TraceOptions::with_sensitive_tracing()` or a
matching convenience method:

```text
llm-provenance = { version = "0.1", features = ["sensitive-diagnostics"] }

let digest = context.digest_with_sensitive_tracing()?;
request.emit_sensitive_trace()?;
```

Sensitive events are sent to `llm_provenance::sensitive` and may include
canonical context JSON, full serialized intent envelopes, and detailed
serialization errors. Never enable this target with a production log sink.

## Typed context hashing

```rust
use llm_provenance::{Context, SchemaId, SchemaVersion};
use serde::Serialize;

#[derive(Serialize)]
struct SupportContext<'a> {
    ticket_id: &'a str,
    latest_message: &'a str,
}

let context = Context::new(
    SchemaId::new("com.example.support-context")?,
    SchemaVersion::new(1)?,
    SupportContext {
        ticket_id: "ticket-123",
        latest_message: "I need help with my order",
    },
);

let digest = context.digest()?;
assert_eq!(digest.hex().len(), 64);
# Ok::<(), llm_provenance::Error>(())
```

`Context::canonical_bytes()` returns the exact hash preimage. It is a JCS
serialization of this envelope:

```json
{
  "domain": "llm-provenance/context-digest/v1",
  "schema": "com.example.support-context",
  "schema_version": 1,
  "payload": {}
}
```

The domain separator, schema identifier, and schema version are all hashed.
Schema versions are one-based and must be bumped whenever the meaning or hashed
shape of a payload changes.

## Dynamic JSON interoperability

```rust
use llm_provenance::{Context, DynamicContext, SchemaId, SchemaVersion};
use serde_json::json;

let context: DynamicContext = Context::new(
    SchemaId::new("com.example.dynamic")?,
    SchemaVersion::new(1)?,
    json!({"b": 2, "a": 1}),
);

let digest = context.digest()?;
assert_eq!(digest.schema().as_str(), "com.example.dynamic");
# Ok::<(), llm_provenance::Error>(())
```

Typed contexts can be converted with `Context::to_dynamic()`. A
`DynamicContext` can be converted back with `try_into_typed()`.

## Hash-bound reconstruction manifests

For reconstructable contexts, applications wrap the resolved payload in a
`ManifestedPayload`. The manifest binds ordered retained source snapshots,
retrieval/selection configuration commitments, and the exact context-builder
version into the context digest. The application retains the actual source and
configuration bytes; this crate does no storage or retrieval.

```rust
use llm_provenance::{
    ArtifactDigest, Context, ContextManifest, ManifestedPayload, SchemaId,
    SchemaVersion, SourceSnapshot, VersionedComponent,
};
use serde_json::json;

let manifest = ContextManifest::new(
    vec![SourceSnapshot::new(
        "ticket-123",
        "revision-7",
        ArtifactDigest::from_bytes(b"retained ticket snapshot"),
    )?],
    ArtifactDigest::from_bytes(b"retrieval-config-v1"),
    ArtifactDigest::from_bytes(b"selection-config-v1"),
    VersionedComponent::new("support-context-builder", "git:abc123")?,
);
let context = Context::new(
    SchemaId::new("com.example.support-context")?,
    SchemaVersion::new(1)?,
    ManifestedPayload::new(manifest, json!({"messages": ["Need help"]})),
);

let digest = context.digest()?;
let canonical_preimage = context.canonical_bytes()?;
assert!(digest.verify_canonical_bytes(&canonical_preimage)?.is_match());
# Ok::<(), llm_provenance::Error>(())
```

`ArtifactDigest` commits to exact raw bytes using a separate domain
(`llm-provenance/artifact-digest/v1`); it does not normalize JSON, Unicode, or
line endings. `ArtifactReference` pairs one of those commitments with an opaque
application-owned locator. Locators must never contain credentials.

## Generation provenance and retained evidence

```rust
use llm_provenance::{
    ArtifactDigest, ArtifactLocator, ArtifactReference, AuditTimestamp, Context,
    ExecutionEvidence, GenerationProvenance, ModelIdentity, PromptEvidence,
    RetainedGenerationArtifacts, SchemaId, SchemaVersion, TokenUsage,
    VersionedComponent, VersionedPrompt,
};
use serde_json::json;

fn evidence(locator: &str, bytes: &[u8]) -> llm_provenance::Result<ArtifactReference> {
    Ok(ArtifactReference::new(
        ArtifactLocator::new(locator)?,
        ArtifactDigest::from_bytes(bytes),
    ))
}

let context = Context::new(
    SchemaId::new("com.example.reply")?,
    SchemaVersion::new(1)?,
    json!({"conversation_revision": 8}),
);
let context_preimage = context.canonical_bytes()?;
let prompt = PromptEvidence::new(
    VersionedPrompt::new("support-reply", 4)?,
    evidence("evidence/prompt-template", b"retained template bytes")?,
    VersionedComponent::new("prompt-renderer", "git:abc123")?,
    evidence("evidence/rendered-request", b"exact rendered request bytes")?,
);
let execution = ExecutionEvidence::new(
    VersionedComponent::new("support-service", "git:def456")?,
    "operation-123",
    1,
    AuditTimestamp::parse_rfc3339("2026-07-11T20:30:00Z")?,
)?;

let provenance = GenerationProvenance::builder(
    ModelIdentity::new("provider", "model", "provider-revision-2026-07")?,
    prompt,
    context.digest()?,
    RetainedGenerationArtifacts::new(
        evidence("evidence/context-preimage", &context_preimage)?,
        evidence("evidence/decoding-config", br#"{"temperature":0}"#)?,
        evidence("evidence/provider-response", b"raw provider response bytes")?,
        evidence("evidence/output", b"normalized persisted output bytes")?,
    ),
    execution,
)
    .usage(TokenUsage {
        input_tokens: Some(120),
        output_tokens: Some(35),
    })
    .provider_generation_id("generation-123")?
    .build()?;

assert!(!provenance.context_changed());
assert!(provenance.verify_context(&context)?.is_match());
# Ok::<(), llm_provenance::Error>(())
```

`GenerationProvenance` is a strict `format_version: 1` record. It retains only
opaque locators and commitments for the canonical context preimage, template,
fully rendered request, decoding/tool/schema configuration, raw provider
response, and normalized output. It also carries immutable model revision,
application build, operation/attempt, and capture time. Its canonical,
domain-separated `fingerprint()` can be signed or added to an application-owned
append-only ledger.

Applications should retain the raw evidence archive separately, with their own
encryption, authorization, retention, deletion, legal-hold, and audited-read
controls. During audit, resolve each reference, verify its `ArtifactDigest`,
rebuild the `Context<ManifestedPayload<T>>`, and call
`provenance.verify_context(&rebuilt)`. This proves the archived evidence and
rebuilt context match the record; it does not claim a new model invocation will
reproduce a nondeterministic output.

## Application-defined intent results

```rust
use llm_provenance::IntentResponse;
use serde::{Deserialize, Serialize};

#[derive(Clone, PartialEq, Serialize, Deserialize)]
struct IntentResult {
    labels: Vec<String>,
    confidence: f64,
}

fn labels(response: &IntentResponse<IntentResult>) -> &[String] {
    &response.classification.labels
}
# let _ = labels;
```

`IntentRequest<T>` and `IntentResponse<T>` are generic so a provider seam can
stay independent of an application's classifier schema. Dynamic aliases using
`serde_json::Value` are available for JSON-first integrations.

## Hash compatibility contract

- The canonical preimage is RFC 8785 JSON Canonicalization Scheme output.
- SHA-256 is the only algorithm in version 0.1.
- The domain separator is `llm-provenance/context-digest/v1`.
- Raw retained artifacts use the independent
  `llm-provenance/artifact-digest/v1` domain.
- Values are restricted to interoperable I-JSON numbers. Integers outside
  `±(2^53−1)`, NaN, and infinities fail instead of being rounded or serialized
  as `null`.
- Large identifiers and exact monetary values should be encoded as strings.
- No Unicode normalization is performed; distinct Unicode sequences remain
  distinct inputs.
- A change to domain separation or canonicalization is breaking. An
  application payload change requires a new application schema version.

The text form of a digest is:

```text
sha256:rfc8785:<schema>:<schema-version>:<64 lowercase hex characters>
```

## MSRV and license

The minimum supported Rust version is 1.88. The project is dual licensed under
MIT or Apache-2.0 at your option.
