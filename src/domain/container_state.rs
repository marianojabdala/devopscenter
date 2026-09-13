//! Container display-state derivation.
//!
//! Replaces the Python `PodInfo.get_containers_to_show` `or`-chain, which
//! short-circuited so that it only ever produced `Running` or `Not Ready`
//! (migration.md **O8**; spec: `tests/test_container_state.py`). This is a
//! deliberate divergence: we derive the real state from the container's
//! `state` / `ready` fields with an exhaustive match.

use k8s_openapi::api::core::v1::ContainerStatus;

/// What the `pods` table shows in the `State` column.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DisplayState {
    Running,
    NotReady,
    Waiting { reason: Option<String> },
    Terminated { reason: Option<String> },
}

impl DisplayState {
    /// Label matching the Python constants where they overlap.
    pub fn label(&self) -> String {
        match self {
            DisplayState::Running => "Running".to_string(),
            DisplayState::NotReady => "Not Ready".to_string(),
            DisplayState::Waiting { reason } => match reason {
                Some(r) => format!("Waiting-{r}"),
                None => "Waiting".to_string(),
            },
            DisplayState::Terminated { reason } => match reason {
                Some(r) => format!("Terminated-{r}"),
                None => "Terminated".to_string(),
            },
        }
    }
}

/// Priority: terminated > waiting > running(ready) > not-ready.
pub fn derive(status: &ContainerStatus) -> DisplayState {
    if let Some(state) = &status.state {
        if let Some(term) = &state.terminated {
            return DisplayState::Terminated {
                reason: term.reason.clone(),
            };
        }
        if let Some(wait) = &state.waiting {
            return DisplayState::Waiting {
                reason: wait.reason.clone(),
            };
        }
        if state.running.is_some() && status.ready {
            return DisplayState::Running;
        }
    }
    DisplayState::NotReady
}

#[cfg(test)]
mod tests {
    use super::*;
    use k8s_openapi::api::core::v1::{
        ContainerState, ContainerStateRunning, ContainerStateTerminated, ContainerStateWaiting,
    };

    fn status(ready: bool, state: Option<ContainerState>) -> ContainerStatus {
        ContainerStatus {
            name: "c".into(),
            ready,
            state,
            image: "img".into(),
            image_id: String::new(),
            restart_count: 0,
            ..Default::default()
        }
    }

    #[test]
    fn ready_running_is_running() {
        let s = status(
            true,
            Some(ContainerState {
                running: Some(ContainerStateRunning::default()),
                ..Default::default()
            }),
        );
        assert_eq!(derive(&s), DisplayState::Running);
    }

    #[test]
    fn running_but_not_ready_is_not_ready() {
        let s = status(
            false,
            Some(ContainerState {
                running: Some(ContainerStateRunning::default()),
                ..Default::default()
            }),
        );
        assert_eq!(derive(&s), DisplayState::NotReady);
    }

    #[test]
    fn terminated_wins_even_when_ready_flag_stale() {
        let s = status(
            true,
            Some(ContainerState {
                terminated: Some(ContainerStateTerminated {
                    reason: Some("Error".into()),
                    exit_code: 1,
                    ..Default::default()
                }),
                ..Default::default()
            }),
        );
        assert_eq!(
            derive(&s),
            DisplayState::Terminated {
                reason: Some("Error".into())
            }
        );
        assert_eq!(derive(&s).label(), "Terminated-Error");
    }

    #[test]
    fn waiting_reason_surfaces() {
        let s = status(
            false,
            Some(ContainerState {
                waiting: Some(ContainerStateWaiting {
                    reason: Some("CrashLoopBackOff".into()),
                    ..Default::default()
                }),
                ..Default::default()
            }),
        );
        assert_eq!(derive(&s).label(), "Waiting-CrashLoopBackOff");
    }

    #[test]
    fn no_state_is_not_ready() {
        assert_eq!(derive(&status(false, None)), DisplayState::NotReady);
    }
}
