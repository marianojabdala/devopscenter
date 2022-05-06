# -*- coding: utf-8 -*-
__author__ = "Mariano Jose Abdala"
__version__ = "0.1.0"


import os
from kubernetes import client, config
from pathlib import Path
from devopscenter.base import Base


class KubeBase(Base):
    def __init__(self):
        super().__init__()
        self.initialize_contexts()

    def initialize_contexts(self):
        if not hasattr(self, "contexts"):
            self.base_path_namespaces = os.path.join(self.base_path, "namespaces.json")
            kubefiles = []
            kube_home = f"{str(Path.home())}/.kube/"
            for root, _, files in os.walk(kube_home):
                if "cache" in root:
                    continue
                kubefiles = files

            self.contexts = []
            self.cores_v1 = {}
            self.apps_v1 = {}
            self.custom_apis = {}
            self.autoscalings = {}
            self.extensionsV1Beta1apis = {}

            for file in kubefiles:
                kube_config = f"{kube_home}{file}"
                config.load_kube_config(kube_config)
                current_cluster_name = config.list_kube_config_contexts(kube_config)[1]["name"]
                if current_cluster_name not in self.contexts:
                    self.contexts.append(current_cluster_name)
                    self.cores_v1.update({current_cluster_name: client.CoreV1Api()})
                    self.apps_v1.update({current_cluster_name: client.AppsV1Api()})
                    self.autoscalings.update({current_cluster_name: client.AutoscalingV1Api()})
                    self.custom_apis.update({current_cluster_name: client.CustomObjectsApi()})
                    self.extensionsV1Beta1apis.update(
                        {current_cluster_name: client.NetworkingV1Api()}
                    )
