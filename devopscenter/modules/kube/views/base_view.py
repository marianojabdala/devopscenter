# -*- coding: utf-8 -*-
__author__ = "Mariano Jose Abdala"
__version__ = "0.1.0"

from devopscenter.modules.kube.kube_base import KubeBase


class ViewBase(KubeBase):
    """
    Base class that is used as base for the views.
    """

    def __init__(self) -> None:
        super().__init__()
