""" Module for Pod Resources(limits, requests)"""
__author__ = "Mariano Jose Abdala"
__version__ = "0.1.0"

from rich.panel import Panel

from devopscenter.modules.kube.views.base_view import ViewBase
from devopscenter.modules.kube.cluster_utils import get_pods


class PodResourcesView(ViewBase):
    """ Class in charge of get the pods resources and show them. """

    def execute(self, args):
        """
        Entrypint of the class that will be used to show the resources.

        :param args
        """
        if len(args) > 1:
            self.__show_pod_resources(str(args[1]))
        else:
            self.__show_pod_resources()

    def __show_pod_resources(self, name_to_filter=None):
        """
        This will get all the pods and show the resources(limits,request)
        if name_to_filter is given just show those resources if not the entire
        pods on the cluster.

        :param name_to_filter restrict the pods to be shown.
        """
        with self.console.status("Getting resouces"):
            pods = get_pods(self.api)
        panels = []
        for pod in pods:
            pod_name = pod.pod_name
            if name_to_filter is not None and name_to_filter not in pod_name:
                continue

            containers = pod.containers

            for container in containers:
                resources = container.resources
                panels.append(
                    self.create_panel(str(resources.requests),
                                      str(resources.limits), pod.namespace,
                                      container.name))

        for panel in panels:
            self.print(panel)

    def create_panel(  # pylint: disable=no-self-use
            self, requests, limits, namespace, container_name) -> Panel:
        """ Creates the panel to be shown. """
        namespace_title = f"[blue]Namespace: {namespace} [/blue]"
        container_title = f"[green]Container: {container_name}[/green][reset]"
        title = namespace_title + "-" + container_title
        body = f"[white]Requests: {requests} \nLimits: {limits}[/white]"
        return Panel(body, title=title, expand=False)
