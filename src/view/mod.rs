//! Presentation layer. Turns [`crate::commands::Output`] into terminal text or
//! JSON. Nothing else in the crate prints command results.

use std::sync::OnceLock;

use clap::ValueEnum;
use comfy_table::{presets::UTF8_FULL, ContentArrangement, Table};
use owo_colors::OwoColorize;

use crate::commands::Output;

/// How command results are printed. Set once at startup from `--output`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, ValueEnum)]
pub enum OutputFormat {
    #[default]
    Table,
    Json,
}

static FORMAT: OnceLock<OutputFormat> = OnceLock::new();

pub fn set_format(format: OutputFormat) {
    let _ = FORMAT.set(format);
}

fn format() -> OutputFormat {
    FORMAT.get().copied().unwrap_or_default()
}

/// Render a command's output to stdout in the configured format.
pub fn render(output: &Output) {
    match format() {
        OutputFormat::Table => render_table(output),
        OutputFormat::Json => render_json(output),
    }
}

fn render_table(output: &Output) {
    match output {
        Output::Empty => {}
        Output::Text(msg) => println!("{msg}"),
        Output::Table { headers, rows } => {
            if rows.is_empty() {
                println!("{}", "(no results)".dimmed());
                return;
            }
            let mut table = Table::new();
            table
                .load_preset(UTF8_FULL)
                .set_content_arrangement(ContentArrangement::Dynamic)
                .set_header(headers.iter().map(|h| h.as_str()));
            for row in rows {
                table.add_row(row.iter().map(|c| c.as_str()));
            }
            println!("{table}");
        }
    }
}

fn render_json(output: &Output) {
    let value = match output {
        Output::Empty => serde_json::Value::Null,
        Output::Text(msg) => serde_json::json!({ "message": msg }),
        Output::Table { headers, rows } => {
            let objects: Vec<serde_json::Value> = rows
                .iter()
                .map(|row| {
                    let map: serde_json::Map<String, serde_json::Value> = headers
                        .iter()
                        .cloned()
                        .zip(row.iter().cloned().map(serde_json::Value::String))
                        .collect();
                    serde_json::Value::Object(map)
                })
                .collect();
            serde_json::Value::Array(objects)
        }
    };
    println!(
        "{}",
        serde_json::to_string_pretty(&value).unwrap_or_default()
    );
}

/// A boxed key/value help listing (`help` at any prompt level). Table format
/// only — in JSON mode it is emitted as an array of `{command, description}`.
pub fn render_help(title: &str, entries: &[(&str, &str)]) {
    if format() == OutputFormat::Json {
        let arr: Vec<_> = entries
            .iter()
            .map(|(c, d)| serde_json::json!({ "command": c, "description": d }))
            .collect();
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::Value::Array(arr)).unwrap_or_default()
        );
        return;
    }
    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL)
        .set_content_arrangement(ContentArrangement::Dynamic)
        .set_header([title, ""]);
    for (verb, desc) in entries {
        table.add_row([*verb, *desc]);
    }
    println!("{table}");
}

/// Info line (matches the Python `self.log` cyan-ish notices). Suppressed in
/// JSON mode so stdout stays valid JSON.
pub fn note(msg: &str) {
    if format() == OutputFormat::Json {
        return;
    }
    println!("{}", msg.cyan());
}

/// Error line — always goes to stderr, so it never corrupts JSON stdout.
pub fn error(msg: &str) {
    eprintln!("{}", msg.red());
}
