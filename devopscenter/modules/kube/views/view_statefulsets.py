""" Module that handle statefulset views. """

__author__ = "Mariano Jose Abdala"
__version__ = "0.1.0"

from rich.table import Table, Column

from devopscenter.modules.kube.views.base_view import ViewBase


class StatefulsetView(ViewBase):
    """ Main class that shows the statfulsets. """

    def execute(self, args):
        """
        Entrypoint for the view
        params: args arguments to be used to setup the view.
        """
        if len(args) == 2:
            self.__show_stateful_sets(args[1])
        else:
            self.__show_stateful_sets()

    def __show_stateful_sets(self, name_to_filter=None):
        """ Get the statefulsets and print them in a table.
            If the name_to_filter param is given just show that one
        """
        statfulsets = self.api.list_stateful_set_for_all_namespaces(
            timeout_seconds=60)
        stateful_set_obj = {}
        for stateful_set in statfulsets.items:
            stateful_name = stateful_set.metadata.name
            if name_to_filter is not None and name_to_filter not in stateful_name:
                continue
            stateful_set_obj.update({
                stateful_name: {
                    "namespace": stateful_set.metadata.namespace
                }
            })
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
                f"{stateful_set} ({index})",
            )
        self.print(table)
