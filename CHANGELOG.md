# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/), and this
project adheres to [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added

- Hash-bound context reconstruction manifests, exact-byte artifact commitments,
  and opaque evidence references.
- Replay verification for rebuilt contexts and archived canonical preimages.
- Strict version-1 generation evidence with retained context/request/response/
  output references, immutable model/build identity, and canonical record
  fingerprints for application-managed signing or ledgers.
- Default-enabled redacted `tracing` instrumentation with app-owned subscriber
  setup and target filtering.
- Feature-gated, explicitly opt-in sensitive tracing for context preimages and
  intent envelopes.

### Fixed

- Validate I-JSON numbers during the same serialization pass that constructs the
  canonical JSON value, preventing stateful serializers from bypassing the
  interoperable integer range.
- Reject non-canonical context-digest text encodings, including uppercase
  hexadecimal and signed or zero-padded schema versions.
- Reject generation-provenance records whose numeric metadata cannot be
  represented as interoperable I-JSON before construction or deserialization.

### Changed

- Replace the unpublished pre-release `GenerationProvenance` shape with the
  strict versioned audit record and validated builder API.

## [0.1.0] - 2026-07-10

### Added

- Typed and dynamic versioned context envelopes.
- RFC 8785 canonical JSON with fail-closed I-JSON number validation.
- Structured SHA-256 context digests with stable JSON and text encodings.
- Audit-safe generation, cache, token, prompt, and monetary provenance types.
- Generic typed and dynamic intent-classification envelopes.
