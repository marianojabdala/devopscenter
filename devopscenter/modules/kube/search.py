# -*- coding: utf-8 -*-
__author__ = "Mariano Jose Abdala"
__version__ = "0.1.0"

import json

from prompt_toolkit.patch_stdout import patch_stdout
from rich.progress import Progress

from devopscenter.modules.kube.kube_base import KubeBase
from devopscenter.modules.kube.cluster_utils import get_pods


class Search(KubeBase):
    """
    This module is in charge of search any microservice into the namespaces.
    """

    def __init__(self, context) -> None:
        """Constructor."""
        super().__init__()
        self.core_v1 = self.cores_v1.get(context)
        self.context = context

    def search_microservice(self, microservice_name) -> str:
        """
        This method will search the microservice on the namespace and if it find
        it will return the namespace name.
        :param microservice_name
        :return namespace name
        """
        try:
            with open(self.base_path_namespaces, "r", encoding="utf-8") as ns:
                namespaces = json.loads(ns.read())

        except (IOError, FileNotFoundError):
            self.log("[red]Error opening namespaces[red]")
            return
        in_namespace = set()
        with Progress(transient=True) as progress:
            for namespace in progress.track(namespaces, description="Searching..."):
                pods = get_pods(self.core_v1, namespace)
                for pod in pods:
                    if microservice_name in pod.pod_name:
                        in_namespace.add(namespace)
        return in_namespace

    def start(self) -> None:
        """
        Entry point for the search command.
        This is a forever loop that will ask for the command to use.
        """
        while True:
            try:
                with patch_stdout(raw=True):
                    text = self.session.prompt(
                        [
                            ("fg:green bold", f"({self.context})"),
                            ("fg:ansimagenta bold", "search"),
                            ("", ":>$"),
                        ],
                    ).strip()

                if text == "exit":
                    break
                if text == "help":
                    self.print("[blue]You have to add the microservice to search[/blue]")
                elif text != "":
                    namespace = self.search_microservice(text)
                    if len(namespace) > 0:
                        self.log(f"[blue]Namespace[/blue][reset]: [green]{namespace}[/green]")
                    else:
                        self.log("[red]Not found[/red]")
            except KeyboardInterrupt:
                continue  # Control-C pressed. Try again.
            except EOFError:
                break  # Control-D pressed.
