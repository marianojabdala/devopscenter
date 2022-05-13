""" Pods Module that will handle any pod command"""
__author__ = "Mariano Jose Abdala"
__version__ = "0.1.0"

from rich.table import Table, Column
from devopscenter.modules.kube.cluster_utils import get_pods
from devopscenter.modules.kube.namespace.commands.base_cmd import BaseCmd


class PodCmd(BaseCmd):
    """Manage the pods."""

    def execute(self, args: None) -> None:
        pods = get_pods(self.core, self.namespace)
        table = Table(
            Column("N°", style="blue"),
            Column("Pod", style="green"),
            Column("Container", style=""),
            Column("State", style=""),
            Column("Node", style=""),
        )
        for index, pod in enumerate(pods):
            containers = pod.get_containers_to_show()
            if len(containers) > 0:
                for index_container, item in enumerate(containers.items()):
                    container_name, container_info = item
                    info = container_info["state"] + (
                        ("-" + container_info["info"]
                         ) if container_info["info"] else ""
                    )  # pylint: disable=line-too-long
                    table.add_row(
                        f"{str(index)}.{index_container}",
                        pod.pod_name,
                        container_name,
                        info,
                        pod.node_name,
                    )

        self.print(table)
