# -*- coding: utf-8 -*-
__author__ = "Mariano Jose Abdala"
__version__ = "0.1.0"

from rich.table import Table, Column

from devopscenter.modules.kube.views.base_view import ViewBase


class HpaView(ViewBase):
    def __init__(self, autoscaling, context):
        super().__init__()
        self.autoscaling = autoscaling
        self.context = context

    def execute(self, args):
        if len(args) == 2:
            self.__show_hpa(args[1])
        else:
            self.__show_hpa()

    def __show_hpa(self, filter=None):
        hpa_list = self.autoscaling.list_horizontal_pod_autoscaler_for_all_namespaces(
            timeout_seconds=60
        )
        hpa_list_obj = {}
        for hpa in hpa_list.items:
            hpa_name = hpa.metadata.name
            if filter is not None and filter not in hpa_name:
                continue
            hpa_list_obj.update(
                {
                    hpa_name: {
                        "namespace": hpa.metadata.namespace,
                        "min_replicas": str(hpa.spec.min_replicas),
                        "max_replicas": str(hpa.spec.max_replicas),
                        "current_replicas": str(hpa.status.current_replicas),
                    }
                }
            )
            table = Table(
                Column("Cluster", style="green"),
                Column("Namespace", style="green"),
                Column("Hpa name", style=""),
                Column("min replicas", style=""),
                Column("max replicas", style=""),
                Column("current replicas", style=""),
            )

            for index, hpa in enumerate(hpa_list_obj):
                data = hpa_list_obj.get(hpa)
                table.add_row(
                    self.context,
                    data["namespace"],
                    f"{hpa} ({index})",
                    data["min_replicas"],
                    data["max_replicas"],
                    data["current_replicas"],
                )
        self.print(table)
