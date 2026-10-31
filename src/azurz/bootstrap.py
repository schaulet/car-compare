"""Composition : enregistre les spécialisations sans les coder dans l'hôte."""

from __future__ import annotations

import tempfile
from pathlib import Path

from azurz.adapters.azdo import AzdoClient, AzureDevOpsWorkItems
from azurz.adapters.filesystem import DeploymentLog, FileArtifacts, FilesystemGateways
from azurz.adapters.memory import MemoryWorkItems
from azurz.agents.deployer import DeployerAgent
from azurz.agents.ignition_module import IgnitionModuleAgent
from azurz.domain.analyzers import FallbackAnalyzer, IgnitionModuleAnalyzer
from azurz.domain.models import CapabilityPolicy, Target
from azurz.domain.registry import AgentRegistry, AnalyzerRegistry, PolicyRegistry
from azurz.platform import Platform


def register_defaults(
    analyzers: AnalyzerRegistry,
    agents: AgentRegistry,
    policies: PolicyRegistry,
) -> None:
    analyzers.register(IgnitionModuleAnalyzer())
    analyzers.register(FallbackAnalyzer())
    agents.register(IgnitionModuleAgent())
    agents.register(DeployerAgent())
    policies.register(
        CapabilityPolicy(
            capability="ignition.module.build",
            required_prerogatives=frozenset({"workitem.read", "artifact.publish", "tests.run"}),
        )
    )
    policies.register(
        CapabilityPolicy(
            capability="artifact.deploy",
            required_prerogatives=frozenset({"artifact.read", "ignition.deploy"}),
        )
    )


def default_targets() -> list[Target]:
    return [
        Target("ligne-a", "Ligne A", "ignition", "http://127.0.0.1:8088", "http://127.0.0.1:8088"),
        Target("ligne-b", "Ligne B", "ignition", "http://127.0.0.1:8089", "http://127.0.0.1:8089"),
        Target(
            "supervision",
            "Supervision",
            "ignition",
            "http://127.0.0.1:8090",
            "http://127.0.0.1:8090",
        ),
    ]


def create_memory(
    *,
    root: Path | None = None,
    targets: list[Target] | None = None,
) -> Platform:
    base = root or Path(tempfile.mkdtemp(prefix="azurz-"))
    analyzers = AnalyzerRegistry()
    agents = AgentRegistry()
    policies = PolicyRegistry()
    register_defaults(analyzers, agents, policies)
    ignition = base / "ignition"
    return Platform(
        workitems=MemoryWorkItems(),
        artifacts=FileArtifacts(base / "artifacts"),
        gateways=FilesystemGateways(ignition),
        deployments=DeploymentLog(),
        analyzers=analyzers,
        agents=agents,
        policies=policies,
        targets=targets if targets is not None else default_targets(),
        work_dir=base / "work",
        ignition_root=ignition,
    )


def create_connected(
    *,
    azdo_url: str,
    azdo_org: str,
    azdo_project: str,
    azdo_pat: str,
    data_dir: Path,
    ignition_root: Path,
    targets: list[Target],
    restart_gateways: bool = False,
) -> Platform:
    analyzers = AnalyzerRegistry()
    agents = AgentRegistry()
    policies = PolicyRegistry()
    register_defaults(analyzers, agents, policies)
    client = AzdoClient(azdo_url, azdo_org, azdo_project, azdo_pat)
    return Platform(
        workitems=AzureDevOpsWorkItems(client),
        artifacts=FileArtifacts(data_dir / "artifacts"),
        gateways=FilesystemGateways(ignition_root, restart=restart_gateways),
        deployments=DeploymentLog(),
        analyzers=analyzers,
        agents=agents,
        policies=policies,
        targets=targets,
        work_dir=data_dir / "work",
        ignition_root=ignition_root,
    )
