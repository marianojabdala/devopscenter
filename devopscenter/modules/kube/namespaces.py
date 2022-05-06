# -*- coding: utf-8 -*-
__author__ = "Mariano Jose Abdala"
__version__ = "0.1.0"

import json
import shlex

from prompt_toolkit.completion import WordCompleter
from prompt_toolkit.patch_stdout import patch_stdout
from rich.table import Table, Column
from devopscenter.modules.kube.kube_base import KubeBase

from devopscenter.modules.kube.namespace.commands.delete import DeleteCmd
from devopscenter.modules.kube.namespace.commands.logs import LogsCmd
from devopscenter.modules.kube.namespace.commands.pods import PodCmd
from devopscenter.modules.kube.namespace.commands.health import HealthCmd
from devopscenter.modules.kube.namespace.commands.exec import ExecCmd


class Namespaces(KubeBase):
    """
    Class that is in charge of managing the namespace interaction
    """

    def __init__(self, context) -> None:
        """
        Constructor
        """
        super().__init__()
        self.core_v1 = self.cores_v1.get(context)
        self.context = context
        self.pods = []
        self.namespace = None

    def get_toolbar(self):
        return "Commands pods - logs - exec - health"

    def show_help(self) -> None:
        table = Table(
            Column("Commands", style="green"),
            Column("Description", style=""),
            Column("Info", style=""),
        )
        table.add_row("pods", "Shows the pods", "Show all the pods that are into the namespace")
        table.add_row(
            "logs",
            "Shows the logs of any selected pod",
            "This will shows the logs of all the containers on the pod",
        )
        table.add_row(
            "exec",
            "Execute the command in the underlining container",
            "Execute the command in the container and showd the result",
        )
        table.add_row(
            "health",
            "It shows the health endpoint of the pod",
            "If it is executed like [blue] health <number>[/blue] it execute the default health check that in on java, if you add the language that health will be executed",
        )
        self.print(table)

    def namespace_center(self, current_ns):
        self.namespace = current_ns
        while True:
            try:
                with patch_stdout(raw=True):
                    self.session.completer = None
                    text = self.session.prompt(
                        [
                            ("fg:green bold", "(" + self.context + "):"),
                            ("fg:ansimagenta bold", current_ns),
                            ("", ":>$"),
                        ],
                        bottom_toolbar=self.get_toolbar(),
                    ).strip()

                    if text == "exit":
                        break
                    elif text == "help" or text == "h":
                        self.show_help()
                    elif "delete" in text or "del" in text:
                        args = shlex.split(text)
                        cmd = DeleteCmd(self.core_v1, self.namespace)
                        cmd.execute(args, self.pods)
                    elif "logs" in text:
                        args = shlex.split(text)
                        cmd = LogsCmd(self.core_v1, self.namespace)
                        cmd.execute(args, self.pods)
                    elif text == "pods":
                        cmd = PodCmd(self.context, self.core_v1, self.namespace)
                        self.pods = cmd.execute()
                    elif "exec" in text:
                        args = shlex.split(text)
                        cmd = ExecCmd(self.core_v1, self.namespace)
                        response = cmd.execute(args, self.pods)
                        self.print(response)
                    elif "health" in text:
                        args = shlex.split(text)
                        cmd = HealthCmd(self.core_v1, self.namespace)
                        cmd.execute(args, self.pods)
            except KeyboardInterrupt:
                continue  # Control-C pressed. Try again.
            except EOFError:
                break  # Control-D pressed.

    def start(self) -> None:
        try:
            with open(self.base_path_namespaces, "r", encoding="utf-8") as ns:
                namespaces = json.loads(ns.read())

        except (IOError, FileNotFoundError):
            self.log("[red]Error opening the file namespaces[red]")
            return

        ns_completer = WordCompleter(namespaces, WORD=True)
        self.session.completer = ns_completer

        while True:
            try:
                with patch_stdout(raw=True):
                    self.session.completer = ns_completer
                    text = self.session.prompt(
                        [
                            ("fg:green bold", "(" + self.context + "):"),
                            ("fg:ansimagenta bold", "namespaces"),
                            ("", ":>$"),
                        ],
                        bottom_toolbar="",
                    ).strip()

                if text == "exit":
                    break

                if text in namespaces:
                    self.namespace_center(text)

            except KeyboardInterrupt:
                continue  # Control-C pressed. Try again.
            except EOFError:
                break  # Control-D pressed.
