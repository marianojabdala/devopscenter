""" Manage the creation, deleting and list of the namespaces. """

__author__ = "Mariano Jose Abdala"
__version__ = "0.1.0"

import shlex
import time

from kubernetes.client.exceptions import ApiException

from prompt_toolkit.completion import WordCompleter
from prompt_toolkit.patch_stdout import patch_stdout
from rich.table import Table, Column

from devopscenter.modules.kube.cluster_utils import get_namespace_names
from devopscenter.modules.kube.kube_base import KubeBase
from devopscenter.modules.kube.namespace.namespaces import Namespaces


class NamespacesManager(KubeBase):
    """
    Class that is in charge of managing the namespace interaction
    """

    def __init__(self, context) -> None:
        """
        Constructor
        """
        super().__init__()
        self.core_v1 = self.cores_v1.get(context)
        self.context = context
        self.pods = []
        self.namespace = None
        self.current_namespaces = []

    def get_toolbar(self):
        """ Get cutoms toolbar too be shown. """
        return "Commands list - create - delete"

    def show_help(self) -> None:
        """ Show the commands that are allowed. """
        table = Table(
            Column("Commands", style="green"),
            Column("Description", style=""),
            Column("Info", style=""),
        )
        table.add_row("list", "Shows all the namespaces")
        table.add_row("create", "Creates the given namespace")
        table.add_row("delete", "Delete the given namespace")
        self.print(table)

    def _delete_ns(self, args) -> None:
        """ Deletes the given namespace, and wait until it is deleted.  """
        namespace_to_delete = args[1]
        if namespace_to_delete in self.current_namespaces:
            try:
                delete_options = self.k8s_client.V1DeleteOptions(
                    grace_period_seconds=0)

                self.core_v1.delete_namespace(namespace_to_delete,
                                              body=delete_options)

                with self.console.status("Terminating namespace"):
                    while True:
                        current_namespaces = get_namespace_names(
                            self.core_v1.list_namespace())
                        if namespace_to_delete in current_namespaces:
                            time.sleep(45)
                        else:
                            break
            except ApiException as api_ex:
                self.log("Somethig went wrong deleting namespace", api_ex)

        else:
            self.print("Namespace doesn't exists!")

    def _create_ns(self, args) -> None:
        """ Creates the namespace that the user enter on the prompt. """
        namespace_to_create = args[1]
        if namespace_to_create not in self.current_namespaces:
            namespace = self.k8s_client.V1Namespace(
                metadata=self.k8s_client.V1ObjectMeta(
                    name=namespace_to_create))
            try:
                self.core_v1.create_namespace(namespace)
            except ApiException as api_ex:
                self.log("Somethig went wrong creating namespace", api_ex)

        else:
            self.print("Namespace already exists!")

    def _show_list(self) -> None:
        """ Show the list of the current namespaces. Like kubectl get ns"""
        table = Table(Column("Namespace Name", style="green"), )
        for namespace in self.current_namespaces:
            table.add_row(namespace)
        self.print(table)

    def start(self) -> None:
        """ Start to interact with namespaces, create, list or delete one. """
        while True:
            try:
                self.current_namespaces = get_namespace_names(
                    self.core_v1.list_namespace())
                self.session.completer = WordCompleter(self.current_namespaces,
                                                       WORD=True)
                with patch_stdout(raw=True):
                    text = self.session.prompt(
                        [
                            ("fg:green bold", "(" + self.context + "):"),
                            ("fg:ansimagenta bold", "namespaces"),
                            ("", ":>$"),
                        ],
                        bottom_toolbar=self.get_toolbar(),
                    ).strip()

                args = shlex.split(text)
                text = args[0]
                if text == "exit":
                    break

                if text == "create":
                    self._create_ns(args)

                if text == "delete":
                    args = shlex.split(text)
                    self._delete_ns(args)

                if text == "list":
                    self._show_list()

                if text in self.current_namespaces:
                    namespaces = Namespaces(self.context, text)
                    namespaces.start()

            except KeyboardInterrupt:
                continue  # Control-C pressed. Try again.
            except EOFError:
                break  # Control-D pressed.
