# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/), and this
project adheres to [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added

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

## [0.1.0] - 2026-07-10

### Added

- Typed and dynamic versioned context envelopes.
- RFC 8785 canonical JSON with fail-closed I-JSON number validation.
- Structured SHA-256 context digests with stable JSON and text encodings.
- Audit-safe generation, cache, token, prompt, and monetary provenance types.
- Generic typed and dynamic intent-classification envelopes.
