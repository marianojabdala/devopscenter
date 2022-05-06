# -*- coding: utf-8 -*-
__author__ = "Mariano Jose Abdala"
__version__ = "0.1.0"

from devopscenter.modules.kube.namespace.commands.base_cmd import BaseCmd


class DeleteCmd(BaseCmd):
    def __init__(self, core, namespace):
        super().__init__()
        self.core = core
        self.namespace = namespace

    def execute(self, args, pods):
        if len(args) == 0:
            self.log("[red]Error please select the number to delete[/red]")
            return
        pod_index, container_index = args[1].split(".")
        pod = self.get_pod(int(pod_index), pods)
        if pod is not None:
            try:
                self.core.delete_namespaced_pod(pod.pod_name, self.namespace)
            except Exception as apiex:
                self.log(apiex)
