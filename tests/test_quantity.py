"""Correctness spec for Kubernetes quantity parsing.

Source: ``devopscenter/modules/kube/cluster_utils.py`` -> ``convert_to_milicore``,
``convert_to_mi``. Consumed by the ``usage`` view.

The Rust port must replace both with a single typed ``Quantity`` parser that
accepts the full suffix set (``n u m k M G Ki Mi Gi Ti`` + bare number) and
never panics. The ``xfail`` cases below are the concrete "must fix" list.
"""

import pytest

from devopscenter.modules.kube.cluster_utils import convert_to_mi, convert_to_milicore

# --------------------------------------------------------------------------- #
# CPU -> millicores                                                           #
# --------------------------------------------------------------------------- #


@pytest.mark.parametrize(
    "raw, expected",
    [
        ("1000000n", "1m"),      # 1e6 nanocores = 1 millicore
        ("500000000n", "500m"),  # 0.5 core
        ("1n", "1m"),            # rounds up (math.ceil)
        ("250000u", "250m"),     # microcores
        ("1500u", "2m"),         # rounds up
    ],
)
def test_convert_to_milicore_known_suffixes(raw, expected):
    assert convert_to_milicore(raw) == expected


@pytest.mark.parametrize(
    "raw, expected",
    [
        ("100m", "100m"),  # already millicores
        ("2", "2000m"),    # whole cores
    ],
)
@pytest.mark.xfail(strict=True, reason="O4: no branch for 'm' / bare cores -> returns '0m'")
def test_convert_to_milicore_millicore_and_core_inputs(raw, expected):
    assert convert_to_milicore(raw) == expected


def test_convert_to_milicore_bare_zero_happens_to_work():
    # Not a bug: "0" has no n/u suffix, milicore stays 0, output "0m".
    assert convert_to_milicore("0") == "0m"


# --------------------------------------------------------------------------- #
# memory -> Mi                                                                #
# --------------------------------------------------------------------------- #


@pytest.mark.parametrize(
    "raw, add_label, expected",
    [
        ("1024000Ki", True, "1024.0Mi"),
        ("1024000Ki", False, 1024.0),
        ("512Mi", True, "512Mi"),
        ("512Mi", False, 512),
    ],
)
def test_convert_to_mi_known_suffixes(raw, add_label, expected):
    assert convert_to_mi(raw, add_label) == expected


@pytest.mark.parametrize("raw", ["2Gi", "1000000000", "1Ti", "900000"])
@pytest.mark.xfail(strict=True, reason="O4: unhandled suffix -> UnboundLocalError on 'converted'")
def test_convert_to_mi_unhandled_suffixes_should_not_crash(raw):
    # Spec: any valid k8s memory quantity converts to a number of Mi.
    result = convert_to_mi(raw, add_label=False)
    assert isinstance(result, (int, float))
