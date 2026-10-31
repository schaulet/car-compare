"""Artefacts sur disque et dépôt de projets dans les instances Ignition."""

from __future__ import annotations

import json
import os
import shutil
import subprocess
import time
import uuid
from datetime import UTC, datetime
from pathlib import Path

import httpx

from azurz.domain.models import Artifact, Deployment, Target
from azurz.errors import AzurzError


class FileArtifacts:
    def __init__(self, root: Path) -> None:
        self.root = root
        self.root.mkdir(parents=True, exist_ok=True)
        self._items: dict[str, Artifact] = {}

    def publish(
        self,
        *,
        name: str,
        version: str,
        demand_id: int,
        source_dir: Path,
        project_name: str,
    ) -> Artifact:
        artifact_id = uuid.uuid4().hex[:12]
        dest = self.root / artifact_id
        if dest.exists():
            shutil.rmtree(dest)
        shutil.copytree(source_dir, dest)
        artifact = Artifact(
            id=artifact_id,
            name=name,
            version=version,
            demand_id=demand_id,
            path=str(dest),
            project_name=project_name,
            created_at=datetime.now(UTC).strftime("%Y-%m-%dT%H:%M:%SZ"),
        )
        self._items[artifact.id] = artifact
        (dest / "artifact.json").write_text(
            json.dumps(
                {
                    "id": artifact.id,
                    "name": artifact.name,
                    "version": artifact.version,
                    "demand_id": artifact.demand_id,
                    "project_name": artifact.project_name,
                    "created_at": artifact.created_at,
                },
                ensure_ascii=False,
                indent=2,
            ),
            encoding="utf-8",
        )
        return artifact

    def get(self, artifact_id: str) -> Artifact:
        try:
            return self._items[artifact_id]
        except KeyError as exc:
            raise AzurzError("missing", f"Artefact {artifact_id} introuvable") from exc

    def find(self, name: str, version: str) -> Artifact | None:
        for artifact in self._items.values():
            if artifact.name == name and artifact.version == version:
                return artifact
        return None

    def list(self) -> list[Artifact]:
        return sorted(self._items.values(), key=lambda item: item.created_at)


class DeploymentLog:
    def __init__(self) -> None:
        self._items: list[Deployment] = []

    def add(
        self,
        *,
        artifact_id: str,
        version: str,
        target_ids: list[str],
        status: str,
        demand_id: int,
        details: list[dict],
    ) -> Deployment:
        deployment = Deployment(
            id=uuid.uuid4().hex[:12],
            artifact_id=artifact_id,
            version=version,
            target_ids=target_ids,
            status=status,
            demand_id=demand_id,
            details=details,
        )
        self._items.append(deployment)
        return deployment

    def list(self) -> list[Deployment]:
        return list(self._items)


class FilesystemGateways:
    """Écrit le projet Ignition dans le volume de l'instance et sonde la passerelle."""

    def __init__(self, root: Path, *, restart: bool = False) -> None:
        self.root = root
        self.restart = restart
        self.root.mkdir(parents=True, exist_ok=True)

    def deploy(self, target: Target, artifact: Artifact) -> dict[str, str]:
        source = Path(artifact.path) / "gateway_project"
        if not source.is_dir():
            return {
                "target_id": target.id,
                "status": "failed",
                "detail": "le projet gateway est absent de l'artefact",
            }
        destination = self.root / target.id / "projects" / artifact.project_name
        if destination.exists():
            shutil.rmtree(destination)
        shutil.copytree(source, destination)
        _relax(destination)
        receipt_dir = self.root / target.id / "azurz-artifacts" / artifact.id
        receipt_dir.mkdir(parents=True, exist_ok=True)
        modl_files = list(Path(artifact.path).glob("*.modl"))
        for modl in modl_files:
            shutil.copy2(modl, receipt_dir / modl.name)
        receipt = {
            "artifact_id": artifact.id,
            "name": artifact.name,
            "version": artifact.version,
            "project": artifact.project_name,
        }
        (receipt_dir / "receipt.json").write_text(
            json.dumps(receipt, ensure_ascii=False, indent=2),
            encoding="utf-8",
        )
        _relax(receipt_dir)
        health = _probe(target.internal_url) if target.internal_url else "not_checked"
        restarted = ""
        if self.restart and target.container:
            restarted, health = _restart_and_wait(target)
        status = "deployed" if health in {"running", "not_checked"} else "staged"
        detail = f"projet {artifact.project_name} écrit ({health}){restarted}"
        return {"target_id": target.id, "status": status, "detail": detail}


def _relax(path: Path) -> None:
    for current in [path, *path.rglob("*")]:
        try:
            os.chmod(current, 0o777 if current.is_dir() else 0o666)
        except OSError:
            continue


def _restart_and_wait(target: Target) -> tuple[str, str]:
    completed = subprocess.run(
        ["docker", "restart", target.container],
        capture_output=True,
        text=True,
        timeout=180,
        check=False,
    )
    if completed.returncode != 0:
        error = (completed.stderr or completed.stdout or "docker restart a échoué").strip()
        return f" ; redémarrage impossible : {error}", "offline"
    deadline = time.monotonic() + 90
    health = "starting"
    while time.monotonic() < deadline:
        health = _probe(target.internal_url) if target.internal_url else "not_checked"
        if health == "running":
            return " ; passerelle redémarrée", health
        time.sleep(3)
    return " ; passerelle redémarrée, état non confirmé", health


def _probe(url: str) -> str:
    try:
        response = httpx.get(f"{url.rstrip('/')}/StatusPing", timeout=3)
    except httpx.HTTPError:
        return "offline"
    if response.status_code != 200:
        return "offline"
    if "RUNNING" in response.text.upper():
        return "running"
    return "starting"
