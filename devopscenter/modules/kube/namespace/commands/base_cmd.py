# -*- coding: utf-8 -*-
__author__ = "Mariano Jose Abdala"
__version__ = "0.1.0"

from devopscenter.base import Base
from devopscenter.modules.kube.pod import PodInfo


class BaseCmd(Base):
    """
    Base class that is used as base for the commands.
    """

    def __init__(self) -> None:
        super().__init__()

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
