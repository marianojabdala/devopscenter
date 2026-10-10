//! Presentation layer. Turns [`crate::commands::Output`] into terminal text,
//! JSON, or a Slack-friendly plain-ASCII table. Nothing else in the crate
//! prints command results.

use std::io;
use std::path::Path;
use std::sync::Mutex;

use clap::ValueEnum;
use comfy_table::{
    presets::{ASCII_FULL, UTF8_FULL},
    ContentArrangement, Table,
};
use owo_colors::OwoColorize;

use crate::commands::Output;

/// How command results are printed. Set from `--output` at startup, and
/// changeable at runtime in the REPL via the `output <table|json|slack>`
/// command (every level).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, ValueEnum)]
pub enum OutputFormat {
    #[default]
    Table,
    Json,
    /// Same layout as `Table`, but with plain ASCII (`+`/`-`/`|`) borders
    /// instead of Unicode box-drawing characters, which often render broken
    /// when pasted into Slack (even inside a code block).
    Slack,
}

static FORMAT: Mutex<OutputFormat> = Mutex::new(OutputFormat::Table);

/// The most recent [`Output`] passed to [`render`] — what `save <path>`
/// writes to disk. `None` until the first command result is rendered.
static LAST: Mutex<Option<Output>> = Mutex::new(None);

pub fn set_format(format: OutputFormat) {
    if let Ok(mut guard) = FORMAT.lock() {
        *guard = format;
    }
}

fn format() -> OutputFormat {
    FORMAT.lock().map(|g| *g).unwrap_or_default()
}

/// Render a command's output to stdout in the configured format, and
/// remember it for a subsequent `save <path>`.
pub fn render(output: &Output) {
    if let Ok(mut guard) = LAST.lock() {
        *guard = Some(output.clone());
    }
    match format() {
        OutputFormat::Table => print_table(output, UTF8_FULL),
        OutputFormat::Slack => print_table(output, ASCII_FULL),
        OutputFormat::Json => println!("{}", render_json_string(output)),
    }
}

/// Re-render the output stored by the last [`render`] call, in the currently
/// configured format, and write it to `path`. `Err` with a human-readable
/// message if there's nothing to save yet, or the write fails.
pub fn save_last(path: &Path) -> Result<(), String> {
    let output = LAST
        .lock()
        .ok()
        .and_then(|guard| guard.clone())
        .ok_or_else(|| "nothing to save yet — run a command first".to_string())?;
    let text = match format() {
        OutputFormat::Table => render_table_string(&output, UTF8_FULL),
        OutputFormat::Slack => render_table_string(&output, ASCII_FULL),
        OutputFormat::Json => render_json_string(&output),
    };
    std::fs::write(path, text).map_err(|err: io::Error| format!("writing {path:?}: {err}"))
}

fn print_table(output: &Output, preset: &str) {
    match output {
        Output::Empty => {}
        Output::Text(msg) => println!("{msg}"),
        Output::Table { rows, .. } if rows.is_empty() => {
            println!("{}", "(no results)".dimmed())
        }
        Output::Table { .. } => println!("{}", render_table_string(output, preset)),
    }
}

/// Plain (uncoloured) rendering of `output` as a table — what both the
/// terminal printer and `save <path>` use, so a saved file matches what was
/// on screen minus ANSI colour codes.
fn render_table_string(output: &Output, preset: &str) -> String {
    match output {
        Output::Empty => String::new(),
        Output::Text(msg) => msg.clone(),
        Output::Table { headers, rows } => {
            if rows.is_empty() {
                return "(no results)".to_string();
            }
            let mut table = Table::new();
            table
                .load_preset(preset)
                .set_content_arrangement(ContentArrangement::Dynamic)
                .set_header(headers.iter().map(|h| h.as_str()));
            for row in rows {
                table.add_row(row.iter().map(|c| c.as_str()));
            }
            table.to_string()
        }
    }
}

fn render_json_string(output: &Output) -> String {
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
    serde_json::to_string_pretty(&value).unwrap_or_default()
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
    let preset = if format() == OutputFormat::Slack {
        ASCII_FULL
    } else {
        UTF8_FULL
    };
    let mut table = Table::new();
    table
        .load_preset(preset)
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex as StdMutex;

    // `FORMAT`/`LAST` are process-wide statics; serialize the tests that
    // touch them so they don't race with each other (or with repl::tests,
    // which also calls set_format/save_last on the same statics).
    static TEST_LOCK: StdMutex<()> = StdMutex::new(());

    #[test]
    fn render_json_string_wraps_text_as_a_message_object() {
        let _guard = TEST_LOCK.lock().unwrap();
        assert_eq!(
            render_json_string(&Output::Text("hi".into())),
            "{\n  \"message\": \"hi\"\n}"
        );
        assert_eq!(render_json_string(&Output::Empty), "null");
    }

    #[test]
    fn render_json_string_turns_table_rows_into_objects() {
        let _guard = TEST_LOCK.lock().unwrap();
        let output = Output::table(["A", "B"], vec![["1", "2"]]);
        let json = render_json_string(&output);
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, serde_json::json!([{"A": "1", "B": "2"}]));
    }

    #[test]
    fn render_table_string_reports_text_and_empty_table_plainly() {
        let _guard = TEST_LOCK.lock().unwrap();
        assert_eq!(
            render_table_string(&Output::Text("hi".into()), UTF8_FULL),
            "hi"
        );
        assert_eq!(render_table_string(&Output::Empty, UTF8_FULL), "");
        let empty_table = Output::table(["A"], Vec::<[String; 1]>::new());
        assert_eq!(render_table_string(&empty_table, UTF8_FULL), "(no results)");
    }

    #[test]
    fn render_table_string_uses_the_given_preset() {
        let _guard = TEST_LOCK.lock().unwrap();
        let output = Output::table(["A"], vec![["1"]]);
        // ASCII preset: no Unicode box-drawing characters at all.
        let ascii = render_table_string(&output, ASCII_FULL);
        assert!(ascii.is_ascii());
        // UTF8 preset: does use them.
        let utf8 = render_table_string(&output, UTF8_FULL);
        assert!(!utf8.is_ascii());
    }

    #[test]
    fn save_last_errors_when_nothing_was_rendered_yet() {
        let _guard = TEST_LOCK.lock().unwrap();
        *LAST.lock().unwrap() = None;
        let path = std::env::temp_dir().join("devopscenter-test-save-missing.out");
        let _ = std::fs::remove_file(&path);
        assert!(save_last(&path).is_err());
        assert!(!path.exists());
    }

    #[test]
    fn save_last_writes_the_last_rendered_output_in_the_current_format() {
        let _guard = TEST_LOCK.lock().unwrap();
        set_format(OutputFormat::Json);
        render(&Output::Text("hello".into()));
        let path = std::env::temp_dir().join("devopscenter-test-save.json");
        save_last(&path).unwrap();
        let written = std::fs::read_to_string(&path).unwrap();
        assert_eq!(written, "{\n  \"message\": \"hello\"\n}");
        std::fs::remove_file(&path).unwrap();
        set_format(OutputFormat::Table);
    }
}
