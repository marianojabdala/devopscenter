//! L4 `can-i <verb> <resource>` — mirrors `kubectl auth can-i`: asks the API
//! server, via a `SelfSubjectAccessReview`, whether the current identity may
//! perform `<verb>` on `<resource>` in this namespace. Covers "why is this
//! Forbidden" without leaving the tool.

use anyhow::{Context as _, Result};
use k8s_openapi::api::authorization::v1::{
    ResourceAttributes, SelfSubjectAccessReview, SelfSubjectAccessReviewSpec,
};
use kube::api::{Api, PostParams};

use super::{Command, Output};
use crate::config::ClusterClient;

pub struct CanI {
    pub namespace: String,
}

#[async_trait::async_trait]
impl Command for CanI {
    fn name(&self) -> &'static str {
        "can-i"
    }
    fn help(&self) -> &'static str {
        "Checks whether you may <verb> <resource> in this namespace (kubectl auth can-i)"
    }
    async fn run(&self, ctx: &ClusterClient, args: &[String]) -> Result<Output> {
        let (Some(verb), Some(resource)) = (args.get(1), args.get(2)) else {
            return Ok(Output::Text("usage: can-i <verb> <resource>".into()));
        };
        let review = SelfSubjectAccessReview {
            metadata: Default::default(),
            spec: SelfSubjectAccessReviewSpec {
                resource_attributes: Some(ResourceAttributes {
                    namespace: Some(self.namespace.clone()),
                    verb: Some(verb.clone()),
                    resource: Some(resource.clone()),
                    ..Default::default()
                }),
                ..Default::default()
            },
            status: None,
        };
        let api: Api<SelfSubjectAccessReview> = Api::all(ctx.client());
        let result = api
            .create(&PostParams::default(), &review)
            .await
            .context("creating SelfSubjectAccessReview")?;
        let status = result.status.unwrap_or_default();
        let verdict = if status.allowed { "yes" } else { "no" };
        let mut msg = format!("{verdict} — can {verb} {resource} in {}", self.namespace);
        if let Some(reason) = status.reason.filter(|r| !r.is_empty()) {
            msg.push_str(&format!(" ({reason})"));
        }
        Ok(Output::Text(msg))
    }
}
