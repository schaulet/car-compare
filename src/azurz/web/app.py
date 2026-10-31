"""API HTTP de l'interface et montage du serveur MCP."""

from __future__ import annotations

import asyncio
import contextlib
import io
import zipfile
from collections.abc import AsyncIterator
from contextlib import asynccontextmanager
from dataclasses import asdict
from pathlib import Path

from fastapi import FastAPI
from fastapi.responses import FileResponse, JSONResponse, Response
from fastapi.staticfiles import StaticFiles
from mcp.server.mcpserver import MCPServer
from pydantic import BaseModel, Field
from starlette.applications import Starlette
from starlette.routing import Mount
from starlette.types import ASGIApp, Receive, Scope, Send

from azurz.contrib import build_catalog
from azurz.errors import AzurzError
from azurz.platform import OEE_MESSAGE, Platform

STATIC = Path(__file__).resolve().parent / "static"


class DemandIn(BaseModel):
    message: str = Field(min_length=1)
    requester: str = "operator"


class MessageIn(BaseModel):
    message: str = Field(min_length=1)


class SettingsIn(BaseModel):
    auto_process: bool


class DeployIn(BaseModel):
    artifact_id: str
    version: str
    target_ids: list[str]


class _AcceptMcpWithoutSlash:
    """Starlette ne monte `/mcp` que sous `/mcp/`. Dust appelle souvent l'URL sans barre."""

    def __init__(self, app: ASGIApp) -> None:
        self.app = app

    async def __call__(self, scope: Scope, receive: Receive, send: Send) -> None:
        if scope.get("path") == "/mcp":
            scope = dict(scope)
            scope["path"] = "/mcp/"
            scope["raw_path"] = b"/mcp/"
        await self.app(scope, receive, send)


class BearerGate:
    def __init__(self, app: ASGIApp, token: str) -> None:
        self.app = app
        self.token = token

    async def __call__(self, scope: Scope, receive: Receive, send: Send) -> None:
        if scope["type"] != "http":
            await self.app(scope, receive, send)
            return
        headers = {
            key.decode("latin1").lower(): value.decode("latin1") for key, value in scope["headers"]
        }
        if headers.get("authorization") != f"Bearer {self.token}":
            response = JSONResponse({"detail": "Jeton MCP refusé"}, status_code=401)
            await response(scope, receive, send)
            return
        await self.app(scope, receive, send)


def create_http_app(platform: Platform, *, mcp_token: str, worker: bool = False) -> Starlette:
    mcp = MCPServer(
        "azurz-devops",
        title="Azurz DevOps",
        description=(
            "Analyse les demandes Dust, crée les tickets Azure DevOps et pilote les agents."
        ),
        instructions=(
            "Utilise creer_demande_azure_devops pour une nouvelle demande, "
            "enrichir_demande pour itérer sur le même ticket, "
            "puis consulter_demande pour la proposition, le rapport de tests et l'artefact."
        ),
    )
    for tool in build_catalog(platform).all():
        mcp.add_tool(
            tool.handler,
            name=tool.name,
            description=tool.description,
            structured_output=False,
        )
    mcp_app = mcp.streamable_http_app(
        streamable_http_path="/",
        stateless_http=True,
        json_response=True,
        host="0.0.0.0",
    )
    api = _api(platform)

    @asynccontextmanager
    async def lifespan(_: Starlette) -> AsyncIterator[None]:
        async with mcp.session_manager.run():
            stop = asyncio.Event()
            task = asyncio.create_task(_worker(platform, stop)) if worker else None
            try:
                yield
            finally:
                stop.set()
                if task is not None:
                    task.cancel()
                    with contextlib.suppress(asyncio.CancelledError):
                        await task

    application = Starlette(
        lifespan=lifespan,
        routes=[
            Mount("/mcp", app=BearerGate(mcp_app, mcp_token)),
            Mount("/", app=api),
        ],
    )
    return _AcceptMcpWithoutSlash(application)


def _api(platform: Platform) -> FastAPI:
    api = FastAPI(title="Azurz", version="0.2.0")

    @api.exception_handler(AzurzError)
    async def _azurz_error(_: object, exc: AzurzError) -> JSONResponse:
        if exc.code == "missing":
            status = 404
        elif exc.code in {"not_ready", "busy", "target"}:
            status = 409
        else:
            status = 400
        return JSONResponse({"detail": exc.message, "code": exc.code}, status_code=status)

    @api.get("/")
    def index() -> FileResponse:
        return FileResponse(STATIC / "index.html")

    @api.get("/api/health")
    def health() -> dict[str, str]:
        return {"status": "ok"}

    @api.get("/api/examples")
    def examples() -> dict[str, str]:
        return {"message": OEE_MESSAGE}

    @api.get("/api/settings")
    def read_settings() -> dict[str, bool]:
        return {"auto_process": platform.auto_process}

    @api.put("/api/settings")
    def write_settings(body: SettingsIn) -> dict[str, object]:
        platform.set_auto(body.auto_process)
        processed = platform.tick() if body.auto_process else []
        return {"auto_process": platform.auto_process, "processed": [item.id for item in processed]}

    @api.get("/api/demands")
    def list_demands() -> list[dict[str, object]]:
        return [asdict(item) for item in platform.workitems.list()]

    @api.post("/api/demands")
    def create_demand(body: DemandIn) -> dict[str, object]:
        return asdict(platform.submit_message(body.message, requester=body.requester))

    @api.post("/api/demands/{demand_id}/messages")
    def add_message(demand_id: int, body: MessageIn) -> dict[str, object]:
        return asdict(platform.iterate(demand_id, body.message))

    @api.post("/api/demands/{demand_id}/process")
    def process(demand_id: int) -> dict[str, object]:
        return asdict(platform.trigger(demand_id))

    @api.get("/api/artifacts")
    def list_artifacts() -> list[dict[str, object]]:
        return [asdict(item) for item in platform.artifacts.list()]

    @api.get("/api/artifacts/{artifact_id}/download")
    def download(artifact_id: str) -> Response:
        artifact = platform.artifacts.get(artifact_id)
        buffer = io.BytesIO()
        with zipfile.ZipFile(buffer, "w", compression=zipfile.ZIP_DEFLATED) as archive:
            root = Path(artifact.path)
            for file in root.rglob("*"):
                if file.is_file():
                    archive.write(file, file.relative_to(root).as_posix())
        filename = f"{artifact.project_name}-{artifact.version}.zip"
        return Response(
            buffer.getvalue(),
            media_type="application/zip",
            headers={"Content-Disposition": f'attachment; filename="{filename}"'},
        )

    @api.get("/api/artifacts/{artifact_id}/rapport")
    def report(artifact_id: str) -> FileResponse:
        artifact = platform.artifacts.get(artifact_id)
        path = Path(artifact.path) / "rapport.html"
        if not path.is_file():
            raise AzurzError("missing", "Rapport absent")
        return FileResponse(path, media_type="text/html")

    @api.get("/api/targets")
    def list_targets() -> list[dict[str, str]]:
        return [asdict(item) for item in platform.targets.values()]

    @api.get("/api/deployments")
    def list_deployments() -> list[dict[str, object]]:
        return [asdict(item) for item in platform.deployments.list()]

    @api.post("/api/deployments")
    def deploy(body: DeployIn) -> dict[str, object]:
        return asdict(platform.deploy(body.artifact_id, body.version, body.target_ids))

    @api.get("/api/agents")
    def list_agents() -> dict[str, object]:
        return {
            "agents": [
                {
                    "id": agent.descriptor.id,
                    "name": agent.descriptor.name,
                    "capabilities": list(agent.descriptor.capabilities),
                    "prerogatives": sorted(agent.descriptor.prerogatives),
                }
                for agent in platform.agents.all()
            ],
            "policies": [
                {
                    "capability": policy.capability,
                    "required": sorted(policy.required_prerogatives),
                }
                for policy in platform.policies.all()
            ],
        }

    api.mount("/static", StaticFiles(directory=STATIC), name="static")
    return api


async def _worker(platform: Platform, stop: asyncio.Event) -> None:
    while not stop.is_set():
        try:
            await asyncio.to_thread(platform.tick)
        except Exception:
            pass
        try:
            await asyncio.wait_for(stop.wait(), timeout=2)
        except TimeoutError:
            continue
