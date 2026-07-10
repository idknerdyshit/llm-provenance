# llm-provenance Agent Guide

## Purpose

`llm-provenance` is a pure Rust library for provider-neutral LLM context
hashing and audit-safe generation provenance. Applications define their own
context and intent payloads. This crate owns the versioned envelope, canonical
hash contract, structured digest, and provenance metadata around those payloads.

The public API is re-exported from `src/lib.rs` and implemented in focused
modules:

- `context`: typed/dynamic context envelopes, schema identifiers and versions,
  canonical preimage construction, and hashing.
- `digest`: structured digest metadata plus stable JSON and text encodings.
- `intent`: generic typed and dynamic classifier request/response envelopes.
- `provenance`: model, prompt, cache, usage, provider ID, and exact-cost metadata.
- `validate`: internal fail-closed I-JSON number validation.
- `error`: the public non-exhaustive error type.

## Hard Invariants

1. Keep the crate pure and provider-neutral. Do not add HTTP clients, provider
   SDKs, async runtimes, databases, filesystem access, application schemas, or
   framework integrations.
2. The context hash domain is exactly
   `llm-provenance/context-digest/v1`. Changing it changes every digest and is a
   breaking protocol change requiring a new major version and migration plan.
3. Context hash input is the RFC 8785 JCS encoding of the domain-separated
   envelope containing `domain`, `schema`, `schema_version`, and `payload`.
   SHA-256 is the only supported algorithm in version 0.1.
4. Canonicalization fails closed before hashing. Reject NaN, infinities, and
   integers outside `±(2^53-1)`; never silently round them or convert them to
   `null`. Large identifiers and exact numeric values belong in strings.
5. Schema identifiers and versions are hash inputs. Schema versions are
   one-based and applications must bump them whenever payload meaning or hashed
   shape changes.
6. Do not normalize Unicode. RFC 8785/JCS property ordering and escaping are the
   canonical contract; distinct Unicode sequences remain distinct inputs.
7. Provenance must never contain raw context, rendered prompts, generated
   bodies, credentials, authorization data, or provider secrets. It may contain
   only stable references, digests, cache metadata, token accounting, provider
   generation IDs, and exact cost metadata.
8. Context and intent envelopes intentionally omit payload-revealing `Debug`
   implementations. Do not add derived or custom `Debug` output that exposes
   their generic payloads.

## Public API and Compatibility

- Treat `Context`, `ContextDigest`, their serialized forms, the digest text
  format, and the hash preimage as protocol APIs, not implementation details.
- Preserve the text format
  `sha256:rfc8785:<schema>:<schema-version>:<64 lowercase hex characters>`.
- Keep constructors and deserializers equally strict so invalid values cannot
  bypass validation through JSON.
- Keep typed generic envelopes primary and maintain the `serde_json::Value`
  dynamic aliases and conversion helpers.
- Adding fields to a serialized public struct or changing enum spellings can be
  breaking even when Rust source compatibility appears intact.
- Public API changes require rustdoc, README examples where useful, changelog
  entries, and tests that lock serialization or digest behavior.
- Do not lower the MSRV below or raise it above Rust 1.88 without an explicit
  release decision and documentation update.

## Dependency Rules

Direct dependencies are limited to pure serialization, canonicalization,
hashing, hexadecimal encoding, and error crates. New dependencies need a clear
reason tied to the core contract and must not introduce I/O, networking,
runtime, platform, or provider coupling.

## Testing

Changes to hashing or serialization must cover:

- Golden canonical-byte and SHA-256 fixtures.
- RFC 8785 property ordering, Unicode/escaping, and number formatting.
- Typed/dynamic digest equivalence.
- Schema, version, and payload sensitivity.
- Unsafe-number and malformed-digest rejection.
- Serde round trips for digest, provenance, and intent types.
- Provenance redaction and observed/final context-change behavior.

Run all release checks with the pinned toolchain:

```sh
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
cargo doc --locked --no-deps
cargo package --locked
cargo publish --dry-run --locked
```

Keep the working tree clean before the package and publish dry runs. Never run
a real `cargo publish` unless the user explicitly requests a crates.io release.

