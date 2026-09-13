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
    describe::PodDescribe,
    events::NamespaceEvents,
    logs::PodLogs,
    namespaces::{NamespaceCreate, NamespaceDelete, NamespacesList},
    pods::PodsList,
    rollout::RolloutStatus,
    search::PodSearch,
    summary::NamespaceSummary,
    views::{
        DeployView, HpaView, IngressView, NodeDescribe, NodesView, PodResourcesView, PvcView,
        StatefulsetView, UsageView,
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
        /// Only show pods whose name contains this substring.
        filter: Option<String>,
        /// Only show pods that aren't healthy (crash looping, pending, failed, ...).
        #[arg(long, short)]
        unhealthy: bool,
    },
    /// Print a pod container's log.
    Logs {
        #[arg(long, short)]
        context: String,
        #[arg(long, short)]
        namespace: String,
        /// `<pod_index>.<container_index>` (container part optional).
        selector: String,
        /// Show the log of the previous (already terminated) container instance.
        #[arg(long, short = 'p')]
        previous: bool,
    },
    /// Describe a pod (like `kubectl describe pod`).
    Describe {
        #[arg(long, short)]
        context: String,
        #[arg(long, short)]
        namespace: String,
        /// `<pod_index>` (a `.<container_index>` suffix is accepted but ignored).
        selector: String,
    },
    /// Show every event in a namespace, oldest first.
    Events {
        #[arg(long, short)]
        context: String,
        #[arg(long, short)]
        namespace: String,
    },
    /// Poll a deployment's rollout until it settles (or ~60s pass).
    Rollout {
        #[arg(long, short)]
        context: String,
        #[arg(long, short)]
        namespace: String,
        deployment: String,
    },
    /// One-shot namespace health rollup (pods/deployments/statefulsets/services/pvcs).
    Summary {
        #[arg(long, short)]
        context: String,
        #[arg(long, short)]
        namespace: String,
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
        /// deploy | stateful | hpa | pvc | resources | usage | ingress | nodes | describe-node
        name: String,
        /// Substring filter on the resource name (or, for describe-node, the node name).
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
        SubCmd::Pods {
            context,
            namespace,
            filter,
            unhealthy,
        } => {
            let mut args = vec!["pods".into()];
            if unhealthy {
                args.push("--unhealthy".into());
            } else {
                args.extend(filter);
            }
            (context, Box::new(PodsList { namespace }), args)
        }
        SubCmd::Logs {
            context,
            namespace,
            selector,
            previous,
        } => {
            let mut args = vec!["logs".into(), selector];
            if previous {
                args.push("--previous".into());
            }
            (context, Box::new(PodLogs { namespace }), args)
        }
        SubCmd::Describe {
            context,
            namespace,
            selector,
        } => (
            context,
            Box::new(PodDescribe { namespace }),
            vec!["describe".into(), selector],
        ),
        SubCmd::Events { context, namespace } => (
            context,
            Box::new(NamespaceEvents { namespace }),
            vec!["events".into()],
        ),
        SubCmd::Rollout {
            context,
            namespace,
            deployment,
        } => (
            context,
            Box::new(RolloutStatus { namespace }),
            vec!["rollout".into(), deployment],
        ),
        SubCmd::Summary { context, namespace } => (
            context,
            Box::new(NamespaceSummary { namespace }),
            vec!["summary".into()],
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
        "nodes" => Box::new(NodesView),
        "describe-node" => Box::new(NodeDescribe),
        _ => return None,
    })
}
