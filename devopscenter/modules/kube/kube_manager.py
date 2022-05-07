""" Module that ask for the context to be used. """

__author__ = "Mariano Jose Abdala"
__version__ = "0.1.0"

from rich.table import Table, Column

from prompt_toolkit.patch_stdout import patch_stdout
from prompt_toolkit.completion import WordCompleter

from devopscenter.modules.kube.kube_base import KubeBase
from devopscenter.modules.kube.context import Context


class KubeManager(KubeBase):
    """ Class that ask for the context to be used"""

    def show_help(self) -> None:
        """ Show module help. """
        self.log("Select the context to use")
        table = Table(Column("Contexts", style="green"))
        for context_name in self.contexts:
            table.add_row(context_name)
        self.print(table)

    def start(self) -> None:
        """ Start in the kube module. """
        kube_completer = WordCompleter(self.contexts, WORD=True)
        table = Table(Column("Contexts", style="green"))
        for context_name in self.contexts:
            table.add_row(context_name)
        self.print(table)

        while True:
            try:
                with patch_stdout(raw=True):
                    self.session.completer = kube_completer
                    text = self.session.prompt([
                        ("fg:ansimagenta bold", "kube"),
                        ("", ":>$"),
                    ])
                    command = text.strip()
                    if command == "exit":
                        break
                    if command in ('help', 'h'):
                        self.show_help()
                    else:
                        if command != "" and command in self.contexts:
                            ctx = Context(command)
                            ctx.start()

            except KeyboardInterrupt:
                continue  # Control-C pressed. Try again.
            except EOFError:
                break  # Control-D pressed.
