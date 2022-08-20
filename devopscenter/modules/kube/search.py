""" Module used to search a microservice on any namespace and
return the namespaces where it is located. """

__author__ = "Mariano Jose Abdala"
__version__ = "0.1.0"

from typing import Set

from rich.progress import Progress

from devopscenter.modules.kube.kube_base import KubeBase
from devopscenter.modules.kube.cluster_utils import get_namespace_names
from devopscenter.modules.kube.cluster_utils import get_pods


class Search(KubeBase):
    """
    This module is in charge of search any microservice into the namespaces.
    """

    def __init__(self, context) -> None:
        """Constructor."""
        super().__init__()
        self.api = self.cores_v1.get(context)
        self.context = context

    def get_toolbar(self):
        """ Get the labels to be used on the toolbar on the context. """
        return None

    def _get_label(self):
        return "search"

    def _get_cmd_label(self):  # pylint: disable=no-self-use
        return "search"

    def search_microservice(self, microservice_name) -> Set[str]:
        """
        This method will search the microservice on the namespace and if it find
        it will return the namespace name.
        :param microservice_name
        :return namespace name
        """
        namespaces = get_namespace_names(self.api.list_namespace())
        in_namespace = set()
        with Progress(transient=True) as progress:
            for namespace in progress.track(namespaces,
                                            description="Searching..."):
                pods = get_pods(self.api, namespace)
                for pod in pods:
                    if microservice_name in pod.pod_name:
                        in_namespace.add(namespace)
        return in_namespace

    def _do_work(self, command=None, args=None) -> None:
        namespace = self.search_microservice(command)
        if len(namespace) > 0:
            self.print(
                f"[blue]Namespace[/blue][reset]: [green]{namespace}[/green]"
            )
        else:
            self.print("[red]Not found[/red]", justify="center")
