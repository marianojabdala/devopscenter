//! L4 `exec <pod>.<container> <cmd...>` (`docs/behaviour-catalogue.md` L4).
//!
//! Non-interactive, matching the Python version: the command is wrapped in
//! `/bin/sh -c`, stdin is closed, combined stdout+stderr is returned.

use anyhow::{anyhow, Context as _, Result};
use kube::api::AttachParams;
use tokio::io::AsyncReadExt;

use super::pods::{list_pods, pods_api, resolve};
use super::{Command, Output};
use crate::config::ClusterClient;

pub struct PodExec {
    pub namespace: String,
}

#[async_trait::async_trait]
impl Command for PodExec {
    fn name(&self) -> &'static str {
        "exec"
    }
    fn help(&self) -> &'static str {
        "Execute a command in the selected container"
    }
    async fn run(&self, ctx: &ClusterClient, args: &[String]) -> Result<Output> {
        if args.len() < 3 {
            return Ok(Output::Text(
                "usage: exec <pod>.<container> <command...>".into(),
            ));
        }
        let selector = &args[1];
        let user_cmd = args[2..].join(" ");

        let pods = list_pods(ctx, &self.namespace).await?;
        let (pod, container) = resolve(&pods, selector)?;
        let pod_name = pod
            .metadata
            .name
            .clone()
            .ok_or_else(|| anyhow!("pod has no name"))?;

        let mut params = AttachParams::default()
            .stdin(false)
            .stdout(true)
            .stderr(true)
            .tty(false);
        if let Some(c) = container {
            params = params.container(c);
        }

        let mut proc = pods_api(ctx, &self.namespace)
            .exec(&pod_name, ["/bin/sh", "-c", &user_cmd], &params)
            .await
            .with_context(|| format!("exec in {pod_name}"))?;

        let mut combined = String::new();
        if let Some(mut out) = proc.stdout() {
            out.read_to_string(&mut combined).await.ok();
        }
        if let Some(mut err) = proc.stderr() {
            let mut e = String::new();
            err.read_to_string(&mut e).await.ok();
            combined.push_str(&e);
        }
        proc.join().await.ok();
        Ok(Output::Text(combined))
    }
}
