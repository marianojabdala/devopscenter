""" Module in charge of Persistent volume claims. """

__author__ = "Mariano Jose Abdala"
__version__ = "0.1.0"

from rich.table import Table, Column

from devopscenter.modules.kube.views.base_view import ViewBase
from devopscenter.modules.kube.cluster_utils import get_pods


class PvcView(ViewBase):
    """ Class used to show Persistent Volume Claims """

    def __init__(self, core, context):
        """
        Constructor.
        """
        super().__init__()
        self.core_v1 = core
        self.context = context

    def execute(self, args):
        """
        Entrypoint for the view
        params: args arguments to be used to setup the view.
        """
        if len(args) == 2:
            self.__show_pvc_table(args[1])
        else:
            self.__show_pvc_table()

    def __show_pvc_table(self, name_to_filter=None):
        """
        Shows a table for all the pvc.
        :param name_to_filter the name_to_filter to use to get the pvc
        """
        with self.console.status("Working..."):
            pods = get_pods(self.core_v1)

        pvcs = self.core_v1.list_persistent_volume_claim_for_all_namespaces(
            timeout_seconds=60)
        pvc_obj = {}

        with self.console.status("Working..."):
            for pvc in pvcs.items:
                pvc_name = pvc.metadata.name
                if name_to_filter is not None and name_to_filter not in pvc_name:
                    continue
                pvc_obj.update({
                    pvc.metadata.name: {
                        "namespace": pvc.metadata.namespace,
                        "volumen_name": pvc.spec.volume_name,
                        "storage": pvc.spec.resources.requests["storage"],
                    }
                })

            for _, pod in enumerate(pods):
                if filter is not None and filter not in pod.pod_name:
                    continue

                volumenes = pod.volumes
                if volumenes is not None:
                    for volumen in volumenes:
                        claim = volumen.persistent_volume_claim
                        if claim is not None and claim != "":
                            data = pvc_obj.get(claim.claim_name)
                            if data is not None:
                                data.update({"pod": pod.pod_name})
                            else:
                                data = {}
                                data.update({"pod": "no pod"})
                            pvc_obj.update({claim.claim_name: data})
            table = Table(
                Column("Cluster", style="green"),
                Column("Namespace", style="green"),
                Column("Pod Name", style=""),
                Column("Pvc", style=""),
                Column("Capacity", style=""),
            )
            for pvc in pvc_obj:
                data = pvc_obj.get(pvc)
                table.add_row(
                    self.context,
                    data["namespace"]
                    if "namespace" in data else "no namespace",
                    data["pod"] if "pod" in data else "sin pod",
                    pvc,
                    data["storage"] if "storage" in data else "no storage",
                )
        self.print(table)
