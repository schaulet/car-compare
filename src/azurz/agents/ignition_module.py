"""Agent qui produit un module Ignition, ses tests et l'artefact."""

from __future__ import annotations

import shutil
from pathlib import Path

from azurz.building import package_modl, prepare_build, run_tests, write_reports
from azurz.domain.models import AgentDescriptor, AgentResult, Demand


class IgnitionModuleAgent:
    def __init__(self) -> None:
        self.descriptor = AgentDescriptor(
            id="ignition-module",
            name="Générateur de modules Ignition",
            capabilities=("ignition.module.build",),
            prerogatives=frozenset({"workitem.read", "artifact.publish", "tests.run"}),
            priority=10,
        )

    def execute(self, demand: Demand, context: object) -> AgentResult:
        spec = demand.spec
        if not spec.get("functions") or not spec.get("module_name"):
            return AgentResult(
                state="failed",
                reason="La spécification du module est incomplète.",
            )
        work = Path(context.work_dir) / f"demand-{demand.id}"  # type: ignore[attr-defined]
        if work.exists():
            shutil.rmtree(work)
        prepare_build(spec, work)
        proposal = (work / "proposition.md").read_text(encoding="utf-8")
        report = run_tests(work)
        write_reports(work, spec, report)
        version = _version_for(context, str(spec["module_name"]), str(spec["version"]))
        spec = {**spec, "version": version}
        if version != demand.spec.get("version"):
            prepare_build(spec, work)
            report = run_tests(work)
            write_reports(work, spec, report)
            proposal = (work / "proposition.md").read_text(encoding="utf-8")
        package_modl(work, spec)
        if not report["passed"]:
            return AgentResult(
                state="failed",
                proposal=proposal,
                report=report,
                reason="Les tests Cucumber du module ont échoué.",
            )
        artifact = context.artifacts.publish(  # type: ignore[attr-defined]
            name=str(spec["module_name"]),
            version=version,
            demand_id=demand.id,
            source_dir=work,
            project_name=str(spec["project_name"]),
        )
        return AgentResult(
            state="done",
            proposal=proposal,
            report=report,
            artifact_ids=[artifact.id],
            comment=f"Artefact {artifact.name} {artifact.version} publié.",
        )


def _version_for(context: object, name: str, requested: str) -> str:
    existing = [
        artifact.version
        for artifact in context.artifacts.list()  # type: ignore[attr-defined]
        if artifact.name == name
    ]
    if requested not in existing:
        return requested
    major, minor, patch = (int(part) for part in requested.split("."))
    candidate = requested
    while candidate in existing:
        patch += 1
        candidate = f"{major}.{minor}.{patch}"
    return candidate
