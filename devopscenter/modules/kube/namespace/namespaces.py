""" Manage the a bunch of operations that can be done on one selected namespace, like
kubectl -n <namespace> get pods, here the only command is pods, to retrieve all of them. """

__author__ = "Mariano Jose Abdala"
__version__ = "0.1.0"

import shlex

from prompt_toolkit.patch_stdout import patch_stdout
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
        self.core_v1 = self.cores_v1.get(context)
        self.context = context
        self.pods = []
        self.namespace = namespace
        self._register_commands()

    def _register_commands(self):
        """ Add the command to be used and showed in the toolbar. """
        self.commands = {
            "pods": PodCmd(self.core_v1, self.namespace),
            "logs": LogsCmd(self.core_v1, self.namespace),
            "exec": ExecCmd(self.core_v1, self.namespace),
            "delete": DeleteCmd(self.core_v1, self.namespace),
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

    def start(self) -> None:
        """ Start in the selected namespace. """
        while True:
            try:
                with patch_stdout(raw=True):
                    self.session.completer = None
                    text = self.session.prompt(
                        [
                            ("fg:green bold", "(" + self.context + "):"),
                            ("fg:ansimagenta bold", self.namespace),
                            ("", ":>$"),
                        ],
                        bottom_toolbar=self.get_toolbar(),
                    ).strip()

                    args = shlex.split(text)
                    command = args[0]

                    if command == "exit":
                        break
                    if command in ("help", "h"):
                        self.show_help()

                    self.commands.get(command, BaseCmd()).execute(args)

            except KeyboardInterrupt:
                continue  # Control-C pressed. Try again.
            except EOFError:
                break  # Control-D pressed.
