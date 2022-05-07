""" Command base class. """

__author__ = "Mariano Jose Abdala"
__version__ = "0.1.0"

from devopscenter.base import Base
from devopscenter.modules.kube.models.pod import PodInfo


class BaseCmd(Base):
    """
        Class that is used as base for the commands and contains common logic.
    """

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
