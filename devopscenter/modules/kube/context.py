# -*- coding: utf-8 -*-
__author__ = "Mariano Jose Abdala"
__version__ = "0.1.0"


import json

from rich.table import Table, Column
from prompt_toolkit.shortcuts import ProgressBar
from prompt_toolkit.patch_stdout import patch_stdout

from devopscenter.modules.kube.kube_base import KubeBase
from devopscenter.modules.kube.cluster_utils import get_namespace_names
from devopscenter.modules.kube.namespaces import Namespaces
from devopscenter.modules.kube.search import Search
from devopscenter.modules.kube.views.custom_views import CustomViews


class Context(KubeBase):
    def __init__(self, context_name) -> None:
        super().__init__()
        self.context = context_name
        self.setup(context_name)

    def setup(self, context_name) -> None:
        core_v1 = self.cores_v1.get(context_name)
        namespaces = get_namespace_names(core_v1.list_namespace())
        namespaces_to_save = []

        try:
            with ProgressBar("Initializing Context") as pb:
                for namespace in pb(namespaces, label="Getting info"):
                    namespaces_to_save.append(namespace)

        except Exception:
            self.console.print_exception()
        with open(self.base_path_namespaces, "w", encoding="utf-8") as kube:
            kube.write(json.dumps(namespaces_to_save, indent=2))

    def get_toolbar(self):
        return "Commands: ns - search - views"

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
                    )
                    command = text.strip()
                    if command == "exit":
                        break
                    elif command == "help":
                        self.show_help()
                    elif command == "views":
                        cv = CustomViews(self.context)
                        cv.start()
                    elif command == "ns":
                        ns = Namespaces(self.context)
                        ns.start()
                    elif command == "search":
                        search = Search(self.context)
                        search.start()

            except KeyboardInterrupt:
                continue  # Control-C pressed. Try again.
            except EOFError:
                break  # Control-D pressed.
