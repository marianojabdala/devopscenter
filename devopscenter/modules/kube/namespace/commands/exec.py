""" Module that be in charge of make calls to the pods and execute a command."""

__author__ = "Mariano Jose Abdala"
__version__ = "0.1.0"

from kubernetes.stream import stream
from devopscenter.modules.kube.namespace.commands.base_cmd import BaseCmd
from devopscenter.modules.kube.cluster_utils import get_container_name


class ExecCmd(BaseCmd):
    """Manage the execution of the command."""

    def exec_command(self, pods, pod_container_index, command):
        """ Execute the given command on the selected pod and return the response. """
        exec_command = [
            "/bin/sh",
            "-c",
        ] + command
        response = ""
        pod_index, container_index = pod_container_index.split(".")
        pod = self.get_pod(int(pod_index), pods)
        container_name = get_container_name(pod, int(container_index))
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

    def execute(self, args, index=None, commands=None):
        """ Execute a command into the pod using the args and print the response.
        :param args arguments to be used,
        :param index, the index to used of the selected pod.
        :param commands to execute in the pod. Eg. ls
        """
        pods = self.get_pods(self.core, self.namespace)
        if args is not None:
            response = self.exec_command(pods, args[1], args[2:])
        else:
            response = self.exec_command(pods, index, commands)

        self.print(response)
