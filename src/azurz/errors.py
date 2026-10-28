"""Erreurs métier du pont Azurz."""


class AzurzError(Exception):
    """Erreur prévisible, exposée au chat et à l'interface."""

    def __init__(self, code: str, message: str) -> None:
        super().__init__(message)
        self.code = code
        self.message = message
