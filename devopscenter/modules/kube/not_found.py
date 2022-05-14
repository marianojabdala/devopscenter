""" This is just a default fallback when something is wrong """
# -*- coding: utf-8 -*-
__author__ = "Mariano Jose Abdala"
__version__ = "0.1.0"

from devopscenter.base import Base


class NotFound(Base):
    """
    Class that is used as default when the command is not implemented or the
    user enter anything else.
    """

    def start(self) -> None:
        """ Method that is the default when the command introduced isn't available. """
        self.print("[red]Command not found!!![/red]")
