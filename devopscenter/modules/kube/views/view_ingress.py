"""Module for Ingresses"""

__author__ = "Mariano Jose Abdala"
__version__ = "0.1.0"

from kubernetes.client.exceptions import ApiException
from rich.table import Table, Column

from devopscenter.modules.kube.views.base_view import ViewBase
from devopscenter.modules.kube.cluster_utils import pretty_annotations


class IngressView(ViewBase):
    """
    Class used to render the ResourceUsage of the pods.
    """

    def execute(self, args):
        """
        Entrypoint of the class where gets the arguments and use them.
        """
        if len(args) == 2:
            self.__show_ingress(args[1])
        else:
            self.__show_ingress()

    def __show_ingress(self, filter_value=None):
        """
        Method that render the resources of the pods.

        :param filter_value the filter_value to be apply to limit the resources to show.
        """
        ingresses = []

        try:
            ingresses = self.api.list_cluster_custom_object(group="extensions",
                                                            version="v1",
                                                            plural="ingresses",
                                                            pretty=True)
        except ApiException:
            self.print("No Resources found")
            return

        table = Table(
            Column("Namespace", style="green"),
            Column("Ingress Name", style="green"),
            Column("Annotations", style="green"),
            show_lines=True,
        )
        ingresses_por_namespace = {}

        for ingress in ingresses["items"]:
            ingress_name = ingress["metadata"]["name"]
            annot = ingress["metadata"]["annotations"]
            namespace = ingress["metadata"]["namespace"]

            if filter_value is not None and filter_value not in ingress_name:
                continue
            annotations = ""

            if annot is not None:
                annotations = list(
                    filter(lambda x: "ingress.kubernetes.io" in x[0],
                           annot.items()))
                if len(annotations) > 0:
                    ingresses_por_namespace.update({
                        f"{namespace}:{ingress_name}": {
                            "ingress_name": ingress_name,
                            "annotations": pretty_annotations(annotations),
                            "namespace": namespace,
                        }
                    })

        for ingress_name in ingresses_por_namespace.items():
            ingress = ingresses_por_namespace[ingress_name]

            table.add_row(
                ingress.get("namespace"),
                ingress.get("ingress_name"),
                str(ingress.get("annotations")),
            )
        self.print(table)
