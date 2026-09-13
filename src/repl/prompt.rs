use std::borrow::Cow;

use reedline::{Prompt, PromptEditMode, PromptHistorySearch, PromptHistorySearchStatus};

/// Renders `<label>:>$ ` on the left and, when set, the available commands on
/// the right — the equivalent of the Python `prompt_toolkit` bottom toolbar.
pub struct LevelPrompt {
    label: String,
    toolbar: Option<String>,
}

impl LevelPrompt {
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            toolbar: None,
        }
    }

    pub fn with_toolbar(label: impl Into<String>, commands: &[&str]) -> Self {
        Self {
            label: label.into(),
            toolbar: Some(format!("commands: {}", commands.join(" "))),
        }
    }
}

impl Prompt for LevelPrompt {
    fn render_prompt_left(&self) -> Cow<'_, str> {
        Cow::Borrowed(&self.label)
    }

    fn render_prompt_right(&self) -> Cow<'_, str> {
        match &self.toolbar {
            Some(t) => Cow::Borrowed(t),
            None => Cow::Borrowed(""),
        }
    }

    fn render_prompt_indicator(&self, _edit_mode: PromptEditMode) -> Cow<'_, str> {
        Cow::Borrowed(":>$ ")
    }

    fn render_prompt_multiline_indicator(&self) -> Cow<'_, str> {
        Cow::Borrowed("... ")
    }

    fn render_prompt_history_search_indicator(
        &self,
        history_search: PromptHistorySearch,
    ) -> Cow<'_, str> {
        let prefix = match history_search.status {
            PromptHistorySearchStatus::Passing => "",
            PromptHistorySearchStatus::Failing => "failing ",
        };
        Cow::Owned(format!(
            "({prefix}reverse-search: {}) ",
            history_search.term
        ))
    }
}
