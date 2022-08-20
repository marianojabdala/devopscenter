""" Entrypint used when the package is installed"""

from devopscenter.devops_center import DevopsCenter


def main():
    """Entrypoint of the system."""
    manager = DevopsCenter()
    manager.start()


if __name__ == "__main__":
    main()
