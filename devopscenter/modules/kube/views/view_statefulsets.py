# -*- coding: utf-8 -*-
__author__ = "Mariano Jose Abdala"
__version__ = "0.1.0"

from rich.table import Table, Column

from devopscenter.modules.kube.views.base_view import ViewBase


class StatefulsetView(ViewBase):
    def __init__(self, app, context):
        super().__init__()
        self.app_v1 = app
        self.context = context

    def execute(self, args):
        if len(args) == 2:
            self.__show_stateful_sets(args[1])
        else:
            self.__show_stateful_sets()

    def __show_stateful_sets(self, filter=None):
        statfulsets = self.app_v1.list_stateful_set_for_all_namespaces(
            timeout_seconds=60
        )
        stateful_set_obj = {}
        self.log(len(statfulsets.items))
        for stateful_set in statfulsets.items:
            stateful_name = stateful_set.metadata.name
            if filter is not None and filter not in stateful_name:
                continue
            stateful_set_obj.update(
                {stateful_name: {"namespace": stateful_set.metadata.namespace}}
            )
        table = Table(
            Column("Cluster", style="green"),
            Column("Namespace", style="green"),
            Column("Statefulset", style=""),
        )

        for index, stateful_set in enumerate(stateful_set_obj):
            data = stateful_set_obj.get(stateful_set)
            table.add_row(
                self.context,
                data["namespace"],
                f"stateful_set ({index})",
            )
        self.print(table)
