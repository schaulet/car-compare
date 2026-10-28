"""Agent de mise à disposition : déploie la version choisie vers les cibles choisies."""

from __future__ import annotations

from azurz.domain.models import AgentDescriptor, AgentResult, Demand


class DeployerAgent:
    def __init__(self) -> None:
        self.descriptor = AgentDescriptor(
            id="deploiement",
            name="Déploiement des artefacts",
            capabilities=("artifact.deploy",),
            prerogatives=frozenset({"artifact.read", "ignition.deploy"}),
            priority=20,
        )

    def execute(self, demand: Demand, context: object) -> AgentResult:
        spec = demand.spec
        artifact = context.artifacts.get(str(spec["artifact_id"]))  # type: ignore[attr-defined]
        if artifact.version != spec.get("version"):
            found = context.artifacts.find(artifact.name, str(spec["version"]))  # type: ignore[attr-defined]
            if found is None:
                return AgentResult(state="failed", reason="Version d'artefact introuvable.")
            artifact = found
        details = []
        statuses = []
        for target_id in spec.get("target_ids") or []:
            target = context.target_by_id(target_id)  # type: ignore[attr-defined]
            if target is None:
                details.append(
                    {
                        "target_id": target_id,
                        "status": "failed",
                        "detail": "instance inconnue",
                    }
                )
                statuses.append("failed")
                continue
            outcome = context.gateways.deploy(target, artifact)  # type: ignore[attr-defined]
            details.append(outcome)
            statuses.append(outcome["status"])
        deployment = context.deployments.add(  # type: ignore[attr-defined]
            artifact_id=artifact.id,
            version=artifact.version,
            target_ids=list(spec.get("target_ids") or []),
            status="failed" if "failed" in statuses else "done",
            demand_id=demand.id,
            details=details,
        )
        report = {
            "passed": deployment.status == "done",
            "deployment_id": deployment.id,
            "scenarios": [
                {
                    "name": item["target_id"],
                    "status": "passed" if item["status"] in {"deployed", "staged"} else "failed",
                    "evidence": item.get("detail") or item["status"],
                }
                for item in details
            ],
        }
        if deployment.status != "done":
            return AgentResult(
                state="failed",
                report=report,
                reason="Le déploiement n'a pas abouti sur toutes les instances.",
            )
        names = ", ".join(spec.get("target_ids") or [])
        return AgentResult(
            state="done",
            report=report,
            comment=f"{artifact.name} {artifact.version} déployé vers {names}.",
            reason="",
        )
