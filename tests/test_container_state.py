"""Correctness spec for pod / container state derivation.

Source: ``devopscenter/modules/kube/models/pod.py`` -> ``PodInfo``.
Consumed by the ``pods`` command and ``logs`` / ``exec`` container lookup.

Finding (O8, wider than first noted): with ``only_errors=False`` the state
expression collapses to just two outcomes because
``(not container.ready and NOT_READY)`` short-circuits before the
terminated / waiting / running branches can ever be evaluated:

    ready truthy  -> "Running"
    otherwise     -> "Not Ready"

So "Terminated", "Waiting" and "Failure" are unreachable dead code here.
The Rust port must derive state from the real container ``state`` /
``last_state`` / phase via an exhaustive ``match``, not this ``or`` chain.
"""

import pytest

from devopscenter.modules.kube.constants import NOT_READY, RUNNING
from devopscenter.modules.kube.models.pod import PodInfo

from _fakes import make_container_status, make_pod


def _pod_info(statuses):
    return PodInfo(make_pod("p1", "ns1", container_statuses=statuses), kube_core=None)


def test_ready_container_is_running():
    info = _pod_info([make_container_status("c1", ready=True)])
    result = info.get_containers_to_show()
    assert result == {"c1": {"state": RUNNING, "info": None}}


@pytest.mark.parametrize("ready_value", [False, None, 0, ""])
def test_not_ready_container_is_not_ready(ready_value):
    info = _pod_info([make_container_status("c1", ready=ready_value)])
    assert info.get_containers_to_show()["c1"]["state"] == NOT_READY


def test_terminated_branch_is_dead_code_today():
    # A not-ready + terminated container still reports "Not Ready", never
    # "Terminated". Documented so the Rust port deliberately diverges.
    info = _pod_info(
        [make_container_status("c1", ready=False, terminated=object())]
    )
    assert info.get_containers_to_show()["c1"]["state"] == NOT_READY


def test_multiple_containers_preserve_order_and_key_by_name():
    info = _pod_info(
        [
            make_container_status("app", ready=True),
            make_container_status("sidecar", ready=False),
        ]
    )
    result = info.get_containers_to_show()
    assert list(result) == ["app", "sidecar"]
    assert result["app"]["state"] == RUNNING
    assert result["sidecar"]["state"] == NOT_READY


def test_no_container_statuses_returns_empty():
    assert _pod_info(None).get_containers_to_show() == {}
    assert _pod_info([]).get_containers_to_show() == {}


def test_pod_info_basic_accessors():
    pod = make_pod("web-0", "prod", node_name="ip-10-0-0-5")
    info = PodInfo(pod, kube_core=None)
    assert info.pod_name == "web-0"
    assert info.namespace == "prod"
    assert info.node_name == "ip-10-0-0-5"
    assert str(info) == "Pod name: web-0"
