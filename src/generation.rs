use crate::{
    formatting::format_type_to_ts_string,
    inference::{infer_type_from_value, merge_types},
    types::{InferredType, InputData, PrimitiveType},
};
use anyhow::Result;
use rayon::iter::{IntoParallelIterator as _, ParallelIterator as _};
use serde_json::Value;
use std::collections::{BTreeMap, HashMap};
use stringcase::pascal_case;

pub fn generate_typescript_definitions(
    json_array: Vec<InputData>,
    root_name: &str,
) -> Result<String> {
    let items = json_array
        .into_par_iter()
        .map(|item| {
            let Ok(first_parse) = serde_json::from_str(&item.content) else {
                return (
                    item.r#type.clone(),
                    Value::String(item.content.clone()),
                    true,
                );
            };

            let final_content: Value = match first_parse {
                Value::String(s) => {
                    if let Ok(parsed) = serde_json::from_str(&s) {
                        parsed
                    } else {
                        return (item.r#type.clone(), Value::String(s), true);
                    }
                }
                _ => first_parse,
            };

            (item.r#type, final_content, false)
        })
        .collect::<Vec<_>>();

    let (type_contents, invalid_json_types): (
        HashMap<String, Vec<Value>>,
        HashMap<String, String>,
    ) = items.into_iter().fold(
        (HashMap::new(), HashMap::new()),
        |(mut type_contents, mut invalid_json_types), (type_name, content, is_invalid)| {
            if is_invalid {
                if let Value::String(s) = content {
                    invalid_json_types.insert(type_name, s);
                }
            } else {
                type_contents.entry(type_name).or_default().push(content);
            }
            (type_contents, invalid_json_types)
        },
    );

    let mut overall_inferred_types: BTreeMap<String, InferredType> = type_contents
        .into_par_iter()
        .map(|(event_type, contents)| {
            let final_type = contents
                .into_par_iter()
                .map(infer_type_from_value)
                .reduce(|| InferredType::Never, merge_types);
            // `contents` is never empty, so `final_type` will not be `Never`.
            (event_type, final_type)
        })
        .collect();
    overall_inferred_types.extend(invalid_json_types.keys().map(|event_type| {
        (
            event_type.clone(),
            InferredType::Primitive(PrimitiveType::String),
        )
    }));

    let (ts_output, event_type_strings): (String, Vec<String>) = overall_inferred_types
        .into_par_iter()
        .map(|(event_type_key, inferred_type)| {
            let type_name = format!("{}Content", pascal_case(&event_type_key));

            let ts_output = if let Some(invalid_json) = invalid_json_types.get(&event_type_key) {
                format!(
                    "// The 'content' field contained invalid JSON: \"{invalid_json}\"\nexport type {type_name} = {};\n\n",
                    format_type_to_ts_string(inferred_type)
                )
            } else {
                format!(
                    "export type {type_name} = {};\n\n",
                    format_type_to_ts_string(inferred_type)
                )
            };

            let event_type_string =
                format!("{{ type: \"{event_type_key}\", content: {type_name} }}");
            (ts_output, event_type_string)
        })
        .unzip();

    let output = format!(
        "{ts_output}export type {root_name} = {};\n",
        event_type_strings.join(" | ")
    );

    Ok(output)
}
