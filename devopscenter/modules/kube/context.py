""" Context Module that will be used to reference the kubeconfig context
loaded from the files """
# -*- coding: utf-8 -*-
__author__ = "Mariano Jose Abdala"
__version__ = "0.1.0"

import shlex

from prompt_toolkit.patch_stdout import patch_stdout
from rich.table import Table, Column

from devopscenter.modules.kube.kube_base import KubeBase
from devopscenter.modules.kube.namespace.namespaces_manager import NamespacesManager
from devopscenter.modules.kube.search import Search
from devopscenter.modules.kube.views.custom_views import CustomViews


class Context(KubeBase):
    """
    Class that encapsulate commands that can be used on any kubernetes context like
    searching a microservice, get into a namespace, show custom views.
    """

    def __init__(self, context_name) -> None:
        """ Constructor. """
        super().__init__()
        self.context = context_name
        self._register_commands()

    def _register_commands(self):
        """ Add the command to be used and showed in the toolbar. """
        self.commands = {
            "ns": NamespacesManager(self.context),
            "search": Search(self.context),
            "views": CustomViews(self.context),
        }

    def get_toolbar(self):
        """ Get the labels to be used on the toolbar on the context. """
        return f'Commands: {" ".join(list(self.commands.keys()))}'

    def show_help(self) -> None:
        """
        Shows the commands that can be used.
        """
        table = Table(
            Column("Commands", style="green"),
            Column("Description", style=""),
        )
        table.add_row("ns", "Interact with namespaces")
        table.add_row("search", "Look for a microservice into the namespaces")
        table.add_row("views", "Shows distinct views")
        self.print(table)

    def start(self) -> None:
        """ Start in the selected context. """
        while True:
            try:
                with patch_stdout(raw=True):
                    text = self.session.prompt(
                        [
                            ("fg:green bold", "(" + self.context + "):"),
                            ("fg:ansimagenta bold", "context"),
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

                    self.commands.get(command, KubeBase()).start()

            except KeyboardInterrupt:
                continue  # Control-C pressed. Try again.
            except EOFError:
                break  # Control-D pressed.
