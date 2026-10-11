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
    can_i::CanI,
    configcheck::ConfigCheck,
    configmap::ConfigMapShow,
    describe::PodDescribe,
    endpoints::ServiceEndpoints,
    events::NamespaceEvents,
    imagecheck::ImageCheck,
    istio_sidecar::IstioSidecarCheck,
    labels::PodLabels,
    logs::PodLogs,
    namespaces::{NamespaceCreate, NamespaceDelete, NamespacesList},
    pods::PodsList,
    rollout::RolloutStatus,
    search::PodSearch,
    secret::SecretShow,
    summary::NamespaceSummary,
    views::{
        CronJobView, DaemonSetView, DeployView, HpaView, IngressView, JobView, NodeDescribe,
        NodesView, PdbView, PodResourcesView, PvcView, StatefulsetView, TaintsView, UsageView,
    },
    warnings::Warnings,
    whois::WhoIs,
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
        /// Only show pods with at least one restarted container.
        #[arg(long, short)]
        restarted: bool,
    },
    /// Print a pod container's log, or every matching pod's log.
    Logs {
        #[arg(long, short)]
        context: String,
        #[arg(long, short)]
        namespace: String,
        /// `<pod_index>.<container_index>` (container part optional), or a
        /// name substring matching one or more pods.
        selector: String,
        /// Show the log of the previous (already terminated) container instance.
        #[arg(long, short = 'p')]
        previous: bool,
        /// Only keep output lines containing this substring (case-insensitive).
        #[arg(long)]
        filter: Option<String>,
    },
    /// Show a pod's labels as a Key/Value table, or every matching pod's.
    Labels {
        #[arg(long, short)]
        context: String,
        #[arg(long, short)]
        namespace: String,
        /// `<pod_index>.<container_index>` (container part ignored), or a
        /// name substring matching one or more pods.
        selector: String,
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
    /// Show a service's backing pods and their ready/not-ready state.
    Endpoints {
        #[arg(long, short)]
        context: String,
        #[arg(long, short)]
        namespace: String,
        service: String,
    },
    /// Checks that every ConfigMap/Secret a pod references actually exists.
    #[command(name = "configcheck")]
    ConfigCheck {
        #[arg(long, short)]
        context: String,
        #[arg(long, short)]
        namespace: String,
        /// `<pod_index>` (a `.<container_index>` suffix is accepted but ignored).
        selector: String,
    },
    /// Shows a ConfigMap's full key/value content.
    #[command(name = "configmap")]
    ConfigMap {
        #[arg(long, short)]
        context: String,
        #[arg(long, short)]
        namespace: String,
        name: String,
    },
    /// Shows a Secret's keys and byte lengths — never the decoded values.
    Secret {
        #[arg(long, short)]
        context: String,
        #[arg(long, short)]
        namespace: String,
        name: String,
    },
    /// Compares a deployment's desired image against what's actually running.
    #[command(name = "imagecheck")]
    ImageCheck {
        #[arg(long, short)]
        context: String,
        #[arg(long, short)]
        namespace: String,
        deployment: String,
    },
    /// Checks whether you may <verb> <resource> in a namespace (kubectl auth can-i).
    CanI {
        #[arg(long, short)]
        context: String,
        #[arg(long, short)]
        namespace: String,
        verb: String,
        resource: String,
    },
    /// Checks whether a pod has its Istio sidecar injected, and why/why not.
    IstioSidecar {
        #[arg(long, short)]
        context: String,
        #[arg(long, short)]
        namespace: String,
        /// `<pod_index>` (a `.<container_index>` suffix is accepted but ignored).
        selector: String,
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
    /// Find the pod and/or service for a given IP, across all namespaces.
    Whois {
        #[arg(long, short)]
        context: String,
        ip: String,
    },
    /// Every Warning event across all namespaces, newest first.
    Warnings {
        #[arg(long, short)]
        context: String,
    },
    /// Run a read-only cluster view.
    View {
        #[arg(long, short)]
        context: String,
        /// deploy | stateful | daemonsets | jobs | cronjobs | hpa | pdb | pvc | resources | usage | ingress | nodes | describe-node | taints
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
            restarted,
        } => {
            let mut args = vec!["pods".into()];
            if unhealthy {
                args.push("--unhealthy".into());
            } else if restarted {
                args.push("--restarted".into());
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
            filter,
        } => {
            let mut args = vec!["logs".into(), selector];
            if previous {
                args.push("--previous".into());
            }
            if let Some(term) = filter {
                args.push("--filter".into());
                args.push(term);
            }
            (context, Box::new(PodLogs { namespace }), args)
        }
        SubCmd::Labels {
            context,
            namespace,
            selector,
        } => (
            context,
            Box::new(PodLabels { namespace }),
            vec!["labels".into(), selector],
        ),
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
        SubCmd::Endpoints {
            context,
            namespace,
            service,
        } => (
            context,
            Box::new(ServiceEndpoints { namespace }),
            vec!["endpoints".into(), service],
        ),
        SubCmd::ConfigCheck {
            context,
            namespace,
            selector,
        } => (
            context,
            Box::new(ConfigCheck { namespace }),
            vec!["configcheck".into(), selector],
        ),
        SubCmd::ConfigMap {
            context,
            namespace,
            name,
        } => (
            context,
            Box::new(ConfigMapShow { namespace }),
            vec!["configmap".into(), name],
        ),
        SubCmd::Secret {
            context,
            namespace,
            name,
        } => (
            context,
            Box::new(SecretShow { namespace }),
            vec!["secret".into(), name],
        ),
        SubCmd::ImageCheck {
            context,
            namespace,
            deployment,
        } => (
            context,
            Box::new(ImageCheck { namespace }),
            vec!["imagecheck".into(), deployment],
        ),
        SubCmd::CanI {
            context,
            namespace,
            verb,
            resource,
        } => (
            context,
            Box::new(CanI { namespace }),
            vec!["can-i".into(), verb, resource],
        ),
        SubCmd::IstioSidecar {
            context,
            namespace,
            selector,
        } => (
            context,
            Box::new(IstioSidecarCheck { namespace }),
            vec!["istio-sidecar".into(), selector],
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
        SubCmd::Whois { context, ip } => (context, Box::new(WhoIs), vec!["whois".into(), ip]),
        SubCmd::Warnings { context } => (context, Box::new(Warnings), vec!["warnings".into()]),
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
        "daemonsets" => Box::new(DaemonSetView),
        "jobs" => Box::new(JobView),
        "cronjobs" => Box::new(CronJobView),
        "hpa" => Box::new(HpaView),
        "pdb" => Box::new(PdbView),
        "pvc" => Box::new(PvcView),
        "resources" => Box::new(PodResourcesView),
        "usage" => Box::new(UsageView),
        "ingress" => Box::new(IngressView),
        "nodes" => Box::new(NodesView),
        "describe-node" => Box::new(NodeDescribe),
        "taints" => Box::new(TaintsView),
        _ => return None,
    })
}
