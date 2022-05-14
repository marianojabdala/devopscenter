""" Manage the a bunch of operations that can be done on one selected namespace, like
kubectl -n <namespace> get pods, here the only command is pods, to retrieve all of them. """

__author__ = "Mariano Jose Abdala"
__version__ = "0.1.0"

from rich.table import Table, Column
from devopscenter.modules.kube.kube_base import KubeBase

from devopscenter.modules.kube.namespace.commands.base_cmd import BaseCmd
from devopscenter.modules.kube.namespace.commands.delete import DeleteCmd
from devopscenter.modules.kube.namespace.commands.logs import LogsCmd
from devopscenter.modules.kube.namespace.commands.pods import PodCmd
from devopscenter.modules.kube.namespace.commands.exec import ExecCmd


class Namespaces(KubeBase):
    """
    Class that is in charge of managing the namespace interaction
    """

    def __init__(self, context, namespace) -> None:
        """
        Constructor
        """
        super().__init__()
        self.api = self.cores_v1.get(context)
        self.context = context
        self.pods = []
        self.namespace = namespace
        self._register_commands()

    def _register_commands(self):
        """ Add the command to be used and showed in the toolbar. """
        self.commands = {
            "pods": PodCmd(self.api, self.namespace),
            "logs": LogsCmd(self.api, self.namespace),
            "exec": ExecCmd(self.api, self.namespace),
            "delete": DeleteCmd(self.api, self.namespace),
        }

    def show_help(self) -> None:
        """ Show the help of the commands. """
        table = Table(
            Column("Commands", style="green"),
            Column("Description", style=""),
            Column("Info", style=""),
        )
        table.add_row("pods", "Shows the pods",
                      "Show all the pods that are into the namespace")
        table.add_row(
            "logs",
            "Shows the logs of any selected pod",
            "This will shows the logs of all the containers on the pod",
        )
        table.add_row(
            "exec",
            "Execute the command in the underlining container",
            "Execute the command in the container and shows the result",
        )
        self.print(table)

    def _get_label(self):
        return self.context

    def _get_cmd_label(self):  # pylint: disable=no-self-use
        return self.namespace

    def _do_work(self, command=None, args=None) -> None:
        self.commands.get(command, BaseCmd()).execute(args)
