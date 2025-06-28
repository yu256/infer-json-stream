use crate::types::{InferredType, PrimitiveType};
use rayon::iter::{IntoParallelIterator as _, ParallelIterator as _};
use std::borrow::Cow;

fn format_property_key(key: &str) -> Cow<'_, str> {
    fn is_valid_ts_identifier(s: &str) -> bool {
        s.chars().next().is_some_and(|c| !c.is_numeric())
            && s.chars()
                .all(|c| c.is_alphanumeric() || c == '_' || c == '$')
    }

    if is_valid_ts_identifier(key) {
        Cow::Borrowed(key)
    } else {
        Cow::Owned(format!("\"{}\"", key.replace("\"", "\\\"")))
    }
}

pub fn format_type_to_ts_string(inferred_type: InferredType) -> Cow<'static, str> {
    match inferred_type {
        InferredType::Primitive(prim_type) => Cow::Borrowed(prim_type.as_str()),
        InferredType::Any => Cow::Borrowed("any"),
        InferredType::PrimitiveUnion(types) => {
            let type_strings: Vec<&str> = types.iter().map(PrimitiveType::as_str).collect();
            Cow::Owned(type_strings.join(" | "))
        }
        InferredType::PrimitiveTuple(types) => {
            if types.is_empty() {
                return Cow::Borrowed("[]");
            }
            let type_strings: Vec<&str> = types.iter().map(PrimitiveType::as_str).collect();
            Cow::Owned(format!("[{}]", type_strings.join(", ")))
        }
        InferredType::Array(item_type) => {
            Cow::Owned(format!("Array<{}>", format_type_to_ts_string(*item_type)))
        }
        InferredType::Object(properties) => {
            if properties.is_empty() {
                return Cow::Borrowed("object");
            }

            let mut sorted = properties.into_iter().collect::<Vec<_>>();
            sorted.sort_by(|(key1, _), (key2, _)| key1.cmp(key2));
            let props = sorted
                .into_par_iter()
                .map(|(key, prop_def)| {
                    let optional_marker = if prop_def.optional { "?" } else { "" };
                    format!(
                        "  {}{}: {}",
                        format_property_key(&key),
                        optional_marker,
                        format_type_to_ts_string(prop_def.r#type)
                    )
                })
                .collect::<Vec<_>>();
            Cow::Owned(format!("{{\n{}\n}}", props.join(";\n")))
        }
        InferredType::NullableObj(obj) => {
            let inner_type = format_type_to_ts_string(*obj);
            Cow::Owned(format!("{inner_type} | null"))
        }
        InferredType::Never => unreachable!(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_property_key() {
        assert_eq!(format_property_key("normalKey"), "normalKey");
        assert_eq!(format_property_key("with-dash"), "\"with-dash\"");
        assert_eq!(format_property_key("123numeric"), "\"123numeric\"");
        assert_eq!(format_property_key("with\"quote"), "\"with\\\"quote\"");
        assert_eq!(format_property_key("$special"), "$special");
        assert_eq!(format_property_key("_underscore"), "_underscore");
    }
}
