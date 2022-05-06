from rich.console import Console
from prompt_toolkit import PromptSession
import os
import os.path
from pathlib import Path


class Base:
    """
    Base class that is used everywhere.
    """

    def __init__(self) -> None:
        self.console = Console(emoji=False)
        self.session = PromptSession()
        self.base_path = os.path.join(str(Path.home()), ".local", "share", "devopscenter")
        os.makedirs(self.base_path, exist_ok=True)

    def log(self, *args, **kargs) -> None:
        self.console.log(*args, **kargs)

    def print(self, *args, **kargs) -> None:
        try:
            self.console.print(*args, **kargs)
        except:
            print(*args, **kargs)
