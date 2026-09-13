"""Correctness spec for cross-namespace microservice search.

Source: ``devopscenter/modules/kube/search.py`` -> ``Search.search_microservice``.

The Rust port runs the per-namespace pod listing concurrently (O9); the
observable contract is just the returned set of namespace names.
"""

import pytest

from devopscenter.modules.kube import search as search_mod
from devopscenter.modules.kube.search import Search

from _fakes import FakeCoreApi, make_pod


class _NoProgress:
    """Replaces rich.Progress so the test does not render to the terminal."""

    def __enter__(self):
        return self

    def __exit__(self, *exc):
        return False

    def track(self, iterable, description=None):  # noqa: D401 - match rich signature
        return iterable


class DirectSearch(Search):
    """Construct without ``KubeBase.__init__`` (which does kubeconfig I/O)."""

    def __init__(self, api):
        self.api = api
        self.context = "test-ctx"


@pytest.fixture(autouse=True)
def _patch_progress(monkeypatch):
    monkeypatch.setattr(search_mod, "Progress", lambda *a, **k: _NoProgress())


@pytest.fixture
def api():
    return FakeCoreApi(
        namespaces=["team-a", "team-b", "team-c"],
        pods_by_namespace={
            "team-a": [make_pod("payments-api-abc", "team-a"), make_pod("redis-0", "team-a")],
            "team-b": [make_pod("cache-warmer", "team-b")],
            "team-c": [make_pod("payments-worker-xyz", "team-c")],
        },
    )


def test_returns_namespaces_containing_a_substring_match(api):
    assert DirectSearch(api).search_microservice("payments") == {"team-a", "team-c"}


def test_returns_empty_set_when_no_match(api):
    assert DirectSearch(api).search_microservice("nonexistent") == set()


def test_match_is_substring_not_exact(api):
    assert DirectSearch(api).search_microservice("redis-0") == {"team-a"}
    assert DirectSearch(api).search_microservice("edis") == {"team-a"}


def test_scans_every_namespace(api):
    DirectSearch(api).search_microservice("payments")
    listed = [c[1]["namespace"] for c in api.calls if c[0] == "list_namespaced_pod"]
    assert set(listed) == {"team-a", "team-b", "team-c"}
