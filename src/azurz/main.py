"""Point d'entrée du pont Azurz."""

from __future__ import annotations

import uvicorn

from azurz.bootstrap import create_connected, create_memory
from azurz.config import load_settings
from azurz.web.app import create_http_app


def build_platform():
    settings = load_settings()
    if settings.azdo_url:
        return settings, create_connected(
            azdo_url=settings.azdo_url,
            azdo_org=settings.azdo_org,
            azdo_project=settings.azdo_project,
            azdo_pat=settings.azdo_pat,
            data_dir=settings.data_dir,
            ignition_root=settings.ignition_root,
            targets=settings.targets,
            restart_gateways=settings.restart_gateways,
        )
    return settings, create_memory(root=settings.data_dir, targets=settings.targets)


def main() -> None:
    settings, platform = build_platform()
    app = create_http_app(platform, mcp_token=settings.mcp_token, worker=settings.worker)
    uvicorn.run(app, host=settings.host, port=settings.port)
