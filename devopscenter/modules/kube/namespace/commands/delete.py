""" Delete module that will handle all the deletion process of a pod. """

__author__ = "Mariano Jose Abdala"
__version__ = "0.1.0"

from kubernetes.client.exceptions import ApiException

from devopscenter.modules.kube.namespace.commands.base_cmd import BaseCmd
from devopscenter.modules.kube.cluster_utils import get_pods


class DeleteCmd(BaseCmd):
    """ Class in charge of delete a pod. """

    def __init__(self, core, namespace):
        """ Constructor. """
        super().__init__()
        self.core = core
        self.namespace = namespace

    def execute(self, args):
        """ Execute the deletion of the pod using the args."""
        if len(args) == 0:
            self.log("[red]Error please select the number to delete[/red]")
            return
        pods = get_pods(self.core, self.namespace)
        pod_index, _ = args[1].split(".")
        pod = self.get_pod(int(pod_index), pods)
        if pod is not None:
            try:
                self.core.delete_namespaced_pod(pod.pod_name, self.namespace)
            except ApiException as api_ex:
                self.log(api_ex)
