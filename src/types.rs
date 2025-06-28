use serde::Deserialize;
use std::collections::HashMap;

#[derive(Deserialize, Debug)]
pub struct InputData {
    pub r#type: String,
    pub content: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PrimitiveType {
    String,
    Number,
    Boolean,
    Null,
}

#[derive(Debug, PartialEq)]
pub enum InferredType {
    Primitive(PrimitiveType),
    Any,
    Array(Box<InferredType>),
    Object(HashMap<String, PropertyDefinition>),
    PrimitiveUnion(Vec<PrimitiveType>),
    PrimitiveTuple(Vec<PrimitiveType>),
    /// Represents an object type, which can also be an array.
    NullableObj(Box<InferredType>),
    /// Represents the identity element for type union operations.
    Never,
}

#[derive(Debug, PartialEq)]
pub struct PropertyDefinition {
    pub r#type: InferredType,
    pub optional: bool,
}

impl PrimitiveType {
    pub fn as_str(&self) -> &'static str {
        match self {
            PrimitiveType::String => "string",
            PrimitiveType::Number => "number",
            PrimitiveType::Boolean => "boolean",
            PrimitiveType::Null => "null",
        }
    }
}
