# llm-provenance

`llm-provenance` provides provider-neutral building blocks for reproducible LLM
context hashes and audit-safe generation provenance. Applications keep ownership
of their context and classifier schemas; this crate supplies typed envelopes,
RFC 8785 canonicalization, SHA-256 digests, and generation metadata.

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

## Generation provenance

```rust
use llm_provenance::{
    CacheProvenance, Context, GenerationProvenance, SchemaId, SchemaVersion,
    TokenUsage, VersionedPrompt,
};
use serde_json::json;

let context = Context::new(
    SchemaId::new("com.example.reply")?,
    SchemaVersion::new(1)?,
    json!({"conversation_revision": 8}),
);

let provenance = GenerationProvenance {
    model: "provider/model".to_owned(),
    prompt: VersionedPrompt::new("support-reply", 4)?,
    context: context.digest()?,
    observed_context: None,
    cache: CacheProvenance::default(),
    usage: TokenUsage {
        input_tokens: Some(120),
        output_tokens: Some(35),
    },
    provider_generation_id: Some("generation-123".to_owned()),
    estimated_cost: None,
};

assert!(!provenance.context_changed());
# Ok::<(), llm_provenance::Error>(())
```

Provenance intentionally contains no rendered prompt, raw context, generated
body, or provider credential. Applications should persist the provenance next
to their own artifact and retain raw data according to their own privacy policy.

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
