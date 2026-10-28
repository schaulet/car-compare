"""Client REST Azure DevOps et miroir des demandes."""

from __future__ import annotations

import base64
from typing import Any

import httpx

from azurz.domain.models import Demand
from azurz.errors import AzurzError


class AzdoClient:
    def __init__(self, base_url: str, org: str, project: str, pat: str) -> None:
        self.project = project
        self._base = f"{base_url.rstrip('/')}/{org}/{project}"
        token = base64.b64encode(f":{pat}".encode()).decode("ascii")
        self._headers = {"Authorization": f"Basic {token}"}

    def create_work_item(
        self,
        fields: dict[str, str],
        work_item_type: str = "User Story",
    ) -> dict[str, Any]:
        patch = [
            {"op": "add", "path": f"/fields/{key}", "value": value} for key, value in fields.items()
        ]
        response = httpx.post(
            f"{self._base}/_apis/wit/workitems/${work_item_type}",
            params={"api-version": "7.1"},
            headers={**self._headers, "Content-Type": "application/json-patch+json"},
            json=patch,
            timeout=15,
        )
        if response.status_code >= 400:
            raise AzurzError("azdo", f"Création refusée ({response.status_code})")
        return response.json()

    def update_work_item(
        self,
        item_id: int,
        fields: dict[str, str],
        comment: str | None = None,
    ) -> dict[str, Any]:
        patch = [
            {"op": "replace", "path": f"/fields/{key}", "value": value}
            for key, value in fields.items()
        ]
        response = httpx.patch(
            f"{self._base}/_apis/wit/workitems/{item_id}",
            params={"api-version": "7.1"},
            headers={**self._headers, "Content-Type": "application/json-patch+json"},
            json=patch,
            timeout=15,
        )
        if response.status_code >= 400:
            raise AzurzError("azdo", f"Mise à jour refusée ({response.status_code})")
        if comment:
            note = httpx.post(
                f"{self._base}/_apis/wit/workItems/{item_id}/comments",
                params={"api-version": "7.1-preview.3"},
                headers={**self._headers, "Content-Type": "application/json"},
                json={"text": comment},
                timeout=15,
            )
            if note.status_code >= 400:
                raise AzurzError("azdo", f"Commentaire refusé ({note.status_code})")
        return response.json()

    def get_work_item(self, item_id: int) -> dict[str, Any]:
        response = httpx.get(
            f"{self._base}/_apis/wit/workitems/{item_id}",
            params={"api-version": "7.1"},
            headers=self._headers,
            timeout=15,
        )
        if response.status_code == 404:
            raise AzurzError("missing", f"Work item {item_id} introuvable")
        if response.status_code >= 400:
            raise AzurzError("azdo", f"Lecture refusée ({response.status_code})")
        return response.json()


_AZDO_STATE = {
    "needs_input": "New",
    "ready": "New",
    "in_progress": "Active",
    "done": "Resolved",
    "failed": "Resolved",
    "blocked": "New",
}


class AzureDevOpsWorkItems:
    """Azure DevOps est le registre des tickets. L'overlay garde le détail agent."""

    def __init__(self, client: AzdoClient) -> None:
        self.client = client
        self._items: dict[int, Demand] = {}

    def create(self, demand: Demand) -> Demand:
        created = self.client.create_work_item(_fields(demand))
        demand.id = int(created["id"])
        demand.rev = int(created.get("rev") or 1)
        self._items[demand.id] = demand
        return demand

    def save(self, demand: Demand) -> Demand:
        if demand.id not in self._items and demand.id <= 0:
            raise AzurzError("missing", "Demande sans identifiant Azure DevOps")
        comment = demand.journal[-1] if demand.journal else None
        updated = self.client.update_work_item(demand.id, _fields(demand), comment=comment)
        demand.rev = int(updated.get("rev") or demand.rev + 1)
        self._items[demand.id] = demand
        return demand

    def get(self, demand_id: int) -> Demand:
        if demand_id in self._items:
            return self._items[demand_id]
        self.client.get_work_item(demand_id)
        raise AzurzError("missing", f"Demande {demand_id} absente du pont")

    def list(self) -> list[Demand]:
        return sorted(self._items.values(), key=lambda item: item.id)


def _fields(demand: Demand) -> dict[str, str]:
    tags = list(demand.tags)
    markers = ("azurz:ready", "azurz:needs_input", "azurz:done", "azurz:failed", "azurz:blocked")
    for marker in markers:
        if marker in tags:
            tags.remove(marker)
    tags.append(f"azurz:{demand.state}")
    history = "\n\n".join(f"- {message}" for message in demand.messages)
    description = "\n".join(
        [
            demand.description,
            "",
            "## Échanges",
            history,
            "",
            "## Analyse",
            demand.journal[-1] if demand.journal else "",
        ]
    )
    return {
        "System.Title": demand.title,
        "System.Description": description.strip(),
        "System.State": _AZDO_STATE.get(demand.state, "New"),
        "System.Tags": "; ".join(tags),
        "Microsoft.VSTS.Common.AcceptanceCriteria": demand.acceptance_criteria,
    }
