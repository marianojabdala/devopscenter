# -*- coding: utf-8 -*-
__author__ = "Mariano Jose Abdala"
__version__ = "0.1.0"

from devopscenter.modules.kube.views.base_view import ViewBase
from devopscenter.modules.kube.cluster_utils import get_pods

from rich.panel import Panel


class PodResourcesView(ViewBase):
    def __init__(self, core, context):
        """
        Class constructor.
        """
        super().__init__()
        self.core_v1 = core
        self.context = context

    def execute(self, args):
        """
        Entrypint of the class that will be used to show the resources.

        :param args
        """
        if len(args) > 1:
            self.__show_pod_resources(str(args[1]))
        else:
            self.__show_pod_resources()

    def __show_pod_resources(self, filter=None):
        """
        This will get all the pods and show the resources(limits,request)
        if filter is given just show those resources if not the entire
        pods on the cluster.

        :param filter restrict the pods to be shown.
        """
        with self.console.status("Getting resouces"):
            pods = get_pods(self.core_v1)
        panels = []
        for pod in pods:
            pod_name = pod.pod_name
            if filter is not None and filter not in pod_name:
                continue

            containers = pod.containers

            for container in containers:
                resources = container.resources
                panels.append(
                    Panel(
                        f"[white]Requests: {str(resources.requests)} \nLimits: {str(resources.limits)}[/white]",
                        title=f"[blue]Namespace: {pod.namespace} [/blue]- [green]Container: {container.name}[/green][reset]",
                        expand=False,
                    )
                )
        for panel in panels:
            self.print(panel)
