"""Shared fakes for the Phase 0 characterization suite.

The real ``kubernetes`` client models are large generated classes; for
characterization we only need duck-typed attribute access, so we build the
minimal shapes each function under test actually touches.
"""

from types import SimpleNamespace
from typing import Iterable, Optional


def ns_meta(name: str, namespace: Optional[str] = None) -> SimpleNamespace:
    """A ``V1ObjectMeta``-ish object."""
    return SimpleNamespace(name=name, namespace=namespace, annotations=None)


def make_namespace(name: str) -> SimpleNamespace:
    """A ``V1Namespace``-ish object (only ``.metadata.name`` is read)."""
    return SimpleNamespace(metadata=ns_meta(name))


def make_namespace_list(names: Iterable[str]) -> SimpleNamespace:
    """A ``V1NamespaceList``-ish object."""
    return SimpleNamespace(items=[make_namespace(n) for n in names])


def make_container_status(
    name: str,
    ready: object = True,
    terminated: object = None,
    waiting: object = None,
    running: object = None,
) -> SimpleNamespace:
    """A ``V1ContainerStatus``-ish object as read by ``PodInfo``."""
    return SimpleNamespace(
        name=name,
        ready=ready,
        state=SimpleNamespace(terminated=terminated, waiting=waiting, running=running),
    )


def make_pod(
    name: str,
    namespace: str = "default",
    node_name: str = "node-a",
    container_statuses: Optional[list] = None,
    containers: Optional[list] = None,
    volumes: Optional[list] = None,
) -> SimpleNamespace:
    """A ``V1Pod``-ish object covering every attribute the code reads."""
    return SimpleNamespace(
        metadata=ns_meta(name, namespace),
        spec=SimpleNamespace(
            node_name=node_name,
            containers=containers or [],
            volumes=volumes,
        ),
        status=SimpleNamespace(container_statuses=container_statuses),
    )


def make_pod_list(pods: Iterable[SimpleNamespace]) -> SimpleNamespace:
    return SimpleNamespace(items=list(pods))


class FakeCoreApi:
    """Stand-in for ``client.CoreV1Api`` used by ``get_pods`` / ``Search``.

    ``pods_by_namespace`` maps namespace name -> list of fake pod objects.
    """

    def __init__(self, namespaces=None, pods_by_namespace=None):
        self._namespaces = list(namespaces or [])
        self._pods = dict(pods_by_namespace or {})
        self.calls = []

    def list_namespace(self):
        self.calls.append(("list_namespace", {}))
        return make_namespace_list(self._namespaces)

    def list_namespaced_pod(self, namespace=None, **kwargs):
        self.calls.append(("list_namespaced_pod", {"namespace": namespace, **kwargs}))
        return make_pod_list(self._pods.get(namespace, []))

    def list_pod_for_all_namespaces(self, **kwargs):
        self.calls.append(("list_pod_for_all_namespaces", kwargs))
        everything = [p for pods in self._pods.values() for p in pods]
        return make_pod_list(everything)
