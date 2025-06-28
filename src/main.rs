use anyhow::{Context as _, Result};
use clap::Parser;
use infer_json_stream::{generation::generate_typescript_definitions, types::InputData};
use rayon::iter::{IntoParallelIterator as _, ParallelBridge, ParallelIterator};
use serde_json::Value;
use std::fs;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(short, long, default_value = "input.json")]
    input: String,
    #[arg(short, long, default_value = "output.ts")]
    output: String,
    #[arg(short, long, default_value = "Events")]
    root_name: String,
    #[arg(long, default_value = "type")]
    tag: String,
    #[arg(long, default_value = "content")]
    content: String,
    #[arg(long)]
    json_array: bool,
}

fn main() -> Result<()> {
    let args = Args::parse();

    let read_start = std::time::Instant::now();
    let bytes = fs::read(&args.input)?;
    let json_input = String::from_utf8(bytes)?;
    println!("File reading took: {:?}", read_start.elapsed());

    let parse_start = std::time::Instant::now();
    let json_array = if args.json_array {
        let par_iter = serde_json::from_str::<Vec<Value>>(&json_input)?.into_par_iter();
        parse_json(par_iter, &args.tag, &args.content)
    } else {
        let par_iter = json_input
            .lines()
            .par_bridge()
            .filter(|line| !line.trim().is_empty())
            .map(|line| serde_json::from_str::<Value>(line).expect("Failed to parse JSON line"));
        parse_json(par_iter, &args.tag, &args.content)
    }?;
    println!("JSON parsing took: {:?}", parse_start.elapsed());

    let gen_start = std::time::Instant::now();
    let ts_output = generate_typescript_definitions(json_array, &args.root_name)?;
    println!("TypeScript generation took: {:?}", gen_start.elapsed());

    let write_start = std::time::Instant::now();
    fs::write(&args.output, ts_output)?;
    println!("File writing took: {:?}", write_start.elapsed());

    Ok(())
}

fn parse_json(
    par_iter: impl ParallelIterator<Item = Value>,
    tag: &str,
    content: &str,
) -> Result<Vec<InputData>> {
    par_iter
        .map(|value| {
            let r#type = value
                .get(tag)
                .and_then(Value::as_str)
                .with_context(|| format!("Missing or invalid {tag} field in value: {value}"))?
                .to_string();
            let content = value
                .get(content)
                .and_then(Value::as_str)
                .with_context(|| format!("Missing or invalid {content} field in type {type}"))?
                .to_string();
            Ok(InputData { r#type, content })
        })
        .collect()
}
