use crate::types::{InferredType, PrimitiveType, PropertyDefinition};
use serde_json::Value;
use std::collections::HashMap;

const EMPTY_TUPLE: InferredType = InferredType::PrimitiveTuple(Vec::new());

pub fn infer_type_from_value(value: Value) -> InferredType {
    match value {
        Value::Null => InferredType::Primitive(PrimitiveType::Null),
        Value::Bool(_) => InferredType::Primitive(PrimitiveType::Boolean),
        Value::Number(_) => InferredType::Primitive(PrimitiveType::Number),
        Value::String(_) => InferredType::Primitive(PrimitiveType::String),
        Value::Array(arr) => {
            // First, attempt to infer a tuple type (only for primitive types).
            let tuple = 'block: {
                let mut tuple = Vec::new();
                for val in arr.iter() {
                    match val {
                        Value::Null => tuple.push(PrimitiveType::Null),
                        Value::Bool(_) => tuple.push(PrimitiveType::Boolean),
                        Value::Number(_) => tuple.push(PrimitiveType::Number),
                        Value::String(_) => tuple.push(PrimitiveType::String),
                        _ => break 'block None,
                    }
                }
                tuple.sort();
                Some(InferredType::PrimitiveTuple(tuple))
            };

            tuple.unwrap_or_else(|| {
                // Otherwise, fall back to array type inference.
                match arr
                    .into_iter()
                    .map(infer_type_from_value)
                    .reduce(merge_types)
                {
                    Some(item_type) => InferredType::Array(Box::new(item_type)),
                    None => EMPTY_TUPLE,
                }
            })
        }
        Value::Object(obj) => {
            let properties: HashMap<String, PropertyDefinition> = obj
                .into_iter()
                .map(|(key, val)| {
                    (
                        key,
                        PropertyDefinition {
                            r#type: infer_type_from_value(val),
                            optional: false,
                        },
                    )
                })
                .collect();
            InferredType::Object(properties)
        }
    }
}

pub fn merge_types(type1: InferredType, type2: InferredType) -> InferredType {
    if type1 == type2 {
        return type1;
    }

    match (type1, type2) {
        (InferredType::Any, _) | (_, InferredType::Any) => InferredType::Any,
        (InferredType::Never, t) | (t, InferredType::Never) => t,
        (InferredType::Primitive(p1), InferredType::Primitive(p2)) => {
            InferredType::PrimitiveUnion(if p1 < p2 { vec![p1, p2] } else { vec![p2, p1] })
        }
        (InferredType::Primitive(p), InferredType::PrimitiveUnion(mut types))
        | (InferredType::PrimitiveUnion(mut types), InferredType::Primitive(p)) => {
            if types.contains(&p) {
                InferredType::PrimitiveUnion(types)
            } else {
                types.push(p);
                types.sort();
                InferredType::PrimitiveUnion(types)
            }
        }
        (InferredType::PrimitiveUnion(types1), InferredType::PrimitiveUnion(types2)) => {
            if types1 == types2 {
                return InferredType::PrimitiveUnion(types1);
            }
            let mut merged_types = types1;
            for t in types2.iter() {
                if !merged_types.contains(t) {
                    merged_types.push(*t);
                }
            }
            merged_types.sort();
            InferredType::PrimitiveUnion(merged_types)
        }
        (InferredType::PrimitiveTuple(types1), InferredType::PrimitiveTuple(types2)) => {
            if types1 == types2 {
                InferredType::PrimitiveTuple(types1)
            } else {
                let all_types: Vec<PrimitiveType> =
                    types1.iter().chain(types2.iter()).copied().collect();

                let Some((&first_type, tail_types)) = all_types.split_first() else {
                    return EMPTY_TUPLE;
                };
                let all_same_type = tail_types.iter().all(|t| *t == first_type);

                if all_same_type {
                    InferredType::Array(Box::new(InferredType::Primitive(first_type)))
                } else {
                    // If types differ, create a union of all unique types
                    let mut unique_types = all_types;
                    unique_types.sort();
                    unique_types.dedup();

                    InferredType::Array(Box::new(InferredType::PrimitiveUnion(unique_types)))
                }
            }
        }
        (InferredType::PrimitiveTuple(types), InferredType::Array(item_type))
        | (InferredType::Array(item_type), InferredType::PrimitiveTuple(types)) => {
            // Convert the tuple to an array and merge.
            let primitive_item_type = match *item_type {
                InferredType::Primitive(p) => Some(p),
                InferredType::PrimitiveUnion(mut union_types) => {
                    // If the array already has a union type, include all of its elements.
                    let mut tuple_has_new_type = false;

                    for t in types.iter() {
                        if !union_types.contains(t) {
                            union_types.push(*t);
                            tuple_has_new_type = true;
                        }
                    }

                    if tuple_has_new_type {
                        union_types.sort();
                    }

                    return InferredType::Array(Box::new(InferredType::PrimitiveUnion(
                        union_types,
                    )));
                }
                _ => None,
            };

            // Check if all elements in the tuple have the same type.
            let Some((&first_type, tail_types)) = types.split_first() else {
                return InferredType::Array(item_type);
            };
            let all_same_type = tail_types.iter().all(|t| *t == first_type);
            if !all_same_type {
                // If types differ, create a union of all unique types
                let mut unique_types = types;
                if let Some(p) = primitive_item_type {
                    if !unique_types.contains(&p) {
                        unique_types.push(p);
                    }
                }
                unique_types.sort();

                return InferredType::Array(Box::new(InferredType::PrimitiveUnion(unique_types)));
            }

            match primitive_item_type {
                Some(p) if p == first_type => {
                    InferredType::Array(Box::new(InferredType::Primitive(p)))
                }
                Some(p) => {
                    // If types differ, create a union of all unique types
                    let union_types = if p < first_type {
                        vec![p, first_type]
                    } else {
                        vec![first_type, p]
                    };
                    InferredType::Array(Box::new(InferredType::PrimitiveUnion(union_types)))
                }
                None => InferredType::Array(Box::new(InferredType::Primitive(first_type))),
            }
        }
        (InferredType::Array(item_type1), InferredType::Array(item_type2)) => {
            InferredType::Array(Box::new(merge_types(*item_type1, *item_type2)))
        }
        (InferredType::Object(obj1), InferredType::Object(mut obj2)) => {
            let mut merged_props = HashMap::new();

            for (key, prop1) in obj1 {
                let prop_def = match obj2.remove(&key) {
                    Some(p2) => PropertyDefinition {
                        r#type: merge_types(prop1.r#type, p2.r#type),
                        optional: prop1.optional || p2.optional,
                    },
                    None => PropertyDefinition {
                        optional: true,
                        ..prop1
                    },
                };
                merged_props.insert(key, prop_def);
            }
            for (key, prop2) in obj2 {
                merged_props.insert(
                    key,
                    PropertyDefinition {
                        r#type: prop2.r#type,
                        optional: true,
                    },
                );
            }
            InferredType::Object(merged_props)
        }
        (t, InferredType::Primitive(PrimitiveType::Null))
        | (InferredType::Primitive(PrimitiveType::Null), t) => match t {
            InferredType::Object(_) | InferredType::Array(_) => {
                InferredType::NullableObj(Box::new(t))
            }
            _ => unreachable!(),
        },
        (InferredType::NullableObj(obj), InferredType::NullableObj(obj2)) => {
            InferredType::NullableObj(Box::new(merge_types(*obj, *obj2)))
        }
        (InferredType::NullableObj(obj), t) | (t, InferredType::NullableObj(obj)) => {
            InferredType::NullableObj(Box::new(merge_types(*obj, t)))
        }
        _ => InferredType::Any,
    }
}
