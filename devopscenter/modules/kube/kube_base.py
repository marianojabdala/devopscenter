""" Base class that concentrate all the common funcionality. """

__author__ = "Mariano Jose Abdala"
__version__ = "0.1.0"

import os
from pathlib import Path

from kubernetes import client, config

from devopscenter.base import Base


class KubeBase(Base):  # pylint: disable=too-many-instance-attributes
    """ Base class that will instanciate all the objects the system needs. """

    def __init__(self):
        super().__init__()
        self.commands = {}
        self.k8s_client = client
        self.contexts = []
        self.cores_v1 = {}
        self.apps_v1 = {}
        self.custom_apis = {}
        self.autoscalings = {}

        self.initialize_contexts()

    def start(self) -> None:
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
