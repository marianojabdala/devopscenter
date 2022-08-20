"""
This class will be in charge of encapsulate the real pod and
create useful methods that extract data from it to be use as
we need it.
"""

__author__ = "Mariano Jose Abdala"
__version__ = "0.1.0"

import json
from typing import List
from enum import Enum


class PodState(Enum):
    """ Class that holds the state of the pods to be shown. """
    TERMINATED = "Terminated"
    FAILURE = "Failure"
    RUNNING = "Running"
    WAITING = "Waiting"
    NOT_READY = "Not Ready"
    COMPLETED = "Completed"
    NOT_READY_MOUNT = "Not Ready-FailedMount"


class PodInfo:  # pylint: disable=too-few-public-methods
    """
    This class is meant to be used to have the real pod and it's containers
    and get what we want using the real pod info.
    """

    def __init__(self, pod, kube_core):
        """
        Constructor
        """
        self.pod = pod
        self.kube_core = kube_core

    @property
    def node_name(self) -> str:
        """
        Returns the name/ip of the node where the pod is located
        """
        return self.pod.spec.node_name

    @property
    def namespace(self) -> str:
        """
        Returns the namespace where the pod is located
        """
        return self.pod.metadata.namespace

    @property
    def real_pod(self):
        """
        Returns the real pod tha was collected
        """
        return self.pod

    @property
    def pod_name(self):
        """
        Returns the pod name
        """
        return self.pod.metadata.name

    @property
    def volumes(self) -> List["V1Volume"]:
        """
        Returns all the volumes of the pod
        return list of V1Volume
        """
        return self.pod.spec.volumes

    @property
    def containers_statuses(self) -> List:
        """
        Return the status of the containers
        """

        return self.pod.status.container_statuses if self.pod.status.container_statuses else []

    @property
    def containers(self):
        """ Retrieves the conteiners of the pod. """
        return self.real_pod.spec.containers

    def get_containers_to_show(self, only_errors=False):
        """
        Get all the containers of the pod that will be shown.

        :param only_errors just get the containers with errors.
        """
        containers_errors = {}
        containers_info = {}
        field_selector = "involvedObject.name=" + self.pod_name

        for container in self.containers_statuses:
            state = ((container.ready and PodState.RUNNING.value)
                     or ((container.state.terminated and
                          (container.state.terminated.reason == "Completed"
                           and PodState.COMPLETED.value)) or PodState.TERMINATED.value)
                     or (container.state.waiting and PodState.WAITING.value)
                     or (container.state.running and PodState.RUNNING.value)
                     or (container.state.failure and PodState.FAILURE.value) or PodState.NOT_READY.value)
            info = None
            if only_errors:
                if state in (PodState.WAITING, PodState.TERMINATED, PodState.NOT_READY, PodState.NOT_READY_MOUNT,
                             PodState.FAILURE):
                    events_raw = self.kube_core.list_namespaced_event(
                        namespace=self.namespace,
                        field_selector=field_selector,
                        _preload_content=False,
                    )
                    events = json.loads(events_raw.data)
                    for event in events["items"]:
                        if container.name in event["metadata"]["name"]:
                            info = f'{event["reason"]} - {event["message"]}'

                    containers_errors.update(
                        {container.name: {
                            "state": state,
                            "info": info
                        }})
            else:
                containers_info.update(
                    {container.name: {
                        "state": state,
                        "info": info
                    }})
        return containers_errors if only_errors else containers_info

    def __str__(self):
        return f"Pod name: {self.pod_name}"

    def __repr__(self):
        return f"Pod name: {self.pod_name}"
