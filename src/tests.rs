use crate::{
    generation::generate_typescript_definitions,
    inference::{infer_type_from_value, merge_types},
    types::{InferredType, InputData, PrimitiveType, PropertyDefinition},
};
use rstest::rstest;
use std::{borrow::Cow, collections::HashMap};

#[rstest]
#[case::simple_primitives(
    r#"[
        { "type": "simpleEvent", "content": "\"{\\\"name\\\":\\\"test\\\",\\\"value\\\":123,\\\"isActive\\\":true,\\\"meta\\\":null}\"" }
    ]"#,
    r#"export type SimpleEventContent = {
  isActive: boolean;
  meta: null;
  name: string;
  value: number
};

export type Events = { type: "simpleEvent", content: SimpleEventContent };
"#
)]
#[case::optional_fields(
    r#"[
        { "type": "userEvent", "content": "\"{\\\"id\\\":1,\\\"tags\\\":[\\\"a\\\",\\\"b\\\"]}\"" },
        { "type": "userEvent", "content": "\"{\\\"id\\\":2,\\\"name\\\":\\\"User2\\\",\\\"tags\\\":[\\\"c\\\"]}\"" }
    ]"#,
    r#"export type UserEventContent = {
  id: number;
  name?: string;
  tags: Array<string>
};

export type Events = { type: "userEvent", content: UserEventContent };
"#
)]
#[case::mixed_array_types(
    r#"[
        { "type": "dataEvent", "content": "\"{\\\"values\\\":[1,2]}\"" },
        { "type": "dataEvent", "content": "\"{\\\"values\\\":[\\\"a\\\",\\\"b\\\"]}\"" }
    ]"#,
    r#"export type DataEventContent = {
  values: Array<string | number>
};

export type Events = { type: "dataEvent", content: DataEventContent };
"#
)]
fn test_basic_type_inference(#[case] json_input: &str, #[case] expected_output: &str) {
    assert_eq!(
        generate_typescript_definitions(
            serde_json::from_str::<Vec<InputData>>(json_input).unwrap(),
            "Events"
        )
        .unwrap()
        .trim(),
        expected_output.trim()
    );
}

#[rstest]
#[case::empty_then_typed_array(
    r#"[
        { "type": "userCreated", "content": "\"{\\\"id\\\":1,\\\"name\\\":\\\"Alice\\\",\\\"email\\\":\\\"alice@example.com\\\",\\\"arr\\\":[]}\"" },
        { "type": "userCreated", "content": "\"{\\\"id\\\":2,\\\"name\\\":\\\"Bob\\\",\\\"age\\\":30,\\\"arr\\\":[1,2,3]}\"" }
    ]"#,
    r#"export type UserCreatedContent = {
  age?: number;
  arr: Array<number>;
  email?: string;
  id: number;
  name: string
};

export type Events = { type: "userCreated", content: UserCreatedContent };
"#
)]
fn test_array_empty_to_typed(#[case] json_input: &str, #[case] expected_output: &str) {
    let result = generate_typescript_definitions(
        serde_json::from_str::<Vec<InputData>>(json_input).unwrap(),
        "Events",
    )
    .unwrap();
    let result_normalized = normalize_ts_output(&result);
    let expected_normalized = normalize_ts_output(expected_output);
    assert_eq!(result_normalized, expected_normalized);
}

#[rstest]
#[case::simple_tuple(
    r#"[
        { "type": "tupleEvent", "content": "\"{\\\"coords\\\":[10, 20]}\"" }
    ]"#,
    r#"export type TupleEventContent = {
  coords: [number, number]
};

export type Events = { type: "tupleEvent", content: TupleEventContent };
"#
)]
#[case::tuple_length_mismatch(
    r#"[
        { "type": "tupleEvent", "content": "\"{\\\"coords\\\":[10, 20]}\"" },
        { "type": "tupleEvent", "content": "\"{\\\"coords\\\":[30, 40, 50]}\"" }
    ]"#,
    r#"export type TupleEventContent = {
  coords: Array<number>
};

export type Events = { type: "tupleEvent", content: TupleEventContent };
"#
)]
#[case::tuple_type_mismatch(
    r#"[
        { "type": "tupleEvent", "content": "\"{\\\"mixed\\\":[10, \\\"hello\\\"]}\"" }
    ]"#,
    r#"export type TupleEventContent = {
  mixed: [string, number]
};

export type Events = { type: "tupleEvent", content: TupleEventContent };
"#
)]
fn test_tuple_inference(#[case] json_input: &str, #[case] expected_output: &str) {
    let result = generate_typescript_definitions(
        serde_json::from_str::<Vec<InputData>>(json_input).unwrap(),
        "Events",
    )
    .unwrap();
    assert_eq!(result.trim(), expected_output.trim());
}

#[rstest]
#[case::nested_objects(
    r#"[
        { "type": "nestedEvent", "content": "\"{\\\"user\\\":{\\\"id\\\":1,\\\"profile\\\":{\\\"name\\\":\\\"Alice\\\",\\\"age\\\":30}}}\"" }
    ]"#,
    r#"export type NestedEventContent = {
  user: {
  id: number;
  profile: {
  age: number;
  name: string
}
}
};

export type Events = { type: "nestedEvent", content: NestedEventContent };
"#
)]
#[case::nullable_object(
    r#"[
        { "type": "nullableEvent", "content": "\"{\\\"user\\\":{\\\"id\\\":1,\\\"name\\\":\\\"Alice\\\"}}\"" },
        { "type": "nullableEvent", "content": "\"{\\\"user\\\":null}\"" }
    ]"#,
    r#"export type NullableEventContent = {
  user: {
  id: number;
  name: string
} | null
};

export type Events = { type: "nullableEvent", content: NullableEventContent };
"#
)]
fn test_nested_and_nullable_objects(#[case] json_input: &str, #[case] expected_output: &str) {
    let result = generate_typescript_definitions(
        serde_json::from_str::<Vec<InputData>>(json_input).unwrap(),
        "Events",
    )
    .unwrap();
    assert_eq!(result.trim(), expected_output.trim());
}

#[rstest]
#[case::complex_array_with_objects(
    r#"[
        { "type": "itemsEvent", "content": "\"{\\\"items\\\":[{\\\"id\\\":1,\\\"name\\\":\\\"Item1\\\"},{\\\"id\\\":2,\\\"name\\\":\\\"Item2\\\"}]}\"" }
    ]"#,
    r#"export type ItemsEventContent = {
  items: Array<{
  id: number;
  name: string
}>
};

export type Events = { type: "itemsEvent", content: ItemsEventContent };
"#
)]
#[case::mixed_type_objects_in_array(
    r#"[
        { "type": "mixedItems", "content": "\"{\\\"items\\\":[{\\\"id\\\":1,\\\"type\\\":\\\"product\\\"},{\\\"code\\\":\\\"ABC\\\",\\\"type\\\":\\\"coupon\\\"}]}\"" }
    ]"#,
    r#"export type MixedItemsContent = {
  items: Array<{
  code?: string;
  id?: number;
  type: string
}>
};

export type Events = { type: "mixedItems", content: MixedItemsContent };
"#
)]
fn test_complex_array_objects(#[case] json_input: &str, #[case] expected_output: &str) {
    let result = generate_typescript_definitions(
        serde_json::from_str::<Vec<InputData>>(json_input).unwrap(),
        "Events",
    )
    .unwrap();
    assert_eq!(result.trim(), expected_output.trim());
}

#[rstest]
#[case::multiple_events(
    r#"[
        { "type": "login", "content": "\"{\\\"userId\\\":123,\\\"timestamp\\\":1621234567890}\"" },
        { "type": "logout", "content": "\"{\\\"userId\\\":123,\\\"timestamp\\\":1621234599999}\"" },
        { "type": "purchase", "content": "\"{\\\"userId\\\":123,\\\"productId\\\":456,\\\"amount\\\":29.99}\"" }
    ]"#,
    r#"export type LoginContent = {
  timestamp: number;
  userId: number
};

export type LogoutContent = {
  timestamp: number;
  userId: number
};

export type PurchaseContent = {
  amount: number;
  productId: number;
  userId: number
};

export type Events = { type: "login", content: LoginContent } | { type: "logout", content: LogoutContent } | { type: "purchase", content: PurchaseContent };
"#
)]
fn test_multiple_event_types(#[case] json_input: &str, #[case] expected_output: &str) {
    let result = generate_typescript_definitions(
        serde_json::from_str::<Vec<InputData>>(json_input).unwrap(),
        "Events",
    )
    .unwrap();
    let result_normalized = normalize_ts_output(&result);
    let expected_normalized = normalize_ts_output(expected_output);
    assert_eq!(result_normalized, expected_normalized);
}

#[rstest]
#[case::complex_property_keys(
    r#"[
        { "type": "specialKeys", "content": "\"{\\\"valid-key\\\":true,\\\"123numeric\\\":42,\\\"normal_key\\\":\\\"value\\\"}\"" }
    ]"#,
    r#"export type SpecialKeysContent = {
  "123numeric": number;
  normal_key: string;
  "valid-key": boolean
};

export type Events = { type: "specialKeys", content: SpecialKeysContent };
"#
)]
fn test_complex_property_keys(#[case] json_input: &str, #[case] expected_output: &str) {
    let result = generate_typescript_definitions(
        serde_json::from_str::<Vec<InputData>>(json_input).unwrap(),
        "Events",
    )
    .unwrap();
    assert_eq!(result.trim(), expected_output.trim());
}

#[rstest]
#[case::three_way_primitive_union(
    r#"[
        { "type": "unionEvent", "content": "\"{\\\"value\\\":true}\"" },
        { "type": "unionEvent", "content": "\"{\\\"value\\\":42}\"" },
        { "type": "unionEvent", "content": "\"{\\\"value\\\":\\\"string\\\"}\"" }
    ]"#,
    r#"export type UnionEventContent = {
  value: string | number | boolean
};

export type Events = { type: "unionEvent", content: UnionEventContent };
"#
)]
fn test_union_types(#[case] json_input: &str, #[case] expected_output: &str) {
    let result = generate_typescript_definitions(
        serde_json::from_str::<Vec<InputData>>(json_input).unwrap(),
        "Events",
    )
    .unwrap();
    assert_eq!(result.trim(), expected_output.trim());
}

#[rstest]
#[case::nullable_primitive(
    r#"[
        { "type": "nullablePrimitive", "content": "\"{\\\"value\\\":42}\"" },
        { "type": "nullablePrimitive", "content": "\"{\\\"value\\\":null}\"" }
    ]"#,
    r#"export type NullablePrimitiveContent = {
  value: number | null
};

export type Events = { type: "nullablePrimitive", content: NullablePrimitiveContent };
"#
)]
#[case::nested_nullable_objects(
    r#"[
        { "type": "deepNullable", "content": "\"{\\\"user\\\":{\\\"profile\\\":{\\\"settings\\\":{\\\"theme\\\":\\\"dark\\\"}}}}\"" },
        { "type": "deepNullable", "content": "\"{\\\"user\\\":{\\\"profile\\\":{\\\"settings\\\":null}}}\"" },
        { "type": "deepNullable", "content": "\"{\\\"user\\\":{\\\"profile\\\":null}}\"" },
        { "type": "deepNullable", "content": "\"{\\\"user\\\":null}\"" }
    ]"#,
    r#"export type DeepNullableContent = {
  user: {
  profile: {
  settings: {
  theme: string
} | null
} | null
} | null
};

export type Events = { type: "deepNullable", content: DeepNullableContent };
"#
)]
fn test_nullable_complex_structures(#[case] json_input: &str, #[case] expected_output: &str) {
    let result = generate_typescript_definitions(
        serde_json::from_str::<Vec<InputData>>(json_input).unwrap(),
        "Events",
    )
    .unwrap();
    assert_eq!(result.trim(), expected_output.trim());
}

#[rstest]
#[case::empty_object(
    r#"[
        { "type": "emptyEvent", "content": "\"{}\""}
    ]"#,
    r#"export type EmptyEventContent = object;

export type Events = { type: "emptyEvent", content: EmptyEventContent };
"#
)]
#[case::special_key(
    r#"[
        { "type": "specialKey", "content": "\"{\\\"normal\\\":\\\"value\\\",\\\"special-key\\\":\\\"test\\\"}\"" }
    ]"#,
    r#"export type SpecialKeyContent = {
  normal: string;
  "special-key": string
};

export type Events = { type: "specialKey", content: SpecialKeyContent };
"#
)]
fn test_edge_cases(#[case] json_input: &str, #[case] expected_output: &str) {
    let result = generate_typescript_definitions(
        serde_json::from_str::<Vec<InputData>>(json_input).unwrap(),
        "Events",
    )
    .unwrap();
    assert_eq!(result.trim(), expected_output.trim());
}

#[rstest]
#[case::mixed_content_types(
    r#"[
        { "type": "mixedContent", "content": "\"{\\\"id\\\":1,\\\"data\\\":\\\"string data\\\"}\"" },
        { "type": "mixedContent", "content": "{\"id\": 2, \"data\": \"object data directly\"}" }
    ]"#,
    r#"export type MixedContentContent = {
  data: string;
  id: number
};

export type Events = { type: "mixedContent", content: MixedContentContent };
"#
)]
fn test_mixed_content_formats(#[case] json_input: &str, #[case] expected_output: &str) {
    let result = generate_typescript_definitions(
        serde_json::from_str::<Vec<InputData>>(json_input).unwrap(),
        "Events",
    )
    .unwrap();
    let result_normalized = normalize_ts_output(&result);
    let expected_normalized = normalize_ts_output(expected_output);
    assert_eq!(result_normalized, expected_normalized);
}

#[rstest]
#[case::deeply_nested_arrays(
    r#"[
        { "type": "nestedArrays", "content": "\"{\\\"data\\\":[[1,2],[3,4]]}\"" }
    ]"#,
    r#"export type NestedArraysContent = {
  data: Array<[number, number]>
};

export type Events = { type: "nestedArrays", content: NestedArraysContent };
"#
)]
#[case::complex_nested_data(
    r#"[
        { "type": "complexNested", "content": "\"{\\\"users\\\":[{\\\"id\\\":1,\\\"addresses\\\":[{\\\"city\\\":\\\"NYC\\\",\\\"zipCode\\\":10001},{\\\"city\\\":\\\"SF\\\",\\\"zipCode\\\":94107}]}]}\"" }
    ]"#,
    r#"export type ComplexNestedContent = {
  users: Array<{
  addresses: Array<{
  city: string;
  zipCode: number
}>;
  id: number
}>
};

export type Events = { type: "complexNested", content: ComplexNestedContent };
"#
)]
fn test_complex_nested_structures(#[case] json_input: &str, #[case] expected_output: &str) {
    let result = generate_typescript_definitions(
        serde_json::from_str::<Vec<InputData>>(json_input).unwrap(),
        "Events",
    )
    .unwrap();
    assert_eq!(result.trim(), expected_output.trim());
}

fn normalize_ts_output(output: &str) -> String {
    output
        .lines()
        .map(|line| {
            if line.trim().starts_with("export type") && line.contains("= {") {
                let parts: Vec<&str> = line.splitn(2, "= {").collect();
                if parts.len() == 2 {
                    let type_def_start = parts[0];
                    let properties_block = parts[1].trim_end_matches("};").trim_end_matches("}");

                    let mut properties: Vec<&str> = properties_block
                        .split(';')
                        .map(|p| p.trim())
                        .filter(|p| !p.is_empty())
                        .collect();
                    properties.sort();
                    return format!("{} = {{\n{}\n}};", type_def_start, properties.join(";\n"))
                        .into();
                }
            }
            line.into()
        })
        .collect::<Vec<Cow<str>>>()
        .join("\n")
}

#[test]
fn test_primitive_type_as_str() {
    assert_eq!(PrimitiveType::String.as_str(), "string");
    assert_eq!(PrimitiveType::Number.as_str(), "number");
    assert_eq!(PrimitiveType::Boolean.as_str(), "boolean");
    assert_eq!(PrimitiveType::Null.as_str(), "null");
}

#[test]
fn test_infer_primitive_types() {
    assert!(matches!(
        infer_type_from_value(serde_json::Value::String("test".to_string())),
        InferredType::Primitive(PrimitiveType::String)
    ));

    assert!(matches!(
        infer_type_from_value(serde_json::Value::Number(serde_json::Number::from(42))),
        InferredType::Primitive(PrimitiveType::Number)
    ));

    assert!(matches!(
        infer_type_from_value(serde_json::Value::Bool(true)),
        InferredType::Primitive(PrimitiveType::Boolean)
    ));

    assert!(matches!(
        infer_type_from_value(serde_json::Value::Null),
        InferredType::Primitive(PrimitiveType::Null)
    ));
}

#[test]
fn test_merge_primitive_types() {
    // Merging identical types should result in the same type.
    assert_eq!(
        merge_types(
            InferredType::Primitive(PrimitiveType::String),
            InferredType::Primitive(PrimitiveType::String)
        ),
        InferredType::Primitive(PrimitiveType::String)
    );

    // Merging different primitive types should create a union type.
    let merged = merge_types(
        InferredType::Primitive(PrimitiveType::String),
        InferredType::Primitive(PrimitiveType::Number),
    );

    if let InferredType::PrimitiveUnion(types) = merged {
        assert_eq!(types.len(), 2);
        assert!(types.contains(&PrimitiveType::String));
        assert!(types.contains(&PrimitiveType::Number));
    } else {
        panic!("Expected PrimitiveUnion, got {merged:?}");
    }

    // Merging any type with 'Any' should result in 'Any'.
    assert_eq!(
        merge_types(
            InferredType::Primitive(PrimitiveType::String),
            InferredType::Any
        ),
        InferredType::Any
    );
}

#[test]
fn test_merge_objects() {
    // Create two simple object types for testing.
    let mut obj1 = HashMap::new();
    obj1.insert(
        "id".to_string(),
        PropertyDefinition {
            r#type: InferredType::Primitive(PrimitiveType::Number),
            optional: false,
        },
    );
    obj1.insert(
        "name".to_string(),
        PropertyDefinition {
            r#type: InferredType::Primitive(PrimitiveType::String),
            optional: false,
        },
    );

    let mut obj2 = HashMap::new();
    obj2.insert(
        "id".to_string(),
        PropertyDefinition {
            r#type: InferredType::Primitive(PrimitiveType::Number),
            optional: false,
        },
    );
    obj2.insert(
        "age".to_string(),
        PropertyDefinition {
            r#type: InferredType::Primitive(PrimitiveType::Number),
            optional: false,
        },
    );

    // Merge the two object types.
    let merged = merge_types(InferredType::Object(obj1), InferredType::Object(obj2));

    if let InferredType::Object(props) = merged {
        assert_eq!(props.len(), 3);

        // Verify that the 'id' property exists and is not optional.
        assert!(!props.get("id").unwrap().optional);

        // Verify that the 'name' property exists and is optional.
        assert!(props.get("name").unwrap().optional);

        // Verify that the 'age' property exists and is optional.
        assert!(props.get("age").unwrap().optional);
    } else {
        panic!("Expected Object, got {merged:?}");
    }
}

#[rstest]
#[case::heterogeneous_objects_array(
    r#"[
        { "type": "mixedObjects", "content": "\"{\\\"items\\\":[{\\\"type\\\":\\\"user\\\",\\\"id\\\":1,\\\"name\\\":\\\"Alice\\\"},{\\\"type\\\":\\\"product\\\",\\\"id\\\":100,\\\"price\\\":29.99}]}\"" }
    ]"#,
    r#"export type MixedObjectsContent = {
  items: Array<{
  id: number;
  name?: string;
  price?: number;
  type: string
}>
};

export type Events = { type: "mixedObjects", content: MixedObjectsContent };
"#
)]
#[case::empty_array_then_populated(
    r#"[
        { "type": "arrayEvent", "content": "\"{\\\"items\\\":[]}\"" },
        { "type": "arrayEvent", "content": "\"{\\\"items\\\":[{\\\"id\\\":1}]}\"" }
    ]"#,
    r#"export type ArrayEventContent = {
  items: Array<{
  id: number
}>
};

export type Events = { type: "arrayEvent", content: ArrayEventContent };
"#
)]
fn test_heterogeneous_arrays(#[case] json_input: &str, #[case] expected_output: &str) {
    let result = generate_typescript_definitions(
        serde_json::from_str::<Vec<InputData>>(json_input).unwrap(),
        "Events",
    )
    .unwrap();
    let result_normalized = normalize_ts_output(&result);
    let expected_normalized = normalize_ts_output(expected_output);
    assert_eq!(result_normalized, expected_normalized);
}

#[test]
fn test_invalid_json_handling() {
    // Test how the application handles invalid JSON in the 'content' field.
    let input_data = vec![InputData {
        r#type: "invalidJson".to_string(),
        content: "{invalid-json}".to_string(),
    }];

    let result = generate_typescript_definitions(input_data, "Events");
    assert!(result.is_ok(), "Should handle invalid JSON gracefully");

    let ts_output = result.unwrap();
    assert!(
        ts_output.contains("// The 'content' field contained invalid JSON: \"{invalid-json}\"")
            && ts_output.contains("export type InvalidJsonContent = string;"),
        "Should output a string type for invalid JSON"
    );
}

#[test]
fn test_custom_primitive_type_ordering() {
    // Verify that primitive types are ordered correctly within union types.
    let types = [
        PrimitiveType::String,
        PrimitiveType::Number,
        PrimitiveType::Boolean,
        PrimitiveType::Null,
    ];
    let mut sorted_types = types;
    sorted_types.sort();
    assert_eq!(sorted_types, types,);
}
