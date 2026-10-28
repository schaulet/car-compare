"""API Azure DevOps préconfigurée pour le lab (projets, work items, WIQL, commentaires)."""

from __future__ import annotations

import base64
import os
import re
from typing import Any

from fastapi import FastAPI, HTTPException, Request
from fastapi.responses import JSONResponse

PAT = os.environ.get("AZURZ_AZDO_PAT", "azurz-lab-pat")
ORG = os.environ.get("AZURZ_AZDO_ORG", "azurz")
PROJECT = os.environ.get("AZURZ_AZDO_PROJECT", "AzurzDemo")


class _Store:
    def __init__(self) -> None:
        self.projects = [
            {
                "id": "azurz-demo",
                "name": PROJECT,
                "description": "Projet préconfiguré du lab Azurz",
                "state": "wellFormed",
            }
        ]
        self.items: dict[int, dict[str, Any]] = {}
        self.comments: dict[int, list[dict[str, str]]] = {}
        self._next = 1

    def create(self, project: str, work_item_type: str, fields: dict[str, Any]) -> dict[str, Any]:
        item_id = self._next
        self._next += 1
        stored = {
            "id": item_id,
            "rev": 1,
            "fields": {
                "System.TeamProject": project,
                "System.WorkItemType": work_item_type,
                **fields,
            },
            "url": f"/{ORG}/{project}/_apis/wit/workitems/{item_id}",
        }
        self.items[item_id] = stored
        self.comments[item_id] = []
        return stored

    def update(self, item_id: int, fields: dict[str, Any]) -> dict[str, Any]:
        item = self.items[item_id]
        item["rev"] += 1
        item["fields"].update(fields)
        return item


def create_lab_app(store: _Store | None = None, pat: str = PAT) -> FastAPI:
    app = FastAPI(title="Azurz Azure DevOps Lab", version="1.0.0")
    data = store or _Store()
    expected = "Basic " + base64.b64encode(f":{pat}".encode()).decode("ascii")

    @app.middleware("http")
    async def _auth(request: Request, call_next):
        if request.url.path == "/health":
            return await call_next(request)
        if request.headers.get("authorization") != expected:
            return JSONResponse({"detail": "PAT refusé"}, status_code=401)
        return await call_next(request)

    @app.get("/health")
    def health() -> dict[str, str]:
        return {"status": "ok", "organization": ORG, "project": PROJECT}

    @app.get("/{org}/_apis/projects")
    def projects(org: str) -> dict[str, Any]:
        _org(org)
        return {"count": len(data.projects), "value": data.projects}

    @app.get("/{org}/{project}/_apis/wit/workitems/{item_id}")
    def get_item(org: str, project: str, item_id: int) -> dict[str, Any]:
        _scope(org, project)
        return _get(data, item_id)

    @app.patch("/{org}/{project}/_apis/wit/workitems/{item_id}")
    async def patch_item(org: str, project: str, item_id: int, request: Request) -> dict[str, Any]:
        _scope(org, project)
        item = _get(data, item_id)
        ops = await request.json()
        fields = _apply(item["fields"], ops)
        return data.update(item_id, fields)

    @app.post("/{org}/{project}/_apis/wit/workitems/{wit_type}")
    async def create_item(
        org: str,
        project: str,
        wit_type: str,
        request: Request,
    ) -> dict[str, Any]:
        _scope(org, project)
        if wit_type.isdigit():
            raise HTTPException(status_code=404, detail="utilisez PATCH pour un work item existant")
        name = wit_type[1:] if wit_type.startswith("$") else wit_type
        name = name.replace("%20", " ")
        ops = await request.json()
        fields = _apply({}, ops)
        return data.create(project, name, fields)

    @app.post("/{org}/{project}/_apis/wit/wiql")
    async def wiql(org: str, project: str, request: Request) -> dict[str, Any]:
        _scope(org, project)
        body = await request.json()
        matched = _query(data, project, str(body.get("query") or ""))
        return {"workItems": [{"id": item["id"], "url": item["url"]} for item in matched]}

    @app.post("/{org}/{project}/_apis/wit/workItems/{item_id}/comments")
    async def add_comment(org: str, project: str, item_id: int, request: Request) -> dict[str, Any]:
        _scope(org, project)
        _get(data, item_id)
        body = await request.json()
        text = str(body.get("text") or "")
        note = {"text": text}
        data.comments[item_id].append(note)
        return note

    @app.get("/{org}/{project}/_apis/wit/workItems/{item_id}/comments")
    def list_comments(org: str, project: str, item_id: int) -> dict[str, Any]:
        _scope(org, project)
        _get(data, item_id)
        notes = data.comments.get(item_id, [])
        return {"count": len(notes), "comments": notes}

    return app


def _org(org: str) -> None:
    if org != ORG:
        raise HTTPException(status_code=404, detail="organisation inconnue")


def _scope(org: str, project: str) -> None:
    _org(org)
    if project != PROJECT:
        raise HTTPException(status_code=404, detail="projet inconnu")


def _get(data: _Store, item_id: int) -> dict[str, Any]:
    item = data.items.get(item_id)
    if item is None:
        raise HTTPException(status_code=404, detail="work item introuvable")
    return item


def _apply(fields: dict[str, Any], ops: list[dict[str, Any]]) -> dict[str, Any]:
    updated = dict(fields)
    for op in ops:
        path = str(op.get("path") or "")
        if not path.startswith("/fields/"):
            continue
        key = path.removeprefix("/fields/")
        operation = op.get("op")
        if operation in {"add", "replace"}:
            updated[key] = op.get("value")
        elif operation == "remove":
            updated.pop(key, None)
    return updated


def _query(data: _Store, project: str, query: str) -> list[dict[str, Any]]:
    tag = None
    found = re.search(r"\[System\.Tags\]\s+CONTAINS\s+'([^']+)'", query, re.IGNORECASE)
    if found:
        tag = found.group(1)
    state = None
    found_state = re.search(r"\[System\.State\]\s*=\s*'([^']+)'", query, re.IGNORECASE)
    if found_state:
        state = found_state.group(1)
    rows = []
    for item in data.items.values():
        fields = item["fields"]
        if fields.get("System.TeamProject") != project:
            continue
        if tag and tag not in str(fields.get("System.Tags") or ""):
            continue
        if state and fields.get("System.State") != state:
            continue
        rows.append(item)
    return rows


def main() -> None:
    import uvicorn

    port = int(os.environ.get("AZURZ_AZDO_PORT", "8080"))
    uvicorn.run(create_lab_app(), host="0.0.0.0", port=port)
