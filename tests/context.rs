use std::cell::Cell;
use std::collections::BTreeMap;
use std::str::FromStr;

use llm_provenance::{
    ArtifactDigest, Context, ContextDigest, ContextManifest, ContextVerification, Error,
    ManifestedPayload, SchemaId, SchemaVersion, SourceSnapshot, VersionedComponent,
};
use serde::{Serialize, Serializer};
use serde_json::json;

fn schema(value: &str) -> SchemaId {
    SchemaId::new(value).expect("valid test schema")
}

fn version(value: u32) -> SchemaVersion {
    SchemaVersion::new(value).expect("valid test version")
}

#[derive(Serialize)]
struct TypedPayload<'a> {
    name: &'a str,
    count: u32,
}

struct ChangesBetweenSerializations {
    calls: Cell<u8>,
}

impl ChangesBetweenSerializations {
    fn new() -> Self {
        Self {
            calls: Cell::new(0),
        }
    }
}

impl Serialize for ChangesBetweenSerializations {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let call = self.calls.get();
        self.calls.set(call + 1);
        if call == 0 {
            serializer.serialize_u64(1)
        } else {
            serializer.serialize_u64(9_007_199_254_740_992)
        }
    }
}

#[test]
fn canonical_bytes_and_digest_are_locked() {
    let context = Context::new(
        schema("com.example.message"),
        version(1),
        TypedPayload {
            name: "Sam",
            count: 2,
        },
    );

    let canonical = context.canonical_bytes().expect("canonical context");
    assert_eq!(
        String::from_utf8(canonical).expect("UTF-8 JSON"),
        r#"{"domain":"llm-provenance/context-digest/v1","payload":{"count":2,"name":"Sam"},"schema":"com.example.message","schema_version":1}"#
    );
    assert_eq!(
        context.digest().expect("digest").hex(),
        "82abdd6ad165dfcaf0f3eafba4c2da0349edbcb698f9cd5fc4906eaae354bf6c"
    );
}

#[test]
fn typed_and_dynamic_contexts_have_identical_hashes() {
    let context = Context::new(
        schema("com.example.message"),
        version(1),
        TypedPayload {
            name: "Sam",
            count: 2,
        },
    );
    let dynamic = context.to_dynamic().expect("dynamic context");
    assert_eq!(
        context.canonical_bytes().expect("typed bytes"),
        dynamic.canonical_bytes().expect("dynamic bytes")
    );
    assert_eq!(
        context.digest().expect("typed digest"),
        dynamic.digest().expect("dynamic digest")
    );
}

#[test]
fn dynamic_json_order_and_whitespace_do_not_change_hash() {
    let first: serde_json::Value =
        serde_json::from_str(r#"{"z": 1, "a": { "last": false, "first": true }}"#)
            .expect("first JSON");
    let second: serde_json::Value = serde_json::from_str(
        r#"{
            "a":{"first":true,"last":false},
            "z":1
        }"#,
    )
    .expect("second JSON");
    let first = Context::new(schema("example.order"), version(1), first);
    let second = Context::new(schema("example.order"), version(1), second);
    assert_eq!(
        first.digest().expect("first digest"),
        second.digest().expect("second digest")
    );
}

#[test]
fn schema_version_and_payload_are_digest_inputs() {
    let base = Context::new(schema("example.input"), version(1), json!({"value": 1}));
    let changed_schema = Context::new(schema("example.other"), version(1), json!({"value": 1}));
    let changed_version = Context::new(schema("example.input"), version(2), json!({"value": 1}));
    let changed_payload = Context::new(schema("example.input"), version(1), json!({"value": 2}));
    let base = base.digest().expect("base digest");
    assert_ne!(base, changed_schema.digest().expect("schema digest"));
    assert_ne!(base, changed_version.digest().expect("version digest"));
    assert_ne!(base, changed_payload.digest().expect("payload digest"));
}

#[test]
fn typed_dynamic_typed_round_trip_preserves_metadata() {
    #[derive(Debug, Serialize, serde::Deserialize, PartialEq)]
    struct OwnedPayload {
        label: String,
    }

    let context = Context::new(
        schema("example.roundtrip"),
        version(3),
        OwnedPayload {
            label: "support".to_owned(),
        },
    );
    let expected_digest = context.digest().expect("original digest");
    let restored: Context<OwnedPayload> = context
        .to_dynamic()
        .expect("dynamic")
        .try_into_typed()
        .expect("typed");
    assert_eq!(restored.schema().as_str(), "example.roundtrip");
    assert_eq!(restored.schema_version().get(), 3);
    assert_eq!(restored.payload().label, "support");
    assert_eq!(restored.digest().expect("restored digest"), expected_digest);
}

#[test]
fn rejects_invalid_schema_metadata() {
    assert!(matches!(SchemaId::new(""), Err(Error::InvalidSchemaId(_))));
    assert!(matches!(
        SchemaId::new("contains:delimiter"),
        Err(Error::InvalidSchemaId(_))
    ));
    assert_eq!(SchemaVersion::new(0), Err(Error::InvalidSchemaVersion));
}

#[test]
fn rejects_non_finite_and_unsafe_numbers() {
    for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        let context = Context::new(schema("example.float"), version(1), value);
        assert!(matches!(
            context.digest(),
            Err(Error::InvalidIJsonNumber(_))
        ));
    }

    let too_large = Context::new(
        schema("example.integer"),
        version(1),
        9_007_199_254_740_992_u64,
    );
    let too_small = Context::new(
        schema("example.integer"),
        version(1),
        -9_007_199_254_740_992_i64,
    );
    assert!(matches!(
        too_large.digest(),
        Err(Error::InvalidIJsonNumber(_))
    ));
    assert!(matches!(
        too_small.digest(),
        Err(Error::InvalidIJsonNumber(_))
    ));
}

#[test]
fn validation_and_json_conversion_use_one_serialization_pass() {
    let canonical_payload = ChangesBetweenSerializations::new();
    let canonical_context =
        Context::new(schema("example.single-pass"), version(1), canonical_payload);
    let canonical = String::from_utf8(
        canonical_context
            .canonical_bytes()
            .expect("first serialization is safe"),
    )
    .expect("UTF-8 JSON");
    assert!(canonical.contains(r#""payload":1"#));
    assert_eq!(canonical_context.payload().calls.get(), 1);

    let dynamic_payload = ChangesBetweenSerializations::new();
    let dynamic_context = Context::new(schema("example.single-pass"), version(1), dynamic_payload);
    let dynamic = dynamic_context
        .to_dynamic()
        .expect("first serialization is safe");
    assert_eq!(dynamic.payload(), &json!(1));
    assert_eq!(dynamic_context.payload().calls.get(), 1);
}

#[test]
fn rfc_8785_number_string_and_property_order_vector() {
    let value = json!({
        "numbers": [333_333_333.333_333_3_f64, 1E30_f64, 4.50_f64, 2e-3_f64, 1e-27_f64],
        "string": "€$\u{000f}\nA'B\"\\\\\"/",
        "literals": [serde_json::Value::Null, json!(true), json!(false)]
    });
    let context = Context::new(schema("rfc8785.example"), version(1), value);
    let canonical = String::from_utf8(context.canonical_bytes().expect("canonical RFC vector"))
        .expect("UTF-8 JSON");
    assert!(canonical.contains(
        r#""payload":{"literals":[null,true,false],"numbers":[333333333.3333333,1e+30,4.5,0.002,1e-27],"string":"€$\u000f\nA'B\"\\\\\"/"}"#
    ));
}

#[test]
fn digest_text_and_json_round_trip_strictly() {
    let digest = Context::new(
        schema("example.digest"),
        version(7),
        BTreeMap::from([("a", 1)]),
    )
    .digest()
    .expect("digest");
    let text = digest.to_string();
    assert_eq!(ContextDigest::from_str(&text).expect("text digest"), digest);
    let json = serde_json::to_string(&digest).expect("digest JSON");
    assert_eq!(
        serde_json::from_str::<ContextDigest>(&json).expect("JSON digest"),
        digest
    );

    for invalid in [
        "sha1:rfc8785:example.digest:7:00",
        "sha256:rfc8785:example.digest:0:00",
        "sha256:rfc8785:example.digest:7:not-hex",
        "sha256:rfc8785:example.digest:7:00",
    ] {
        assert!(ContextDigest::from_str(invalid).is_err(), "{invalid}");
    }

    let (prefix, hexadecimal) = text.rsplit_once(':').expect("digest delimiter");
    let uppercase_hex = format!("{prefix}:{}", hexadecimal.to_uppercase());
    assert!(ContextDigest::from_str(&uppercase_hex).is_err());
    assert!(ContextDigest::from_str(&text.replacen(":7:", ":07:", 1)).is_err());
    assert!(ContextDigest::from_str(&text.replacen(":7:", ":+7:", 1)).is_err());

    let unsupported = json.replace("sha256", "future-hash");
    assert!(serde_json::from_str::<ContextDigest>(&unsupported).is_err());
}

#[test]
fn artifact_digest_commits_to_exact_raw_bytes() {
    let digest = ArtifactDigest::from_bytes(b"source snapshot");
    assert_eq!(
        digest.to_string(),
        "sha256:bytes:v1:ecf495d855430f121746d2f133e19e20518d429bb53176a7960bf59893c71e34"
    );
    assert!(digest.verify_bytes(b"source snapshot"));
    assert!(!digest.verify_bytes(b"source snapshot\n"));
    assert_eq!(
        ArtifactDigest::from_str(&digest.to_string()).expect("text digest"),
        digest
    );
    let json = serde_json::to_string(&digest).expect("JSON digest");
    assert_eq!(
        serde_json::from_str::<ArtifactDigest>(&json).expect("decoded digest"),
        digest
    );
    let uppercase = digest.to_string().to_uppercase();
    assert!(ArtifactDigest::from_str(&uppercase).is_err());
}

fn manifest(first: &str, second: &str, builder_version: &str) -> ContextManifest {
    manifest_with(
        vec![
            SourceSnapshot::new(
                "ticket-123",
                "revision-2",
                ArtifactDigest::from_bytes(first),
            )
            .expect("source one"),
            SourceSnapshot::new("policy-4", "revision-7", ArtifactDigest::from_bytes(second))
                .expect("source two"),
        ],
        b"retrieval-config-v1",
        b"selection-config-v1",
        builder_version,
    )
}

fn manifest_with(
    sources: Vec<SourceSnapshot>,
    retrieval: &[u8],
    selection: &[u8],
    builder_version: &str,
) -> ContextManifest {
    ContextManifest::new(
        sources,
        ArtifactDigest::from_bytes(retrieval),
        ArtifactDigest::from_bytes(selection),
        VersionedComponent::new("support-context-builder", builder_version).expect("builder"),
    )
}

fn manifested_digest(manifest: ContextManifest) -> ContextDigest {
    Context::new(
        schema("example.manifested"),
        version(1),
        ManifestedPayload::new(
            manifest,
            json!({"messages": ["ticket body"], "policy": "policy body"}),
        ),
    )
    .digest()
    .expect("manifest digest")
}

#[test]
fn manifested_payload_binds_reconstruction_metadata_and_replays() {
    let context = Context::new(
        schema("example.manifested"),
        version(1),
        ManifestedPayload::new(
            manifest("ticket body", "policy body", "git:abc"),
            json!({"messages": ["ticket body"], "policy": "policy body"}),
        ),
    );
    let digest = context.digest().expect("digest");
    let canonical = context.canonical_bytes().expect("canonical bytes");
    assert!(matches!(
        context
            .verify_digest(&digest)
            .expect("context verification"),
        ContextVerification::Match
    ));
    assert!(matches!(
        digest
            .verify_canonical_bytes(&canonical)
            .expect("stored preimage verification"),
        ContextVerification::Match
    ));
    let unrelated = Context::new(
        schema("example.manifested"),
        version(1),
        json!({"different": true}),
    )
    .digest()
    .expect("unrelated digest");
    assert!(matches!(
        unrelated
            .verify_canonical_bytes(&canonical)
            .expect("mismatched archived preimage"),
        ContextVerification::Mismatch { .. }
    ));

    let mismatched = Context::new(
        schema("example.manifested"),
        version(1),
        ManifestedPayload::new(
            manifest("ticket body", "policy body", "git:abc"),
            json!({"messages": ["changed"], "policy": "policy body"}),
        ),
    );
    assert!(matches!(
        mismatched.verify_digest(&digest).expect("mismatch"),
        ContextVerification::Mismatch { .. }
    ));
}

#[test]
fn manifest_metadata_changes_context_digest() {
    let digest = manifested_digest(manifest("ticket body", "policy body", "git:abc"));
    assert_ne!(
        digest,
        manifested_digest(manifest("changed ticket body", "policy body", "git:abc"))
    );
    assert_ne!(
        digest,
        manifested_digest(manifest("ticket body", "policy body", "git:def"))
    );

    let source_one = SourceSnapshot::new(
        "ticket-123",
        "revision-2",
        ArtifactDigest::from_bytes(b"ticket body"),
    )
    .expect("source one");
    let source_two = SourceSnapshot::new(
        "policy-4",
        "revision-7",
        ArtifactDigest::from_bytes(b"policy body"),
    )
    .expect("source two");
    assert_ne!(
        digest,
        manifested_digest(manifest_with(
            vec![source_two.clone(), source_one.clone()],
            b"retrieval-config-v1",
            b"selection-config-v1",
            "git:abc",
        ))
    );
    let changed_revision = SourceSnapshot::new(
        "ticket-123",
        "revision-3",
        ArtifactDigest::from_bytes(b"ticket body"),
    )
    .expect("revised source");
    assert_ne!(
        digest,
        manifested_digest(manifest_with(
            vec![changed_revision, source_two.clone()],
            b"retrieval-config-v1",
            b"selection-config-v1",
            "git:abc",
        ))
    );
    assert_ne!(
        digest,
        manifested_digest(manifest_with(
            vec![source_one.clone(), source_two.clone()],
            b"retrieval-config-v2",
            b"selection-config-v1",
            "git:abc",
        ))
    );
    assert_ne!(
        digest,
        manifested_digest(manifest_with(
            vec![source_one, source_two],
            b"retrieval-config-v1",
            b"selection-config-v2",
            "git:abc",
        ))
    );
}

#[test]
fn archived_context_preimages_must_be_canonical() {
    let context = Context::new(
        schema("example.preimage"),
        version(1),
        json!({"b": 2, "a": 1}),
    );
    let digest = context.digest().expect("digest");
    let noncanonical = br#"{"schema":"example.preimage","domain":"llm-provenance/context-digest/v1","schema_version":1,"payload":{"b":2,"a":1}}"#;
    assert!(digest.verify_canonical_bytes(noncanonical).is_err());
}

#[test]
fn manifested_payload_wire_format_rejects_unknown_fields() {
    let payload = ManifestedPayload::new(
        manifest("ticket body", "policy body", "git:abc"),
        json!({"messages": ["ticket body"]}),
    );
    let mut wire = serde_json::to_value(payload).expect("wire");
    wire.as_object_mut()
        .expect("manifested payload object")
        .insert("unexpected".to_owned(), json!(true));

    assert!(serde_json::from_value::<ManifestedPayload<serde_json::Value>>(wire).is_err());
}
