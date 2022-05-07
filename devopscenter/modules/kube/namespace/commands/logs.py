""" Log Module for retrieve the logs of the pod."""

__author__ = "Mariano Jose Abdala"
__version__ = "0.1.0"

from kubernetes.client.exceptions import ApiException

from devopscenter.modules.kube.namespace.commands.base_cmd import BaseCmd
from devopscenter.modules.kube.cluster_utils import get_pods


class LogsCmd(BaseCmd):
    """Manage how the logs are printed."""

    def __init__(self, core, namespace):
        """Initialize a new log command."""
        super().__init__()
        self.core = core
        self.namespace = namespace

    def __exec_and_show_logs(self, pod_name, container_name):
        """ Exectute the command and print the logs."""
        try:
            logs = self.core.read_namespaced_pod_log(
                namespace=self.namespace,
                name=pod_name,
                container=container_name,
                _preload_content=False,
            )

            for line in logs.stream():
                self.print(line.decode("UTF-8"))
        except ApiException as api_ex:
            self.log(api_ex)
        except KeyboardInterrupt:
            self.log("Breaking logs")
            return
        except Exception as ex:  # pylint: disable=broad-except
            self.log(ex)

    def execute(self, args):
        """
        Does the real execution of the command.

        :param args arguments taken from the interface
        :param pods list of pods to be used
        """
        if len(args) < 2:
            self.log(
                "[red]Error you should select the number of the pod to show the log. Eg logs 0.0[/red]"
            )
            return

        try:
            pod_index, container_index = args[1].split(".")
        except ValueError:
            self.log(
                "[red]Error you should select the number of the pod to show the log. Eg logs 0.0[/red]"
            )
            return
        pods = get_pods(self.core, self.namespace)
        pod = self.get_pod(int(pod_index), pods)

        if pod is not None:
            containers = pod.get_containers_to_show()
            if len(containers) > 0:
                for index, item in enumerate(containers.items()):
                    if int(index) == int(container_index):
                        container_name, _ = item
                        self.__exec_and_show_logs(pod.pod_name, container_name)
