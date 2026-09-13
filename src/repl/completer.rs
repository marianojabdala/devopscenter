//! Prefix completion whose candidate set the REPL swaps as it moves between
//! prompt levels — and refreshes from live cluster data (context names,
//! namespace names). Delivers the "per-level" + "completion from live data"
//! goals without rebuilding the `Reedline` instance.

use std::sync::{Arc, Mutex};

use reedline::{Completer, CompletionResult, Span, Suggestion};

/// Shared, swappable list of completion words for the current prompt level.
#[derive(Clone, Default)]
pub struct Candidates(Arc<Mutex<Vec<String>>>);

impl Candidates {
    pub fn set<I, S>(&self, words: I)
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let mut guard = self.0.lock().expect("candidate lock");
        guard.clear();
        guard.extend(words.into_iter().map(Into::into));
        guard.sort();
        guard.dedup();
    }

    fn snapshot(&self) -> Vec<String> {
        self.0.lock().expect("candidate lock").clone()
    }
}

pub struct LevelCompleter {
    candidates: Candidates,
}

impl LevelCompleter {
    pub fn new(candidates: Candidates) -> Self {
        Self { candidates }
    }
}

impl Completer for LevelCompleter {
    fn complete(&mut self, line: &str, pos: usize) -> CompletionResult {
        let start = line[..pos]
            .rfind(char::is_whitespace)
            .map(|i| i + 1)
            .unwrap_or(0);
        let prefix = &line[start..pos];
        let span = Span::new(start, pos);

        let suggestions: Vec<Suggestion> = self
            .candidates
            .snapshot()
            .into_iter()
            .filter(|w| w.starts_with(prefix))
            .map(|value| Suggestion {
                value,
                display_override: None,
                description: None,
                style: None,
                extra: None,
                span,
                append_whitespace: true,
                match_indices: None,
            })
            .collect();
        CompletionResult::fresh(suggestions)
    }
}
