//! Nested interactive prompts. Mirrors the Python level ladder
//! (`docs/behaviour-catalogue.md`): L0 top → L1 context picker → L2 context →
//! {L3 namespaces → L4 namespace ops | L3 search | L3 views}.
//!
//! The REPL knows nothing about Kubernetes types — it only drives
//! [`crate::commands`] and hands their [`Output`] to [`crate::view`].
//!
//! Phase 4: file-backed history, per-level + live-data Tab completion, and a
//! right-hand "toolbar" listing the level's commands.

mod completer;
mod prompt;

use anyhow::Result;
use reedline::{
    default_emacs_keybindings, ColumnarMenu, Emacs, FileBackedHistory, KeyCode, KeyModifiers,
    MenuBuilder, Reedline, ReedlineEvent, ReedlineMenu, Signal,
};

use crate::commands::{
    describe::PodDescribe,
    events::NamespaceEvents,
    exec::PodExec,
    logs::PodLogs,
    namespaces::{namespace_names, NamespaceCreate, NamespaceDelete, NamespacesList},
    pods::{PodDelete, PodsList},
    rollout::RolloutStatus,
    search::PodSearch,
    summary::NamespaceSummary,
    views::{
        CanaryView, DeployView, HpaView, IngressView, IstioDestinationRulesView, IstioGatewaysView,
        IstioPeerAuthView, IstioVirtualServicesView, NodeDescribe, NodesView, PodResourcesView,
        PvcView, StatefulsetView, UsageView,
    },
    Command, Output,
};
use crate::config::{self, ClusterClient, ClusterRegistry};
use crate::view;
use completer::{Candidates, LevelCompleter};
use prompt::LevelPrompt;

enum Line {
    Text(String),
    /// Ctrl-C — abandon the line, stay in the loop (Python `KeyboardInterrupt`).
    Interrupted,
    /// Ctrl-D / read error — leave the level (Python `EOFError`).
    Eof,
}

/// Shared REPL state: the line editor plus the swappable completion word list.
struct Session {
    editor: Reedline,
    candidates: Candidates,
}

impl Session {
    fn new() -> Self {
        let candidates = Candidates::default();

        let history = history_path()
            .and_then(|path| FileBackedHistory::with_file(1000, path).ok())
            .map(Box::new);

        let mut keybindings = default_emacs_keybindings();
        keybindings.add_binding(
            KeyModifiers::NONE,
            KeyCode::Tab,
            ReedlineEvent::UntilFound(vec![
                ReedlineEvent::Menu("completion_menu".into()),
                ReedlineEvent::MenuNext,
            ]),
        );

        let mut editor = Reedline::create()
            .with_completer(Box::new(LevelCompleter::new(candidates.clone())))
            .with_menu(ReedlineMenu::EngineCompleter(Box::new(
                ColumnarMenu::default().with_name("completion_menu"),
            )))
            .with_edit_mode(Box::new(Emacs::new(keybindings)));
        if let Some(history) = history {
            editor = editor.with_history(history);
        }

        Self { editor, candidates }
    }

    /// Point completion at this level's words and read one line.
    fn read(&mut self, prompt: &LevelPrompt, words: &[&str]) -> Line {
        self.candidates.set(words.iter().copied());
        match self.editor.read_line(prompt) {
            Ok(Signal::Success(buffer)) => Line::Text(buffer),
            Ok(Signal::CtrlC) => Line::Interrupted,
            Ok(Signal::CtrlD) => Line::Eof,
            Ok(_) => Line::Interrupted,
            Err(_) => Line::Eof,
        }
    }
}

fn history_path() -> Option<std::path::PathBuf> {
    let dir = config::default_data_dir();
    std::fs::create_dir_all(&dir).ok()?;
    Some(dir.join("history.txt"))
}

/// Whitespace split (`shlex.split`, simplified).
fn tokens(line: &str) -> Vec<String> {
    line.split_whitespace().map(str::to_owned).collect()
}

fn is_exit(verb: &str) -> bool {
    verb == "exit"
}

fn is_help(verb: &str) -> bool {
    verb == "help" || verb == "h"
}

/// Run `args` against the first matching command. `false` if no command owns
/// the verb (caller prints its level's "not found" message).
async fn dispatch(cmds: &[Box<dyn Command>], cluster: &ClusterClient, args: &[String]) -> bool {
    let Some(verb) = args.first() else {
        return true;
    };
    match cmds.iter().find(|c| c.name() == verb) {
        Some(cmd) => {
            match cmd.run(cluster, args).await {
                Ok(output) => view::render(&output),
                Err(err) => view::error(&format!("{err:#}")),
            }
            true
        }
        None => false,
    }
}

fn help_entries(cmds: &[Box<dyn Command>]) -> Vec<(&'static str, &'static str)> {
    cmds.iter().map(|c| (c.name(), c.help())).collect()
}

fn verbs(cmds: &[Box<dyn Command>]) -> Vec<&'static str> {
    cmds.iter().map(|c| c.name()).collect()
}

/// Entry point — L0.
pub async fn run(registry: ClusterRegistry) -> Result<()> {
    let mut sess = Session::new();
    view::note("Welcome to Devops Center !");
    let prompt = LevelPrompt::with_toolbar("devops_center", &["kube", "help", "exit"]);

    loop {
        match sess.read(&prompt, &["kube", "help", "exit"]) {
            Line::Text(line) => {
                let args = tokens(&line);
                let Some(verb) = args.first().map(String::as_str) else {
                    continue;
                };
                if is_exit(verb) {
                    break;
                }
                if is_help(verb) {
                    view::render_help("Commands", &[("kube", "Interact with the cluster")]);
                    continue;
                }
                if verb == "kube" {
                    context_picker(&mut sess, &registry).await?;
                }
            }
            Line::Interrupted => continue,
            Line::Eof => {
                view::note("See You!!!!");
                break;
            }
        }
    }
    Ok(())
}

/// L1 — pick a context.
async fn context_picker(sess: &mut Session, registry: &ClusterRegistry) -> Result<()> {
    let names = registry.context_names();
    show_contexts(&names);
    let prompt = LevelPrompt::new("kube");
    let mut words: Vec<&str> = names.iter().map(String::as_str).collect();
    words.extend(["help", "exit"]);

    loop {
        match sess.read(&prompt, &words) {
            Line::Text(line) => {
                let choice = line.trim();
                if choice.is_empty() {
                    continue;
                }
                if is_exit(choice) {
                    break;
                }
                if is_help(choice) {
                    show_contexts(&names);
                    continue;
                }
                match registry.get(choice) {
                    Some(cluster) => context_menu(sess, cluster).await?,
                    None => { /* Python silently ignores unknown contexts */ }
                }
            }
            Line::Interrupted => continue,
            Line::Eof => break,
        }
    }
    Ok(())
}

fn show_contexts(names: &[String]) {
    view::render(&Output::table(
        ["Contexts"],
        names.iter().map(|n| [n.clone()]),
    ));
}

/// L2 — a chosen context.
async fn context_menu(sess: &mut Session, cluster: &ClusterClient) -> Result<()> {
    let label = format!("({}):context", cluster.context());
    let prompt = LevelPrompt::with_toolbar(label, &["ns", "search", "views"]);
    let words = ["ns", "search", "views", "help", "exit"];

    loop {
        match sess.read(&prompt, &words) {
            Line::Text(line) => {
                let args = tokens(&line);
                let Some(verb) = args.first().map(String::as_str) else {
                    continue;
                };
                if is_exit(verb) {
                    break;
                }
                if is_help(verb) {
                    view::render_help(
                        "Commands",
                        &[
                            ("ns", "Interact with namespaces"),
                            ("search", "Look for a microservice into the namespaces"),
                            ("views", "Shows distinct views"),
                        ],
                    );
                    continue;
                }
                match verb {
                    "ns" => namespaces_menu(sess, cluster).await?,
                    "search" => search_menu(sess, cluster).await?,
                    "views" => views_menu(sess, cluster).await?,
                    _ => view::error("Command not found!!!"),
                }
            }
            Line::Interrupted => continue,
            Line::Eof => break,
        }
    }
    Ok(())
}

/// L3 — namespaces. `list` / `create` / `delete`, or a namespace name to enter.
async fn namespaces_menu(sess: &mut Session, cluster: &ClusterClient) -> Result<()> {
    let cmds: Vec<Box<dyn Command>> = vec![
        Box::new(NamespacesList),
        Box::new(NamespaceCreate),
        Box::new(NamespaceDelete),
    ];
    let label = format!("({}):namespaces", cluster.context());
    let prompt = LevelPrompt::with_toolbar(label, &["list", "create", "delete"]);

    // Live completion: verbs + current namespace names.
    let mut ns_names = namespace_names(cluster).await.unwrap_or_default();

    loop {
        let mut words: Vec<&str> = verbs(&cmds);
        words.extend(["help", "exit"]);
        words.extend(ns_names.iter().map(String::as_str));

        match sess.read(&prompt, &words) {
            Line::Text(line) => {
                let args = tokens(&line);
                let Some(verb) = args.first().map(String::as_str) else {
                    continue;
                };
                if is_exit(verb) {
                    break;
                }
                if is_help(verb) {
                    view::render_help("Commands", &help_entries(&cmds));
                    continue;
                }
                if dispatch(&cmds, cluster, &args).await {
                    ns_names = namespace_names(cluster).await.unwrap_or(ns_names);
                    continue;
                }
                if ns_names.iter().any(|n| n == verb) {
                    namespace_ops_menu(sess, cluster, verb).await?;
                } else {
                    view::error("Command not found!!!");
                }
            }
            Line::Interrupted => continue,
            Line::Eof => break,
        }
    }
    Ok(())
}

/// L4 — operations inside one namespace.
async fn namespace_ops_menu(
    sess: &mut Session,
    cluster: &ClusterClient,
    namespace: &str,
) -> Result<()> {
    let ns = namespace.to_string();
    let cmds: Vec<Box<dyn Command>> = vec![
        Box::new(PodsList {
            namespace: ns.clone(),
        }),
        Box::new(PodLogs {
            namespace: ns.clone(),
        }),
        Box::new(PodExec {
            namespace: ns.clone(),
        }),
        Box::new(PodDelete {
            namespace: ns.clone(),
        }),
        Box::new(PodDescribe {
            namespace: ns.clone(),
        }),
        Box::new(NamespaceEvents {
            namespace: ns.clone(),
        }),
        Box::new(RolloutStatus {
            namespace: ns.clone(),
        }),
        Box::new(NamespaceSummary { namespace: ns }),
    ];
    let label = format!("({}):{}", cluster.context(), namespace);
    let prompt = LevelPrompt::with_toolbar(
        label,
        &[
            "pods", "logs", "exec", "delete", "describe", "events", "rollout", "summary",
        ],
    );
    let mut words: Vec<&str> = verbs(&cmds);
    words.extend(["help", "exit"]);

    loop {
        match sess.read(&prompt, &words) {
            Line::Text(line) => {
                let args = tokens(&line);
                let Some(verb) = args.first().map(String::as_str) else {
                    continue;
                };
                if is_exit(verb) {
                    break;
                }
                if is_help(verb) {
                    view::render_help("Commands", &help_entries(&cmds));
                    continue;
                }
                if !dispatch(&cmds, cluster, &args).await {
                    view::error("Command not found!!!");
                }
            }
            Line::Interrupted => continue,
            Line::Eof => break,
        }
    }
    Ok(())
}

/// L3-search — every input line is a query.
async fn search_menu(sess: &mut Session, cluster: &ClusterClient) -> Result<()> {
    let search = PodSearch;
    let label = format!("({})search", cluster.context());
    let prompt = LevelPrompt::new(label);

    loop {
        match sess.read(&prompt, &["help", "exit"]) {
            Line::Text(line) => {
                let query = line.trim();
                if query.is_empty() {
                    continue;
                }
                if is_exit(query) {
                    break;
                }
                if is_help(query) {
                    view::note("You have to add the microservice to search");
                    continue;
                }
                let args = vec!["search".to_string(), query.to_string()];
                match search.run(cluster, &args).await {
                    Ok(output) => view::render(&output),
                    Err(err) => view::error(&format!("{err:#}")),
                }
            }
            Line::Interrupted => continue,
            Line::Eof => break,
        }
    }
    Ok(())
}

/// L3-views — dispatch a verb over the read-only reports.
async fn views_menu(sess: &mut Session, cluster: &ClusterClient) -> Result<()> {
    let cmds: Vec<Box<dyn Command>> = vec![
        Box::new(DeployView),
        Box::new(StatefulsetView),
        Box::new(HpaView),
        Box::new(PvcView),
        Box::new(PodResourcesView),
        Box::new(UsageView),
        Box::new(IngressView),
        Box::new(NodesView),
        Box::new(NodeDescribe),
        Box::new(IstioVirtualServicesView),
        Box::new(IstioDestinationRulesView),
        Box::new(IstioGatewaysView),
        Box::new(IstioPeerAuthView),
        Box::new(CanaryView),
    ];
    let label = format!("({}):views", cluster.context());
    let toolbar: Vec<&str> = verbs(&cmds);
    let prompt = LevelPrompt::with_toolbar(label, &toolbar);
    let mut words: Vec<&str> = verbs(&cmds);
    words.extend(["help", "exit"]);

    loop {
        match sess.read(&prompt, &words) {
            Line::Text(line) => {
                let args = tokens(&line);
                let Some(verb) = args.first().map(String::as_str) else {
                    continue;
                };
                if is_exit(verb) {
                    break;
                }
                if is_help(verb) {
                    view::render_help("Views", &help_entries(&cmds));
                    continue;
                }
                if !dispatch(&cmds, cluster, &args).await {
                    view::error("View not found!!!");
                }
            }
            Line::Interrupted => continue,
            Line::Eof => break,
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokens_splits_on_any_whitespace_and_trims() {
        assert_eq!(tokens("  logs   0.1  "), vec!["logs", "0.1"]);
        assert!(tokens("   ").is_empty());
    }

    #[test]
    fn exit_and_help_verbs() {
        assert!(is_exit("exit"));
        assert!(!is_exit("quit"));
        assert!(is_help("help"));
        assert!(is_help("h"));
        assert!(!is_help("help-me"));
    }
}
