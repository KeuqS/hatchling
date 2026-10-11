//! Boundary tests for the argument validator and tool metadata.
//!
//! The hand-rolled JSON Schema subset validator is the loop's
//! fail-closed gate: every rejection path here is pinned, because a
//! silently-allowed malformed argument is a trust-boundary breach.

use hatchling_loop::{Error, FakeModel, Tool, ToolCall, ToolOutput, ToolRegistry};
use serde_json::{Value, json};

/// A tool with one property per supported scalar type.
struct TypedTool;

impl Tool for TypedTool {
    fn name(&self) -> &'static str {
        "typed"
    }

    fn description(&self) -> &'static str {
        "Typed properties."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "s": { "type": "string" },
                "n": { "type": "number" },
                "i": { "type": "integer" },
                "b": { "type": "boolean" },
                "o": { "type": "object" },
                "a": { "type": "array" },
                "z": { "type": "null" }
            },
            "additionalProperties": false
        })
    }

    fn execute(&self, _arguments: Value) -> Result<ToolOutput, Error> {
        Ok(ToolOutput::Ok(json!({})))
    }
}

/// A tool whose extra fields are validated against a typed schema.
struct HeadersTool;

impl Tool for HeadersTool {
    fn name(&self) -> &'static str {
        "headers"
    }

    fn description(&self) -> &'static str {
        "Typed additionalProperties."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {},
            "additionalProperties": { "type": "string" }
        })
    }

    fn execute(&self, _arguments: Value) -> Result<ToolOutput, Error> {
        Ok(ToolOutput::Ok(json!({})))
    }
}

/// A tool with a malformed `additionalProperties` (not an object).
struct MalformedAdditionalTool;

impl Tool for MalformedAdditionalTool {
    fn name(&self) -> &'static str {
        "malformed"
    }

    fn description(&self) -> &'static str {
        "Malformed additionalProperties."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {},
            "additionalProperties": "not-a-schema"
        })
    }

    fn execute(&self, _arguments: Value) -> Result<ToolOutput, Error> {
        Ok(ToolOutput::Ok(json!({})))
    }
}

fn call(registry: &ToolRegistry<'_>, name: &str, arguments: Value) -> Result<ToolOutput, Error> {
    registry.call(&ToolCall {
        name: String::from(name),
        arguments,
    })
}

/// A tool with a required field.
struct RequiredTool;

impl Tool for RequiredTool {
    fn name(&self) -> &'static str {
        "required"
    }

    fn description(&self) -> &'static str {
        "Required field."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "name": { "type": "string" }
            },
            "required": ["name"],
            "additionalProperties": false
        })
    }

    fn execute(&self, _arguments: Value) -> Result<ToolOutput, Error> {
        Ok(ToolOutput::Ok(json!({})))
    }
}

/// A tool whose `required` entry is not an array.
struct MalformedRequiredTool;

impl Tool for MalformedRequiredTool {
    fn name(&self) -> &'static str {
        "malformed-required"
    }

    fn description(&self) -> &'static str {
        "Malformed required."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "name": { "type": "string" }
            },
            "required": "name",
            "additionalProperties": false
        })
    }

    fn execute(&self, _arguments: Value) -> Result<ToolOutput, Error> {
        Ok(ToolOutput::Ok(json!({})))
    }
}

/// A tool whose `required` array holds a non-string.
struct MalformedRequiredEntryTool;

impl Tool for MalformedRequiredEntryTool {
    fn name(&self) -> &'static str {
        "malformed-required-entry"
    }

    fn description(&self) -> &'static str {
        "Malformed required entry."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "name": { "type": "string" }
            },
            "required": [42],
            "additionalProperties": false
        })
    }

    fn execute(&self, _arguments: Value) -> Result<ToolOutput, Error> {
        Ok(ToolOutput::Ok(json!({})))
    }
}

/// A tool whose top-level schema is not an object schema.
struct NonObjectSchemaTool;

impl Tool for NonObjectSchemaTool {
    fn name(&self) -> &'static str {
        "non-object-schema"
    }

    fn description(&self) -> &'static str {
        "Non-object top-level schema."
    }

    fn parameters_schema(&self) -> Value {
        json!({ "type": "array" })
    }

    fn execute(&self, _arguments: Value) -> Result<ToolOutput, Error> {
        Ok(ToolOutput::Ok(json!({})))
    }
}

/// A tool whose `type` key is missing entirely.
struct MissingTypeTool;

impl Tool for MissingTypeTool {
    fn name(&self) -> &'static str {
        "missing-type"
    }

    fn description(&self) -> &'static str {
        "Missing top-level type."
    }

    fn parameters_schema(&self) -> Value {
        json!({ "properties": {} })
    }

    fn execute(&self, _arguments: Value) -> Result<ToolOutput, Error> {
        Ok(ToolOutput::Ok(json!({})))
    }
}

/// A tool with an unsupported property type.
struct UnsupportedPropertyTypeTool;

impl Tool for UnsupportedPropertyTypeTool {
    fn name(&self) -> &'static str {
        "unsupported-property"
    }

    fn description(&self) -> &'static str {
        "Unsupported property type."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "x": { "type": "banana" }
            }
        })
    }

    fn execute(&self, _arguments: Value) -> Result<ToolOutput, Error> {
        Ok(ToolOutput::Ok(json!({})))
    }
}

/// A tool whose `properties` entry is not an object.
struct MalformedPropertiesTool;

impl Tool for MalformedPropertiesTool {
    fn name(&self) -> &'static str {
        "malformed-properties"
    }

    fn description(&self) -> &'static str {
        "Malformed properties."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": ["not", "an", "object"]
        })
    }

    fn execute(&self, _arguments: Value) -> Result<ToolOutput, Error> {
        Ok(ToolOutput::Ok(json!({})))
    }
}

/// A tool with no `additionalProperties` key at all.
struct AbsentAdditionalTool;

impl Tool for AbsentAdditionalTool {
    fn name(&self) -> &'static str {
        "absent-additional"
    }

    fn description(&self) -> &'static str {
        "Absent additionalProperties."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "name": { "type": "string" }
            }
        })
    }

    fn execute(&self, _arguments: Value) -> Result<ToolOutput, Error> {
        Ok(ToolOutput::Ok(json!({})))
    }
}

/// A tool with `additionalProperties: true`.
struct TrueAdditionalTool;

impl Tool for TrueAdditionalTool {
    fn name(&self) -> &'static str {
        "true-additional"
    }

    fn description(&self) -> &'static str {
        "additionalProperties: true."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "name": { "type": "string" }
            },
            "additionalProperties": true
        })
    }

    fn execute(&self, _arguments: Value) -> Result<ToolOutput, Error> {
        Ok(ToolOutput::Ok(json!({})))
    }
}

fn assert_invalid_contains(err: &Error, needle: &str) {
    // Pin the user-visible message: it must name the tool, the field,
    // and the reason. Matching on the Display string keeps the
    // assertion on the contract, not the enum layout.
    let msg = err.to_string();
    assert!(
        msg.contains("invalid arguments"),
        "expected InvalidArguments, got: {msg}"
    );
    assert!(
        msg.contains(needle),
        "error message should contain '{needle}', got: {msg}"
    );
}

#[test]
fn each_supported_type_accepts_a_matching_value() {
    let mut registry = ToolRegistry::new();
    registry.register(Box::new(TypedTool));
    let cases = [
        ("s", json!("text")),
        ("n", json!(1.5)),
        ("i", json!(7)),
        ("b", json!(true)),
        ("o", json!({ "k": "v" })),
        ("a", json!([1, 2])),
        ("z", Value::Null),
    ];
    for (field, value) in cases {
        let mut args = serde_json::Map::new();
        args.insert(String::from(field), value);
        let result = call(&registry, "typed", Value::Object(args));
        assert!(
            result.is_ok(),
            "field '{field}' should accept its type: {result:?}"
        );
    }
}

#[test]
fn each_supported_type_rejects_a_mismatched_value() {
    let mut registry = ToolRegistry::new();
    registry.register(Box::new(TypedTool));
    // Wrong-typed values: a string where each other type is expected,
    // and a number where a string is expected.
    let cases = [
        ("s", json!(42)),
        ("n", json!("text")),
        ("i", json!("text")),
        ("b", json!("text")),
        ("o", json!("text")),
        ("a", json!("text")),
        ("z", json!("text")),
    ];
    for (field, value) in cases {
        let mut args = serde_json::Map::new();
        args.insert(String::from(field), value);
        let err = call(&registry, "typed", Value::Object(args))
            .expect_err(&format!("field '{field}' should reject a mismatched type"));
        assert_invalid_contains(&err, &format!("field '{field}'"));
    }
}

#[test]
fn negative_integer_is_a_valid_integer() {
    // `is_i64() || is_u64()`: a negative integer is i64-only, so this
    // pins the `||` against an `&&` mutant.
    let mut registry = ToolRegistry::new();
    registry.register(Box::new(TypedTool));
    call(&registry, "typed", json!({ "i": -5 })).expect("negative integers are valid integers");
}

#[test]
fn type_mismatch_error_names_the_actual_type() {
    let mut registry = ToolRegistry::new();
    registry.register(Box::new(TypedTool));
    let err = call(&registry, "typed", json!({ "s": 42 }))
        .expect_err("number for a string field must fail");
    assert_invalid_contains(&err, "got number");
}

#[test]
fn additional_properties_false_rejects_unexpected_fields() {
    let mut registry = ToolRegistry::new();
    registry.register(Box::new(TypedTool));
    let err = call(&registry, "typed", json!({ "surprise": 1 }))
        .expect_err("additionalProperties: false must reject unknown fields");
    assert_invalid_contains(&err, "unexpected field");
}

#[test]
fn typed_additional_properties_validate_extra_fields() {
    let mut registry = ToolRegistry::new();
    registry.register(Box::new(HeadersTool));
    // Valid extra fields pass.
    call(&registry, "headers", json!({ "x-a": "b" })).expect("string extra field should validate");
    // Invalid extra fields fail.
    let err = call(&registry, "headers", json!({ "x-a": 42 }))
        .expect_err("non-string extra field must fail typed additionalProperties");
    assert_invalid_contains(&err, "x-a");
}

#[test]
fn non_object_additional_properties_is_a_schema_error() {
    // A tool schema outside the supported subset fails closed at
    // validation time instead of silently allowing extra fields.
    let mut registry = ToolRegistry::new();
    registry.register(Box::new(MalformedAdditionalTool));
    let err = call(&registry, "malformed", json!({ "x": 1 }))
        .expect_err("non-object additionalProperties must be rejected");
    assert_invalid_contains(&err, "unsupported additionalProperties");
}

#[test]
fn required_field_present_passes() {
    let mut registry = ToolRegistry::new();
    registry.register(Box::new(RequiredTool));
    call(&registry, "required", json!({ "name": "x" }))
        .expect("a present required field must validate");
}

#[test]
fn missing_required_field_reports_its_name() {
    let mut registry = ToolRegistry::new();
    registry.register(Box::new(RequiredTool));
    let err = call(&registry, "required", json!({}))
        .expect_err("a missing required field must fail validation");
    assert_invalid_contains(&err, "missing required field: 'name'");
}

#[test]
fn malformed_required_is_rejected_fail_closed() {
    // A `required` entry that is not an array must be rejected
    // rather than silently skipped.
    let mut registry = ToolRegistry::new();
    registry.register(Box::new(MalformedRequiredTool));
    let err = call(&registry, "malformed-required", json!({ "name": "x" }))
        .expect_err("a non-array 'required' must fail validation");
    assert_invalid_contains(&err, "must be an array of strings");
}

#[test]
fn malformed_required_entry_is_rejected_fail_closed() {
    let mut registry = ToolRegistry::new();
    registry.register(Box::new(MalformedRequiredEntryTool));
    let err = call(
        &registry,
        "malformed-required-entry",
        json!({ "name": "x" }),
    )
    .expect_err("a non-string 'required' entry must fail validation");
    assert_invalid_contains(&err, "must be an array of strings");
}

#[test]
fn non_object_top_level_schema_is_rejected() {
    let mut registry = ToolRegistry::new();
    registry.register(Box::new(NonObjectSchemaTool));
    let err = call(&registry, "non-object-schema", json!([]))
        .expect_err("a non-object top-level schema must fail validation");
    assert_invalid_contains(&err, "unsupported schema type");
}

#[test]
fn missing_top_level_type_is_rejected() {
    let mut registry = ToolRegistry::new();
    registry.register(Box::new(MissingTypeTool));
    let err = call(&registry, "missing-type", json!({}))
        .expect_err("a schema without a top-level type must fail validation");
    assert_invalid_contains(&err, "unsupported schema type");
}

#[test]
fn unsupported_property_type_is_rejected() {
    let mut registry = ToolRegistry::new();
    registry.register(Box::new(UnsupportedPropertyTypeTool));
    let err = call(&registry, "unsupported-property", json!({ "x": 1 }))
        .expect_err("an unsupported property type must fail validation");
    assert_invalid_contains(&err, "unsupported type 'banana'");
}

#[test]
fn malformed_properties_is_rejected_fail_closed() {
    // A `properties` entry that is not an object must be rejected
    // rather than silently skipping property validation.
    let mut registry = ToolRegistry::new();
    registry.register(Box::new(MalformedPropertiesTool));
    let err = call(&registry, "malformed-properties", json!({ "x": 1 }))
        .expect_err("a non-object 'properties' must fail validation");
    assert_invalid_contains(&err, "must be an object");
}

#[test]
fn absent_additional_properties_permits_extra_fields() {
    let mut registry = ToolRegistry::new();
    registry.register(Box::new(AbsentAdditionalTool));
    call(
        &registry,
        "absent-additional",
        json!({ "name": "x", "extra": 1 }),
    )
    .expect("absent additionalProperties must permit extra fields");
}

#[test]
fn true_additional_properties_permits_extra_fields() {
    let mut registry = ToolRegistry::new();
    registry.register(Box::new(TrueAdditionalTool));
    call(
        &registry,
        "true-additional",
        json!({ "name": "x", "extra": 1 }),
    )
    .expect("additionalProperties: true must permit extra fields");
}

#[test]
fn non_object_arguments_are_rejected() {
    let mut registry = ToolRegistry::new();
    registry.register(Box::new(TypedTool));
    let err = call(&registry, "typed", json!([1, 2, 3]))
        .expect_err("array arguments must fail an object schema");
    assert_invalid_contains(&err, "must be a JSON object");
}

#[test]
fn descriptions_lists_every_registered_tool() {
    let mut registry = ToolRegistry::new();
    registry.register(Box::new(TypedTool));
    registry.register(Box::new(HeadersTool));
    let descriptions = registry.descriptions();
    assert_eq!(descriptions.len(), 2);
    let typed = descriptions
        .iter()
        .find(|d| d.name == "typed")
        .expect("typed tool must be described");
    assert_eq!(typed.description, "Typed properties.");
    assert_eq!(typed.parameters_schema.get("type"), Some(&json!("object")));
    assert!(
        descriptions.iter().any(|d| d.name == "headers"),
        "headers tool must be described"
    );
}

#[test]
fn debug_impls_name_their_types() {
    let registry = ToolRegistry::new();
    let registry_dbg = format!("{registry:?}");
    assert!(
        registry_dbg.contains("ToolRegistry"),
        "ToolRegistry Debug should name the type: {registry_dbg}"
    );
    let model = FakeModel::new(Vec::new());
    let model_dbg = format!("{model:?}");
    assert!(
        model_dbg.contains("FakeModel"),
        "FakeModel Debug should name the type: {model_dbg}"
    );
}
