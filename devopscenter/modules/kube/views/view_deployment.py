"""Deployment Module, this will get all the deployment related stuff"""

__author__ = "Mariano Jose Abdala"
__version__ = "0.1.0"

from rich.table import Table, Column

from devopscenter.modules.kube.views.base_view import ViewBase


class DeployView(ViewBase):
    """ Main class. """

    def execute(self, args):
        """ Retrieve the deploy using the args"""
        if len(args) == 2:
            self.__show_deploy(args[1])
        else:
            self.__show_deploy()

    def __show_deploy(self, name_to_filter=None):
        """ Get the deployment and also print them, if the name_to_filter
        param is given, just
        show only the ones that belogns to the filter. """

        deploys = self.api.list_deployment_for_all_namespaces(
            timeout_seconds=60)
        deploy_obj = {}

        for deploy in deploys.items:
            deploy_name = deploy.metadata.name
            if name_to_filter is not None and name_to_filter not in deploy_name:
                continue
            deploy_obj.update({
                deploy_name: {
                    "namespace": deploy.metadata.namespace,
                    "replicas": str(deploy.spec.replicas),
                }
            })
        table = Table(
            Column("Cluster", style="green"),
            Column("Namespace", style="green"),
            Column("Replicas", style=""),
            Column("Name", style=""),
        )

        for index, deploy in enumerate(deploy_obj):
            data = deploy_obj.get(deploy)
            table.add_row(self.context, data["namespace"], data["replicas"],
                          f"{deploy} ({index})")
        self.print(table)
