"""Modèles de domaine partagés par les hôtes et les agents."""

from __future__ import annotations

from dataclasses import dataclass, field
from typing import Any


@dataclass
class Analysis:
    title: str
    summary: str
    capability: str
    actionable: bool
    missing: list[str]
    spec: dict[str, Any]
    acceptance_criteria: str
    tags: list[str]


@dataclass
class Demand:
    id: int
    title: str
    description: str
    state: str
    capability: str
    messages: list[str]
    acceptance_criteria: str
    spec: dict[str, Any]
    proposal: str = ""
    report: dict[str, Any] | None = None
    artifact_ids: list[str] = field(default_factory=list)
    reason: str = ""
    requester: str = "dust"
    rev: int = 1
    missing: list[str] = field(default_factory=list)
    tags: list[str] = field(default_factory=list)
    journal: list[str] = field(default_factory=list)
    agent_id: str = ""


@dataclass(frozen=True)
class AgentDescriptor:
    id: str
    name: str
    capabilities: tuple[str, ...]
    prerogatives: frozenset[str]
    priority: int = 100


@dataclass(frozen=True)
class CapabilityPolicy:
    capability: str
    required_prerogatives: frozenset[str]


@dataclass
class Artifact:
    id: str
    name: str
    version: str
    demand_id: int
    path: str
    project_name: str
    created_at: str


@dataclass(frozen=True)
class Target:
    id: str
    name: str
    kind: str
    internal_url: str
    public_url: str
    container: str = ""


@dataclass
class Deployment:
    id: str
    artifact_id: str
    version: str
    target_ids: list[str]
    status: str
    demand_id: int
    details: list[dict[str, Any]] = field(default_factory=list)


@dataclass
class AgentResult:
    state: str
    proposal: str = ""
    report: dict[str, Any] | None = None
    artifact_ids: list[str] | None = None
    reason: str = ""
    comment: str = ""
