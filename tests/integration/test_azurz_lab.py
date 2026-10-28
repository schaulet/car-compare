"""Contrat du lab Azure DevOps et de l'API du pont."""

import base64

import httpx
import pytest
from starlette.testclient import TestClient

from azurz.adapters.azdo import AzdoClient
from azurz.bootstrap import create_memory
from azurz.labazdo.app import create_lab_app
from azurz.web.app import create_http_app


def _basic(pat: str = "azurz-lab-pat") -> dict[str, str]:
    token = base64.b64encode(f":{pat}".encode()).decode("ascii")
    return {"Authorization": f"Basic {token}"}


def test_lab_rejects_a_missing_pat():
    app = create_lab_app()
    with TestClient(app) as client:
        response = client.get("/azurz/_apis/projects")
    assert response.status_code == 401


def test_lab_roundtrip_matches_the_client():
    app = create_lab_app()
    with TestClient(app) as client:
        projects = client.get("/azurz/_apis/projects", headers=_basic())
        assert projects.status_code == 200
        assert projects.json()["value"][0]["name"] == "AzurzDemo"

        created = client.post(
            "/azurz/AzurzDemo/_apis/wit/workitems/$User Story",
            params={"api-version": "7.1"},
            headers={**_basic(), "Content-Type": "application/json-patch+json"},
            json=[{"op": "add", "path": "/fields/System.Title", "value": "OEE"}],
        )
        assert created.status_code == 200
        item_id = created.json()["id"]
        assert created.json()["fields"]["System.Title"] == "OEE"

        client.post(
            f"/azurz/AzurzDemo/_apis/wit/workItems/{item_id}/comments",
            headers={**_basic(), "Content-Type": "application/json"},
            json={"text": "enrichissement"},
        )
        notes = client.get(
            f"/azurz/AzurzDemo/_apis/wit/workItems/{item_id}/comments",
            headers=_basic(),
        )
        assert notes.json()["comments"][0]["text"] == "enrichissement"

        listed = client.post(
            "/azurz/AzurzDemo/_apis/wit/wiql",
            headers={**_basic(), "Content-Type": "application/json"},
            json={
                "query": "SELECT [System.Id] FROM WorkItems WHERE [System.Tags] CONTAINS 'azurz'"
            },
        )
        assert listed.status_code == 200


def test_azdo_client_against_the_lab(monkeypatch: pytest.MonkeyPatch):
    app = create_lab_app()
    session = TestClient(app)
    session.__enter__()

    def _forward(method: str):
        def _call(url: str, **kwargs):
            path = url.split("://", 1)[-1]
            path = path[path.find("/") :]
            kwargs.pop("timeout", None)
            return session.request(method, path, **kwargs)

        return _call

    monkeypatch.setattr(httpx, "post", _forward("POST"))
    monkeypatch.setattr(httpx, "patch", _forward("PATCH"))
    monkeypatch.setattr(httpx, "get", _forward("GET"))
    try:
        client = AzdoClient("http://lab", "azurz", "AzurzDemo", "azurz-lab-pat")
        created = client.create_work_item({"System.Title": "Depuis le client"})
        fetched = client.get_work_item(created["id"])
    finally:
        session.__exit__(None, None, None)
    assert fetched["fields"]["System.Title"] == "Depuis le client"


def test_ui_lists_agents_and_targets():
    platform = create_memory(targets=[])
    with TestClient(create_http_app(platform, mcp_token="dust-lab-token", worker=False)) as client:
        page = client.get("/")
        assert page.status_code == 200
        assert "Traitement automatique" in page.text
        agents = client.get("/api/agents")
        assert agents.status_code == 200
        names = {item["id"] for item in agents.json()["agents"]}
        assert names == {"ignition-module", "deploiement"}
        denied = client.post("/mcp", json={"jsonrpc": "2.0", "id": 1, "method": "initialize"})
        assert denied.status_code == 401
