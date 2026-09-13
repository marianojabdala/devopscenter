//! devopscenter — interactive Kubernetes REPL (Rust rewrite).
//!
//! Run with no subcommand for the interactive REPL; run a subcommand for a
//! one-shot, script-friendly invocation (honours `--output json`).

mod commands;
mod config;
mod domain;
mod repl;
mod view;

use std::path::PathBuf;

use anyhow::{anyhow, Result};
use clap::{Parser, Subcommand};
use tracing_subscriber::EnvFilter;

use commands::{
    logs::PodLogs,
    namespaces::{NamespaceCreate, NamespaceDelete, NamespacesList},
    pods::PodsList,
    search::PodSearch,
    views::{
        DeployView, HpaView, IngressView, PodResourcesView, PvcView, StatefulsetView, UsageView,
    },
    Command,
};
use config::ClusterRegistry;
use view::OutputFormat;

#[derive(Debug, Parser)]
#[command(name = "devopscenter", version, about)]
pub struct Cli {
    /// Directory scanned for kubeconfig files (recursively, skipping `*cache*`).
    #[arg(long, env = "DEVOPSCENTER_KUBE_DIR", global = true)]
    kube_dir: Option<PathBuf>,

    /// Log level: error|warn|info|debug|trace (or a full RUST_LOG filter).
    #[arg(long, default_value = "warn", global = true)]
    log_level: String,

    /// Output format for command results.
    #[arg(long, value_enum, default_value_t = OutputFormat::Table, global = true)]
    output: OutputFormat,

    #[command(subcommand)]
    command: Option<SubCmd>,
}

#[derive(Debug, Subcommand)]
enum SubCmd {
    /// List the contexts discovered under the kube dir and exit.
    Contexts,
    /// Namespace operations.
    Ns {
        #[arg(long, short)]
        context: String,
        #[command(subcommand)]
        action: NsAction,
    },
    /// List pods (and their containers) in a namespace.
    Pods {
        #[arg(long, short)]
        context: String,
        #[arg(long, short)]
        namespace: String,
    },
    /// Print a pod container's log.
    Logs {
        #[arg(long, short)]
        context: String,
        #[arg(long, short)]
        namespace: String,
        /// `<pod_index>.<container_index>` (container part optional).
        selector: String,
    },
    /// Search for pods whose name contains a substring, across all namespaces.
    Search {
        #[arg(long, short)]
        context: String,
        term: String,
    },
    /// Run a read-only cluster view.
    View {
        #[arg(long, short)]
        context: String,
        /// deploy | stateful | hpa | pvc | resources | usage | ingress
        name: String,
        /// Optional substring filter on the resource name.
        filter: Option<String>,
    },
}

#[derive(Debug, Subcommand)]
enum NsAction {
    /// List namespaces.
    List,
    /// Create a namespace.
    Create { name: String },
    /// Delete a namespace.
    Delete { name: String },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // kube's rustls stack (rustls 0.23) needs a process-wide crypto provider
    // chosen explicitly.
    rustls::crypto::ring::default_provider()
        .install_default()
        .expect("install rustls ring crypto provider");

    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(&cli.log_level)),
        )
        .with_target(false)
        .with_writer(std::io::stderr)
        .init();

    view::set_format(cli.output);

    let kube_dir = cli
        .kube_dir
        .clone()
        .unwrap_or_else(config::default_kube_dir);
    let registry = ClusterRegistry::discover(&kube_dir).await?;
    if registry.is_empty() {
        eprintln!(
            "No usable contexts found under {}. Point --kube-dir at your kubeconfig directory.",
            kube_dir.display()
        );
        std::process::exit(1);
    }

    match cli.command {
        None => repl::run(registry).await,
        Some(SubCmd::Contexts) => {
            let rows: Vec<[String; 2]> = registry
                .context_names()
                .into_iter()
                .map(|name| {
                    let source = registry
                        .get(&name)
                        .map(|c| c.source().display().to_string())
                        .unwrap_or_default();
                    [name, source]
                })
                .collect();
            view::render(&commands::Output::table(["Context", "Source"], rows));
            Ok(())
        }
        Some(sub) => run_once(&registry, sub).await,
    }
}

/// Execute a single subcommand non-interactively.
async fn run_once(registry: &ClusterRegistry, sub: SubCmd) -> Result<()> {
    let (context, cmd, args): (String, Box<dyn Command>, Vec<String>) = match sub {
        SubCmd::Contexts => unreachable!("handled by caller"),
        SubCmd::Ns { context, action } => match action {
            NsAction::List => (context, Box::new(NamespacesList), vec!["list".into()]),
            NsAction::Create { name } => (
                context,
                Box::new(NamespaceCreate),
                vec!["create".into(), name],
            ),
            NsAction::Delete { name } => (
                context,
                Box::new(NamespaceDelete),
                vec!["delete".into(), name],
            ),
        },
        SubCmd::Pods { context, namespace } => (
            context,
            Box::new(PodsList { namespace }),
            vec!["pods".into()],
        ),
        SubCmd::Logs {
            context,
            namespace,
            selector,
        } => (
            context,
            Box::new(PodLogs { namespace }),
            vec!["logs".into(), selector],
        ),
        SubCmd::Search { context, term } => {
            (context, Box::new(PodSearch), vec!["search".into(), term])
        }
        SubCmd::View {
            context,
            name,
            filter,
        } => {
            let view = view_by_name(&name).ok_or_else(|| anyhow!("unknown view {name:?}"))?;
            let mut args = vec![name];
            args.extend(filter);
            (context, view, args)
        }
    };

    let cluster = registry
        .get(&context)
        .ok_or_else(|| anyhow!("context {context:?} not found"))?;
    let output = cmd.run(cluster, &args).await?;
    view::render(&output);
    Ok(())
}

fn view_by_name(name: &str) -> Option<Box<dyn Command>> {
    Some(match name {
        "deploy" => Box::new(DeployView),
        "stateful" => Box::new(StatefulsetView),
        "hpa" => Box::new(HpaView),
        "pvc" => Box::new(PvcView),
        "resources" => Box::new(PodResourcesView),
        "usage" => Box::new(UsageView),
        "ingress" => Box::new(IngressView),
        _ => return None,
    })
}
