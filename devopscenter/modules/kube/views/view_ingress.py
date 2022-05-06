# -*- coding: utf-8 -*-
__author__ = "Mariano Jose Abdala"
__version__ = "0.1.0"
from rich.table import Table, Column

from devopscenter.modules.kube.views.base_view import ViewBase


class IngressView(ViewBase):
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
            self.__show_ingress(args[1])
        else:
            self.__show_ingress()

    def __pretty_annotations(self, annotations):
        new_annotations = []
        for annotation in annotations:
            key, value = annotation
            formated = f"{key}:{value}"
            new_annotations.append(formated)
        return "\n".join(new_annotations)

    def __show_ingress(self, filter_value=None):
        """
        Method that render the resources of the pods.

        :param filter_value the filter_value to be apply to limit the resources to show.
        """

        ingresses = self.custom_api.list_cluster_custom_object(
            group="extensions", version="v1beta1", plural="ingresses", pretty=True
        )
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
                annotations = list(filter(lambda x: "ingress.kubernetes.io" in x[0], annot.items()))
                if len(annotations) > 0:
                    ingresses_por_namespace.update(
                        {
                            f"{namespace}:{ingress_name}": {
                                "ingress_name": ingress_name,
                                "annotations": self.__pretty_annotations(annotations),
                                "namespace": namespace,
                            }
                        }
                    )

        for ingress_name in ingresses_por_namespace:
            ingress = ingresses_por_namespace[ingress_name]

            table.add_row(
                ingress.get("namespace"),
                ingress.get("ingress_name"),
                str(ingress.get("annotations")),
            )
        self.print(table)
