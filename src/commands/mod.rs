//! Command layer: one type per verb, uniform signature, data-only output.
//!
//! Rendering lives in [`crate::view`]; commands never print. This keeps them
//! unit-testable against a mock API and lets the same output feed a future
//! `--output json` mode (migration.md §3.2, §7).

pub mod exec;
pub mod logs;
pub mod namespaces;
pub mod pods;
pub mod search;
pub mod views;

use anyhow::Result;

use crate::config::ClusterClient;

/// Structured result of running a command.
#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(dead_code)] // `Empty` / `Text` are exercised by the Phase 3 verbs.
pub enum Output {
    /// Nothing to show (e.g. a successful mutation).
    Empty,
    /// A single line / short message.
    Text(String),
    /// A table to render with headers and string cells.
    Table {
        headers: Vec<String>,
        rows: Vec<Vec<String>>,
    },
}

impl Output {
    pub fn table<H, R, C>(headers: H, rows: R) -> Self
    where
        H: IntoIterator,
        H::Item: Into<String>,
        R: IntoIterator<Item = C>,
        C: IntoIterator,
        <C as IntoIterator>::Item: Into<String>,
    {
        Output::Table {
            headers: headers.into_iter().map(Into::into).collect(),
            rows: rows
                .into_iter()
                .map(|r| r.into_iter().map(Into::into).collect())
                .collect(),
        }
    }
}

/// A runnable verb. `args` is the whitespace-split remainder of the input line
/// (`args[0]` is the verb itself), matching the Python `shlex.split` contract.
#[async_trait::async_trait]
pub trait Command: Send + Sync {
    /// Verb as typed at the prompt.
    fn name(&self) -> &'static str;

    /// One-line help shown in the level's help table.
    fn help(&self) -> &'static str;

    async fn run(&self, ctx: &ClusterClient, args: &[String]) -> Result<Output>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_builds_headers_and_string_rows() {
        let out = Output::table(["A", "B"], vec![["1", "2"], ["3", "4"]]);
        assert_eq!(
            out,
            Output::Table {
                headers: vec!["A".into(), "B".into()],
                rows: vec![vec!["1".into(), "2".into()], vec!["3".into(), "4".into()],],
            }
        );
    }

    #[test]
    fn table_accepts_zero_rows() {
        let out = Output::table(["only-header"], Vec::<[String; 1]>::new());
        match out {
            Output::Table { headers, rows } => {
                assert_eq!(headers, vec!["only-header".to_string()]);
                assert!(rows.is_empty());
            }
            other => panic!("expected table, got {other:?}"),
        }
    }
}
