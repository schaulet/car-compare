"""Configuration lue dans l'environnement."""

from __future__ import annotations

import os
from dataclasses import dataclass
from pathlib import Path

from azurz.domain.models import Target


@dataclass(frozen=True)
class Settings:
    host: str
    port: int
    data_dir: Path
    ignition_root: Path
    azdo_url: str
    azdo_org: str
    azdo_project: str
    azdo_pat: str
    mcp_token: str
    worker: bool
    restart_gateways: bool
    targets: list[Target]


def _flag(name: str, default: bool) -> bool:
    raw = os.environ.get(name)
    if raw is None:
        return default
    return raw.strip().lower() in {"1", "true", "yes", "on"}


def _targets() -> list[Target]:
    raw = os.environ.get("AZURZ_TARGETS", "").strip()
    if not raw:
        from azurz.bootstrap import default_targets

        return default_targets()
    targets = []
    for chunk in raw.split(";"):
        parts = [part.strip() for part in chunk.split("|")]
        if len(parts) < 4:
            continue
        container = parts[4] if len(parts) > 4 else ""
        targets.append(
            Target(
                id=parts[0],
                name=parts[1],
                kind="ignition",
                internal_url=parts[2],
                public_url=parts[3],
                container=container,
            )
        )
    return targets


def load_settings() -> Settings:
    return Settings(
        host=os.environ.get("AZURZ_HOST", "0.0.0.0"),
        port=int(os.environ.get("AZURZ_PORT", "8080")),
        data_dir=Path(os.environ.get("AZURZ_DATA", ".azurz-data")),
        ignition_root=Path(os.environ.get("AZURZ_IGNITION_ROOT", ".azurz-data/ignition")),
        azdo_url=os.environ.get("AZURZ_AZDO_URL", "").strip(),
        azdo_org=os.environ.get("AZURZ_AZDO_ORG", "azurz"),
        azdo_project=os.environ.get("AZURZ_AZDO_PROJECT", "AzurzDemo"),
        azdo_pat=os.environ.get("AZURZ_AZDO_PAT", "azurz-lab-pat"),
        mcp_token=os.environ.get("AZURZ_MCP_TOKEN", "dust-lab-token"),
        worker=_flag("AZURZ_WORKER", True),
        restart_gateways=_flag("AZURZ_RESTART_GATEWAYS", False),
        targets=_targets(),
    )
