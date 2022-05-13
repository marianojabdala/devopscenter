""" Context Module that will be used to reference the kubeconfig context
loaded from the files """
# -*- coding: utf-8 -*-
__author__ = "Mariano Jose Abdala"
__version__ = "0.1.0"

from rich.table import Table, Column

from devopscenter.modules.kube.kube_base import KubeBase
from devopscenter.modules.kube.not_found import NotFound
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

    def _do_work(self, command=None, args=None) -> None:
        self.commands.get(command, NotFound()).start()

    def _get_label(self):
        return self.context

    def _get_cmd_label(self):  # pylint: disable=no-self-use
        return "context"
