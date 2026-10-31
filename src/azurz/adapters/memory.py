"""Work items en mémoire, même contrat que le miroir Azure DevOps."""

from __future__ import annotations

from azurz.domain.models import Demand
from azurz.errors import AzurzError


class MemoryWorkItems:
    def __init__(self) -> None:
        self._items: dict[int, Demand] = {}
        self._next = 1

    def create(self, demand: Demand) -> Demand:
        demand.id = self._next
        self._next += 1
        demand.rev = 1
        self._items[demand.id] = demand
        return demand

    def save(self, demand: Demand) -> Demand:
        if demand.id not in self._items:
            raise AzurzError("missing", f"Demande {demand.id} introuvable")
        demand.rev += 1
        self._items[demand.id] = demand
        return demand

    def get(self, demand_id: int) -> Demand:
        try:
            return self._items[demand_id]
        except KeyError as exc:
            raise AzurzError("missing", f"Demande {demand_id} introuvable") from exc

    def list(self) -> list[Demand]:
        return sorted(self._items.values(), key=lambda item: item.id)
