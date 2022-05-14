""" Command base class. """

__author__ = "Mariano Jose Abdala"
__version__ = "0.1.0"

from typing import List
from devopscenter.base import Base
from devopscenter.modules.kube.models.pod import PodInfo
from devopscenter.modules.kube.cluster_utils import get_pods


class BaseCmd(Base):
    """
        Class that is used as base for the commands and contains common logic.
    """

    def __init__(self, core=None, namespace=None):
        """Initialize a new log command."""
        super().__init__()
        self.core = core
        self.namespace = namespace

    def _get_pods(self, api, namespace=None) -> List[PodInfo]:  # pylint: disable=no-self-use
        """
        Get the list of pod from the entire cluster or a namespace.
        """
        return get_pods(api, namespace)

    def execute(self, args: None) -> None:  # pylint: disable=unused-argument
        """ Default execute when the command is not found. """
        self.print("Command not found!!!")

    def get_pod(self, pod_index, pods) -> PodInfo:
        """
        Returns the specific pod using the given pod_index
        :param pod_index
        :param pods
        :return PodInfo object
        """
        if pod_index > len(pods):
            self.print("[red]The number is not in the list of pods[/red]")
            return None
        return [pod for index, pod in enumerate(pods) if index == pod_index][0]
