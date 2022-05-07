""" Base class that handle almost all the initialization for the other ones. """
import os
import os.path
from pathlib import Path
from rich.console import Console
from prompt_toolkit import PromptSession


class Base:
    """
    Base class that is used everywhere.
    """

    def __init__(self) -> None:
        """ Constructor method for the class to initialize all.  """
        self.console = Console(emoji=False)
        self.session = PromptSession()
        self.base_path = os.path.join(str(Path.home()), ".local", "share",
                                      "devopscenter")
        os.makedirs(self.base_path, exist_ok=True)

    def log(self, *args, **kargs) -> None:
        """Wrapper used to call the console.log.

        :param args
        :param kargs
        """
        self.console.log(*args, **kargs)

    def print(self, *args, **kargs) -> None:
        """Wrapper used to call the console.print.

        :param args
        :param kargs
        """
        try:
            self.console.print(*args, **kargs)
        except Exception:  # pylint: disable=W0703
            print(*args, **kargs)
