# -*- coding: utf-8 -*-
__author__ = "Mariano Jose Abdala"
__version__ = "0.1.0"

from rich.table import Table, Column

from devopscenter.modules.kube.views.base_view import ViewBase
import json


class DeployView(ViewBase):
    def __init__(self, app, context):
        super().__init__()
        self.app_v1 = app
        self.context = context

    def execute(self, args):
        if len(args) == 2:
            self.__show_deploy(args[1])
        else:
            self.__show_deploy()

    def __show_deploy(self, filter=None):
        deploys = self.app_v1.list_deployment_for_all_namespaces(timeout_seconds=60)
        deploy_obj = {}

        for deploy in deploys.items:
            deploy_name = deploy.metadata.name
            if filter is not None and filter not in deploy_name:
                continue
            deploy_obj.update(
                {
                    deploy_name: {
                        "namespace": deploy.metadata.namespace,
                        "replicas": str(deploy.spec.replicas),
                    }
                }
            )
        table = Table(
            Column("Cluster", style="green"),
            Column("Namespace", style="green"),
            Column("Replicas", style=""),
            Column("Name", style=""),
        )

        for index, deploy in enumerate(deploy_obj):
            data = deploy_obj.get(deploy)
            table.add_row(self.context, data["namespace"], data["replicas"], f"{deploy} ({index})")
        self.print(table)
        with open("deploy.json", "w", encoding="utf-8") as dep:
            dep.write(json.dumps(deploy_obj))
