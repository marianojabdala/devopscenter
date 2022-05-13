""" Module that get the resources usage from the metrics server. """

__author__ = "Mariano Jose Abdala"
__version__ = "0.1.0"

from operator import itemgetter
from rich.table import Table, Column

from devopscenter.modules.kube.views.base_view import ViewBase
from devopscenter.modules.kube.cluster_utils import convert_to_mi
from devopscenter.modules.kube.cluster_utils import convert_to_milicore


class ResourceUsageView(ViewBase):
    """
    Class used to render the ResourceUsage of the pods.
    """

    def __init__(self, custom_api):
        """
        Class Constructor.
        """
        super().__init__()
        self.custom_api = custom_api

    def execute(self, args):
        """
        Entrypoint of the class where gets the arguments and use them.
        """
        if len(args) == 2:
            self.__show_resource_usage(args[1])
        else:
            self.__show_resource_usage()

    def __show_resource_usage(self, name_to_filter=None):
        """
        Method that render the resources of the pods.

        :param name_to_filter the name_to_filter to be apply to limit the resources to show.
        """
        pods = self.custom_api.list_cluster_custom_object(
            group="metrics.k8s.io",
            version="v1beta1",
            plural="pods",
            pretty=True)
        table = Table(
            Column("Namespace", style="green"),
            Column("Pod Name", style="green"),
            Column("Container Name", style="green"),
            Column("Cpu", style="", justify="right", no_wrap=True),
            Column("Memory", style="", justify="right", no_wrap=True),
        )
        pods_por_namespace = {}
        for pod in pods["items"]:
            pod_name = pod["metadata"]["name"]

            if name_to_filter is not None and name_to_filter not in pod_name:
                continue

            containers = pod["containers"]
            namespace = pod["metadata"]["namespace"]
            ns_obj = pods_por_namespace.get(namespace)

            if ns_obj is None:
                ns_obj = {}
            ns_obj.update({pod_name: {
                "containers": containers,
            }})
            pods_por_namespace.update({namespace: ns_obj})
        usage_info_list = []
        for namespace, pods in pods_por_namespace.items():
            for pod in pods:
                containers = pods[pod]["containers"]
                for container in containers:
                    usage_info_list.append({
                        "namespace":
                        namespace,
                        "pod":
                        pod,
                        "container_name":
                        container["name"],
                        "cpu":
                        convert_to_milicore(container["usage"]["cpu"]),
                        "memory":
                        convert_to_mi(container["usage"]["memory"], False),
                    })

        usage_info_list_sorted = sorted(usage_info_list,
                                        key=itemgetter("memory"),
                                        reverse=True)
        for item in usage_info_list_sorted:
            table.add_row(
                item.get("namespace"),
                item.get("pod"),
                item.get("container_name"),
                item.get("cpu"),
                f'{str(item.get("memory"))}Mi',
            )
        self.print(table)
