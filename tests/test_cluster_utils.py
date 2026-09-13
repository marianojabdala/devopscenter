"""Correctness spec for the cluster helper functions.

Source: ``devopscenter/modules/kube/cluster_utils.py``.
"""

import pytest

from devopscenter.modules.kube.cluster_utils import (
    get_container_name,
    get_namespace_names,
    get_pods,
    pretty_annotations,
)
from devopscenter.modules.kube.models.pod import PodInfo

from _fakes import FakeCoreApi, make_container_status, make_namespace_list, make_pod

# --------------------------------------------------------------------------- #
# get_namespace_names                                                         #
# --------------------------------------------------------------------------- #


def test_get_namespace_names_maps_metadata_name_in_order():
    lst = make_namespace_list(["kube-system", "default", "app"])
    assert get_namespace_names(lst) == ["kube-system", "default", "app"]


def test_get_namespace_names_empty():
    assert get_namespace_names(make_namespace_list([])) == []


# --------------------------------------------------------------------------- #
# get_pods                                                                    #
# --------------------------------------------------------------------------- #


def test_get_pods_namespaced_wraps_in_pod_info():
    api = FakeCoreApi(pods_by_namespace={"app": [make_pod("a-1", "app"), make_pod("a-2", "app")]})
    pods = get_pods(api, "app")
    assert [type(p) for p in pods] == [PodInfo, PodInfo]
    assert [p.pod_name for p in pods] == ["a-1", "a-2"]
    assert api.calls[0][0] == "list_namespaced_pod"
    assert api.calls[0][1]["namespace"] == "app"


def test_get_pods_without_namespace_lists_all():
    api = FakeCoreApi(pods_by_namespace={"a": [make_pod("p1", "a")], "b": [make_pod("p2", "b")]})
    pods = get_pods(api, None)
    assert sorted(p.pod_name for p in pods) == ["p1", "p2"]
    assert api.calls[0][0] == "list_pod_for_all_namespaces"


# --------------------------------------------------------------------------- #
# get_container_name (index -> container name, insertion order)               #
# --------------------------------------------------------------------------- #


def _pod_with_containers(*names):
    statuses = [make_container_status(n, ready=True) for n in names]
    return PodInfo(make_pod("p1", "ns1", container_statuses=statuses), kube_core=None)


def test_get_container_name_by_index():
    pod = _pod_with_containers("app", "sidecar", "proxy")
    assert get_container_name(pod, 0) == "app"
    assert get_container_name(pod, 2) == "proxy"


def test_get_container_name_out_of_range_returns_none():
    pod = _pod_with_containers("app")
    assert get_container_name(pod, 5) is None


def test_get_container_name_none_pod_returns_none():
    assert get_container_name(None, 0) is None


# --------------------------------------------------------------------------- #
# pretty_annotations                                                          #
# --------------------------------------------------------------------------- #


def test_pretty_annotations_formats_dict_as_key_value_lines():
    out = pretty_annotations({"a": "1", "b": "2"})
    assert out == "a:1\nb:2"


@pytest.mark.xfail(
    strict=True,
    reason="O5b: view_ingress passes a list of tuples; list has no .items() -> AttributeError",
)
def test_pretty_annotations_accepts_list_of_pairs_from_ingress_view():
    # The ingress view builds `list(filter(..., annot.items()))` and passes it here.
    out = pretty_annotations([("a", "1"), ("b", "2")])
    assert out == "a:1\nb:2"
