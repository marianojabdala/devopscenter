//! L4 `logs <pod>.<container>` (`docs/behaviour-catalogue.md` L4).
//!
//! The Python version streams a non-follow log line by line; fetching the whole
//! (finite) log and printing it is observably equivalent.

use anyhow::{anyhow, Context as _, Result};
use kube::api::LogParams;

use super::pods::{list_pods, pods_api, resolve};
use super::{Command, Output};
use crate::config::ClusterClient;

pub struct PodLogs {
    pub namespace: String,
}

#[async_trait::async_trait]
impl Command for PodLogs {
    fn name(&self) -> &'static str {
        "logs"
    }
    fn help(&self) -> &'static str {
        "Shows the logs of the selected pod/container"
    }
    async fn run(&self, ctx: &ClusterClient, args: &[String]) -> Result<Output> {
        let Some(selector) = args.get(1) else {
            return Ok(Output::Text(
                "Error you should select the number of the pod to show the log. Eg logs 0.0".into(),
            ));
        };
        let pods = list_pods(ctx, &self.namespace).await?;
        let (pod, container) = resolve(&pods, selector)?;
        let pod_name = pod
            .metadata
            .name
            .clone()
            .ok_or_else(|| anyhow!("pod has no name"))?;

        let params = LogParams {
            container,
            follow: false,
            timestamps: false,
            ..Default::default()
        };
        let text = pods_api(ctx, &self.namespace)
            .logs(&pod_name, &params)
            .await
            .with_context(|| format!("reading logs for {pod_name}"))?;
        Ok(Output::Text(text))
    }
}
