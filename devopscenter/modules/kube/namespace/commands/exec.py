# -*- coding: utf-8 -*-
__author__ = "Mariano Jose Abdala"
__version__ = "0.1.0"

from kubernetes.stream import stream
from devopscenter.modules.kube.namespace.commands.base_cmd import BaseCmd


class ExecCmd(BaseCmd):
    """Manage the execution of the command."""

    def __init__(self, core, namespace):
        """Initialize a new exec command."""
        super().__init__()
        self.core = core
        self.namespace = namespace

    def exec_command(self, pods, pod_container_index, command):
        exec_command = [
            "/bin/sh",
            "-c",
        ] + command
        response = ""
        pod_index, container_index = pod_container_index.split(".")

        pod = self.get_pod(int(pod_index), pods)
        if pod is not None:
            containers = pod.get_containers_to_show()
            if len(containers) > 0:
                for index, item in enumerate(containers.items()):
                    if index == int(container_index):
                        container_name, _ = item
                        response = stream(
                            self.core.connect_get_namespaced_pod_exec,
                            name=pod.pod_name,
                            namespace=self.namespace,
                            command=exec_command,
                            container=container_name,
                            stderr=True,
                            stdin=False,
                            stdout=True,
                            tty=False,
                            _preload_content=True,
                        )

        return response

    def execute(self, args, pods, index=None, commands=None):
        if args is not None:
            response = self.exec_command(pods, args[1], args[2:])
        else:
            response = self.exec_command(pods, index, commands)
        return response
