""" Base class that concentrate all the common funcionality. """

__author__ = "Mariano Jose Abdala"
__version__ = "0.1.0"

import os
import shlex
from pathlib import Path

from prompt_toolkit.patch_stdout import patch_stdout
from prompt_toolkit.completion import WordCompleter

from kubernetes import client, config

from devopscenter.base import Base


class KubeBase(Base):  # pylint: disable=too-many-instance-attributes
    """ Base class that will instanciate all the objects the system needs. """

    def __init__(self) -> None:
        super().__init__()
        self.commands = {}
        self.k8s_client = client
        self.contexts = []
        self.cores_v1 = {}
        self.apps_v1 = {}
        self.custom_apis = {}
        self.autoscalings = {}
        self.session.completer = None

        self.initialize_contexts()

    def get_text(self, label, cmd_label) -> str:
        """Returns the text that is given on the prompt"""
        text = self.session.prompt(
            [
                ("fg:green bold", "(" + label + "):"),
                ("fg:ansimagenta bold", cmd_label),
                ("", ":>$"),
            ],
            bottom_toolbar=self.get_toolbar(),
        ).strip()
        return text

    def _get_label(self):  # pylint: disable=no-self-use
        return "None"

    def _get_cmd_label(self):  # pylint: disable=no-self-use
        return "None"

    def start(self) -> None:
        """ Start in the selected context. """
        ns_completer = WordCompleter(list(self.commands.keys()), WORD=True)
        self.session.completer = ns_completer
        command = None
        while True:
            try:
                with patch_stdout(raw=True):
                    text = self.get_text(self._get_label(),
                                         self._get_cmd_label())
                    args = shlex.split(text)

                    if len(args) > 0:
                        command = args[0]
                        if command == "exit":
                            break
                        if command in ("help", "h"):
                            self.show_help()
                            continue

                    self._do_work(command, args)

            except KeyboardInterrupt:
                continue  # Control-C pressed. Try again.
            except EOFError:
                break  # Control-D pressed.

    def show_help(self):  # pylint: disable=no-self-use
        """ Show help of the class."""
        self.print("[red] help not found!!![/red]")

    def _do_work(self, command=None, args=None) -> None:  # pylint: disable=unused-argument
        """ Method that is the default when the command introduced isn't available. """
        self.print("[red]Command not found!!![/red]")

    def get_toolbar(self):
        """ Get the labels to be used on the toolbar on the context. """
        return f'Commands: {" ".join(list(self.commands.keys()))}'

    def initialize_contexts(self):
        """ Retrieves all the contexts from the files in the folder $HOME/.kube
            and creates a map with the possible funcionalities that can be used
            on the system.
        """
        if hasattr(self, "contexts") and len(self.contexts) == 0:
            self.base_path_namespaces = os.path.join(self.base_path,
                                                     "namespaces.json")
            kubefiles = []
            kube_home = f"{str(Path.home())}/.kube/"
            for root, _, files in os.walk(kube_home):
                if "cache" in root:
                    continue
                kubefiles = files

            for file in kubefiles:
                kube_config = f"{kube_home}{file}"
                config.load_kube_config(kube_config)
                current_cluster_name = config.list_kube_config_contexts(
                    kube_config)[1]["name"]
                if current_cluster_name not in self.contexts:
                    self.contexts.append(current_cluster_name)
                    self.cores_v1.update(
                        {current_cluster_name: client.CoreV1Api()})
                    self.apps_v1.update(
                        {current_cluster_name: client.AppsV1Api()})
                    self.autoscalings.update(
                        {current_cluster_name: client.AutoscalingV1Api()})
                    self.custom_apis.update(
                        {current_cluster_name: client.CustomObjectsApi()})
