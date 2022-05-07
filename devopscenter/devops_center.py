""" Here we will load all the modules that can be created on the modules folder."""
from prompt_toolkit import PromptSession

from rich.table import Table, Column

from devopscenter.modules.kube.kube_manager import KubeManager
from devopscenter.base import Base


class Manager(Base):
    """
    Class that is in charge of all the interacion with kubernetes.
    This will load all the modules that needs and use the accordenly
    """

    def show_help(self) -> None:
        """
        Shows the commands that can be used
        """
        table = Table(
            Column("Commands", style="green"),
            Column("Description", style=""),
        )
        table.add_row("kube", "Interact with the cluster")
        self.print(table)

    def start(self):
        """
        Start the interaction with the system.
        """
        session = PromptSession()
        self.log("Welcome to [green]Devops Center[/green] !")
        while True:
            try:
                text = session.prompt([
                    ("fg:ansimagenta bold", "devops_center"),
                    ("", ":>$"),
                ])
                command = text.strip()
                if command == "exit":
                    break
                if command in ("help", "h"):
                    self.show_help()
                elif command == "kube":
                    manager = KubeManager()
                    manager.start()

            except KeyboardInterrupt:
                continue  # Control-C pressed. Try again.
            except EOFError:
                self.print("[blue]See You!!!![/blue]")
                break  # Control-D pressed.
