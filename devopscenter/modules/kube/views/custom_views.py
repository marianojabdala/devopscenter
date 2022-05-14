""" Module for the common parts of views. """

__author__ = "Mariano Jose Abdala"
__version__ = "0.1.0"

from devopscenter.modules.kube.kube_base import KubeBase
from devopscenter.modules.kube.views.base_view import ViewBase
from devopscenter.modules.kube.views.view_pvc import PvcView
from devopscenter.modules.kube.views.view_deployment import DeployView
from devopscenter.modules.kube.views.view_pod_resources import PodResourcesView
from devopscenter.modules.kube.views.view_resources_usage import ResourceUsageView
from devopscenter.modules.kube.views.view_statefulsets import StatefulsetView
from devopscenter.modules.kube.views.view_hpa import HpaView
from devopscenter.modules.kube.views.view_ingress import IngressView


class CustomViews(KubeBase):
    """ This class initialize all the posible views that can be used. """

    def __init__(self, context_name) -> None:
        """ Constructor. """
        super().__init__()
        self.context = context_name
        self.core_v1 = self.cores_v1.get(context_name)
        self.api = self.apps_v1.get(context_name)
        self.custom_api = self.custom_apis.get(context_name)
        self.autoscaling = self.autoscalings.get(context_name)

        self._register_views()

    def _register_views(self):
        """ Add the view that can be used on a map for later execution. """
        self.commands = {
            "pvc": PvcView(self.core_v1, self.context),
            "stateful": StatefulsetView(self.api, self.context),
            "deploy": DeployView(self.api, self.context),
            "hpa": HpaView(self.autoscaling, self.context),
            "resources": PodResourcesView(self.core_v1, self.context),
            "usage": ResourceUsageView(self.custom_api),
            "ingress": IngressView(self.custom_api),
        }

    def _get_label(self):  # pylint: disable=no-self-use
        return self.context

    def _get_cmd_label(self):  # pylint: disable=no-self-use
        return "views"

    def _do_work(self, command=None, args=None) -> None:  # pylint: disable=unused-argument
        self.commands.get(command, ViewBase()).execute(args)
