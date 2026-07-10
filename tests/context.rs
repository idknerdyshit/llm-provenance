use std::collections::BTreeMap;
use std::str::FromStr;

use llm_provenance::{Context, ContextDigest, Error, SchemaId, SchemaVersion};
use serde::Serialize;
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

    let unsupported = json.replace("sha256", "future-hash");
    assert!(serde_json::from_str::<ContextDigest>(&unsupported).is_err());
}
