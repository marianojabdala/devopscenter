""" Horizontal Pod Autoscaling module"""
__author__ = "Mariano Jose Abdala"
__version__ = "0.1.0"

from rich.table import Table, Column

from devopscenter.modules.kube.views.base_view import ViewBase


class HpaView(ViewBase):
    """ Class that will visualize the hpa. """

    def __init__(self, autoscaling, context):
        """ Constructor. """
        super().__init__()
        self.autoscaling = autoscaling
        self.context = context

    def execute(self, args):
        """ Retrieve the hpa using the args"""
        if len(args) == 2:
            self.__show_hpa(args[1])
        else:
            self.__show_hpa()

    def __show_hpa(self, name_to_filter=None):
        """ Get the deployment and also print them, if the name_to_filter param is given, just
        show only the ones that belogns to the name_to_filter. """

        hpa_list = self.autoscaling.list_horizontal_pod_autoscaler_for_all_namespaces(
            timeout_seconds=60)
        hpa_list_obj = {}
        for hpa in hpa_list.items:
            hpa_name = hpa.metadata.name
            if name_to_filter is not None and name_to_filter not in hpa_name:
                continue
            hpa_list_obj.update({
                hpa_name: {
                    "namespace": hpa.metadata.namespace,
                    "min_replicas": str(hpa.spec.min_replicas),
                    "max_replicas": str(hpa.spec.max_replicas),
                    "current_replicas": str(hpa.status.current_replicas),
                }
            })
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
