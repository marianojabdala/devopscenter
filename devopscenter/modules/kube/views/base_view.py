""" Module for the common parts of views. """
__author__ = "Mariano Jose Abdala"
__version__ = "0.1.0"

from devopscenter.modules.kube.kube_base import KubeBase


class ViewBase(KubeBase):
    """
    Base class that is used as base for the views.
    """

    def __init__(self, api=None, context=None):
        """ Constructor. """
        super().__init__()
        self.api = api
        self.context = context

    def execute(self, args: None):  # pylint: disable=unused-argument
        """ Method that is executed when the given view doesn't exists. """
        self.print("View not found!!!")
