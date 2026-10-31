"""Cas d'utilisation du pont : intake, traitement, déploiement."""

from __future__ import annotations

from dataclasses import dataclass, replace
from pathlib import Path
from typing import Any, Protocol

from azurz.domain.models import AgentResult, Demand, Target
from azurz.domain.registry import AgentRegistry, AnalyzerRegistry, PolicyRegistry
from azurz.errors import AzurzError
from azurz.orchestration import apply_result, rejection_reason, select_agents

OEE_MESSAGE = (
    "Je veux un module Ignition nommé « OEE Ligne 1 » qui expose la fonction "
    "compute_oee(availability, performance, quality) = availability * performance * quality "
    "et un tag mémoire Plant/Ligne1/OEE."
)


class WorkItemPort(Protocol):
    def create(self, demand: Demand) -> Demand: ...

    def save(self, demand: Demand) -> Demand: ...

    def get(self, demand_id: int) -> Demand: ...

    def list(self) -> list[Demand]: ...


class ArtifactPort(Protocol):
    def publish(
        self,
        *,
        name: str,
        version: str,
        demand_id: int,
        source_dir: Path,
        project_name: str,
    ) -> Any: ...

    def get(self, artifact_id: str) -> Any: ...

    def find(self, name: str, version: str) -> Any: ...

    def list(self) -> list[Any]: ...


class GatewayPort(Protocol):
    def deploy(self, target: Target, artifact: Any) -> dict[str, str]: ...


class DeploymentPort(Protocol):
    def add(self, **kwargs: Any) -> Any: ...

    def list(self) -> list[Any]: ...


@dataclass
class ExecContext:
    artifacts: ArtifactPort
    gateways: GatewayPort
    deployments: DeploymentPort
    work_dir: Path
    target_by_id: Any


class Platform:
    def __init__(
        self,
        *,
        workitems: WorkItemPort,
        artifacts: ArtifactPort,
        gateways: GatewayPort,
        deployments: DeploymentPort,
        analyzers: AnalyzerRegistry,
        agents: AgentRegistry,
        policies: PolicyRegistry,
        targets: list[Target],
        work_dir: Path,
        ignition_root: Path,
    ) -> None:
        self.workitems = workitems
        self.artifacts = artifacts
        self.gateways = gateways
        self.deployments = deployments
        self.analyzers = analyzers
        self.agents = agents
        self.policies = policies
        self.targets = {target.id: target for target in targets}
        self.work_dir = work_dir
        self.ignition_root = ignition_root
        self.auto_process = False
        self.work_dir.mkdir(parents=True, exist_ok=True)

    def submit_message(self, message: str, requester: str = "dust") -> Demand:
        demand = self._draft([message], requester)
        demand.journal.append("Demande créée depuis le chat.")
        stored = self.workitems.create(demand)
        return self._autostart(stored)

    def iterate(self, demand_id: int, message: str) -> Demand:
        current = self.workitems.get(demand_id)
        if current.state == "in_progress":
            raise AzurzError("busy", "La demande est en cours de traitement.")
        messages = [*current.messages, message]
        drafted = self._draft(messages, current.requester)
        drafted.id = current.id
        drafted.rev = current.rev
        drafted.artifact_ids = list(current.artifact_ids)
        drafted.journal = [
            *current.journal,
            "Demande enrichie depuis le chat.",
            drafted.description,
        ]
        drafted.proposal = "" if drafted.state == "ready" else current.proposal
        drafted.report = None if drafted.state == "ready" else current.report
        drafted.agent_id = ""
        saved = self.workitems.save(drafted)
        return self._autostart(saved)

    def set_auto(self, enabled: bool) -> None:
        self.auto_process = enabled

    def tick(self) -> list[Demand]:
        if not self.auto_process:
            return []
        processed = []
        for demand in self.workitems.list():
            if demand.state == "ready":
                processed.append(self.trigger(demand.id))
        return processed

    def trigger(self, demand_id: int) -> Demand:
        demand = self.workitems.get(demand_id)
        if demand.state != "ready":
            raise AzurzError("not_ready", "La demande n'est pas prête à être traitée.")
        chosen = select_agents(self.agents.all(), self.policies, demand.capability)
        if not chosen:
            demand.state = "blocked"
            demand.reason = rejection_reason(self.agents.all(), self.policies, demand.capability)
            demand.journal.append(demand.reason)
            return self.workitems.save(demand)
        agent = chosen[0]
        demand.state = "in_progress"
        demand.journal.append(f"Pris en charge par {agent.descriptor.id}.")
        self.workitems.save(demand)
        context = self._context()
        try:
            result = agent.execute(demand, context)
        except Exception as exc:
            result = AgentResult(state="failed", reason=str(exc))
        current = self.workitems.get(demand_id)
        apply_result(current, agent.descriptor.id, result)
        return self.workitems.save(current)

    def deploy(self, artifact_id: str, version: str, target_ids: list[str]) -> Demand:
        artifact = self.artifacts.get(artifact_id)
        if artifact.version != version:
            found = self.artifacts.find(artifact.name, version)
            if found is None:
                raise AzurzError("missing", "Version introuvable")
            artifact = found
        unknown = [target_id for target_id in target_ids if target_id not in self.targets]
        if unknown or not target_ids:
            raise AzurzError("target", "Sélectionnez au moins une instance connue.")
        message = f"Déployer {artifact.name} {artifact.version} vers {', '.join(target_ids)}"
        demand = Demand(
            id=0,
            title=f"Déployer {artifact.name} {artifact.version}",
            description=message,
            state="ready",
            capability="artifact.deploy",
            messages=[message],
            acceptance_criteria=message,
            spec={
                "artifact_id": artifact.id,
                "version": artifact.version,
                "target_ids": list(target_ids),
                "name": artifact.name,
            },
            requester="operator",
            tags=["azurz", "deploy"],
            missing=[],
        )
        demand.journal.append("Déploiement demandé depuis l'interface.")
        stored = self.workitems.create(demand)
        return self.trigger(stored.id)

    def _draft(self, messages: list[str], requester: str) -> Demand:
        text = "\n".join(messages)
        analysis = self.analyzers.analyze(text)
        state = "ready" if analysis.actionable else "needs_input"
        return Demand(
            id=0,
            title=analysis.title,
            description=analysis.summary,
            state=state,
            capability=analysis.capability,
            messages=list(messages),
            acceptance_criteria=analysis.acceptance_criteria,
            spec=analysis.spec,
            requester=requester,
            missing=list(analysis.missing),
            tags=list(analysis.tags),
            journal=[analysis.summary],
        )

    def _autostart(self, demand: Demand) -> Demand:
        if self.auto_process and demand.state == "ready":
            return self.trigger(demand.id)
        return demand

    def _context(self) -> ExecContext:
        return ExecContext(
            artifacts=self.artifacts,
            gateways=self.gateways,
            deployments=self.deployments,
            work_dir=self.work_dir,
            target_by_id=self.targets.get,
        )

    def project_dir(self, target_id: str, project_name: str) -> Path:
        return self.ignition_root / target_id / "projects" / project_name

    def revoke_prerogative(self, agent_id: str, prerogative: str) -> None:
        agent = self.agents.get(agent_id)
        agent.descriptor = replace(
            agent.descriptor,
            prerogatives=agent.descriptor.prerogatives - {prerogative},
        )
