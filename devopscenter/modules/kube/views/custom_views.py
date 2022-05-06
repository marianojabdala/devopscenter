# -*- coding: utf-8 -*-
__author__ = "Mariano Jose Abdala"
__version__ = "0.1.0"

import shlex

from prompt_toolkit.patch_stdout import patch_stdout
from prompt_toolkit.completion import WordCompleter
from devopscenter.modules.kube.kube_base import KubeBase
from devopscenter.modules.kube.views.view_pvc import PvcView
from devopscenter.modules.kube.views.view_deployment import DeployView
from devopscenter.modules.kube.views.view_pod_resources import PodResourcesView
from devopscenter.modules.kube.views.view_resources_usage import ResourceUsageView
from devopscenter.modules.kube.views.view_statefulsets import StatefulsetView
from devopscenter.modules.kube.views.view_panel import PanelView
from devopscenter.modules.kube.views.view_hpa import HpaView
from devopscenter.modules.kube.views.view_ingress import IngressView


class CustomViews(KubeBase):
    def __init__(self, context_name) -> None:
        super().__init__()
        self.context = context_name
        self.core_v1 = self.cores_v1.get(context_name)
        self.app_v1 = self.apps_v1.get(context_name)
        self.custom_api = self.custom_apis.get(context_name)
        self.autoscaling = self.autoscalings.get(context_name)
        self.extensionsV1Beta1api = self.extensionsV1Beta1apis.get(
            context_name)

    def get_toolbar(self) -> str:
        return "Commands: pvc stateful deploy resources usage hpa ingress"

    def start(self) -> None:
        ns_completer = WordCompleter(
            ["pvc", "stateful", "deploy", "resources", "usage", "hpa", "ingress"], WORD=True
        )
        self.session.completer = ns_completer
        while True:
            try:

                with patch_stdout(raw=True):
                    text = self.session.prompt(
                        [
                            ("fg:green bold", "(" + self.context + "):"),
                            ("fg:ansimagenta bold", "views"),
                            ("", ":>$"),
                        ],
                        bottom_toolbar=self.get_toolbar(),
                    )
                    command = text.strip()
                    if command == "exit":
                        break
                    elif "pvc" in command:
                        args = shlex.split(command)
                        pvc = PvcView(self.core_v1, self.context)
                        pvc.execute(args)
                    elif "stateful" in command:
                        args = shlex.split(command)
                        pvc = StatefulsetView(self.app_v1, self.context)
                        pvc.execute(args)
                    elif "deploy" in command:
                        args = shlex.split(command)
                        pvc = DeployView(self.app_v1, self.context)
                        pvc.execute(args)
                    elif "hpa" in command:
                        args = shlex.split(command)
                        pvc = HpaView(self.autoscaling, self.context)
                        pvc.execute(args)
                    elif "resources" in command:
                        args = shlex.split(command)
                        pvc = PodResourcesView(self.core_v1, self.context)
                        pvc.execute(args)
                    elif "usage" in command:
                        args = shlex.split(command)
                        pvc = ResourceUsageView(self.custom_api)
                        pvc.execute(args)
                    elif "panel" in command:
                        args = shlex.split(command)
                        pvc = PanelView(self.core_v1, self.context)
                        pvc.execute(args)
                    elif "ingress" in command:
                        args = shlex.split(command)
                        ingress = IngressView(self.custom_api)
                        ingress.execute(args)

            except KeyboardInterrupt:
                continue  # Control-C pressed. Try again.
            except EOFError:
                break  # Control-D pressed.