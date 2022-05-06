# -*- coding: utf-8 -*-
__author__ = "Mariano Jose Abdala"
__version__ = "0.1.0"


"""
Module to be used for helper functions
"""

from typing import List
from devopscenter.modules.kube.pod import PodInfo
import math


def convertToMilicore(value):
    """
    The method takes a value that can be nanocore o microcore and
    convert it to milicore
    :param value, int value
    :return the value converted to milicore.
    """
    milicore = 0
    if "n" in value:
        value = value.replace("n", "")
        milicore = math.ceil(float(value) / 1000000000 * 1000)
    elif "u" in value:
        value = value.replace("u", "")
        milicore = math.ceil(float(value) / 1000000 * 1000)

    return str(milicore) + "m"


def convertToMi(value, add_label=True) -> str:
    """
    Convert the value from Ki to Mi to be more familiar.
    :param value int value
    :return the value in an string format.
    """

    if "Ki" in value:
        converted = int(value.replace("Ki", "")) / 1000
    elif "Mi" in value:
        converted = int(value.replace("Mi", ""))

    if add_label:
        converted = f"{str(converted)}Mi"

    return converted


def get_namespace_names(namespaces_list) -> List:
    """
    Get the list of names of namespaces given the object V1ListNamesapces
    :param namespaces_list
    :return List list of names of namespaces
    """
    namespaces = []
    for namespace in namespaces_list.items:
        namespaces.append(namespace.metadata.name)
    return namespaces


def get_pods(core_v1, namespace=None) -> List[PodInfo]:
    """
    Get the list of pod from the entire cluster or a namespace.
    """
    if namespace is not None:
        pods = core_v1.list_namespaced_pod(
            namespace=namespace, timeout_seconds=5, _request_timeout=5
        )
    else:
        pods = core_v1.list_pod_for_all_namespaces()

    pod_names = []
    for pod in pods.items:
        pod_names.append(PodInfo(pod, core_v1))

    return pod_names
