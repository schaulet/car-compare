"""Registres génériques : analyseurs, politiques, outils."""

from __future__ import annotations

from collections.abc import Callable
from dataclasses import dataclass
from typing import Any, Protocol

from azurz.domain.models import AgentDescriptor, Analysis, CapabilityPolicy


class Analyzer(Protocol):
    def score(self, text: str) -> float: ...

    def analyze(self, text: str) -> Analysis: ...


class Agent(Protocol):
    descriptor: AgentDescriptor

    def execute(self, demand: Any, context: Any) -> Any: ...


@dataclass(frozen=True)
class ToolContribution:
    name: str
    description: str
    handler: Callable[..., str]


class AnalyzerRegistry:
    def __init__(self) -> None:
        self._items: list[Analyzer] = []

    def register(self, analyzer: Analyzer) -> None:
        self._items.append(analyzer)

    def analyze(self, text: str) -> Analysis:
        if not self._items:
            raise RuntimeError("aucun analyseur enregistré")
        ranked = sorted(self._items, key=lambda item: item.score(text), reverse=True)
        return ranked[0].analyze(text)


class PolicyRegistry:
    def __init__(self) -> None:
        self._items: dict[str, CapabilityPolicy] = {}

    def register(self, policy: CapabilityPolicy) -> None:
        self._items[policy.capability] = policy

    def required(self, capability: str) -> frozenset[str]:
        policy = self._items.get(capability)
        if policy is None:
            return frozenset()
        return policy.required_prerogatives

    def all(self) -> list[CapabilityPolicy]:
        return list(self._items.values())


class AgentRegistry:
    def __init__(self) -> None:
        self._items: dict[str, Agent] = {}

    def register(self, agent: Agent) -> None:
        self._items[agent.descriptor.id] = agent

    def get(self, agent_id: str) -> Agent:
        return self._items[agent_id]

    def all(self) -> list[Agent]:
        return list(self._items.values())


class ToolCatalog:
    def __init__(self) -> None:
        self._items: list[ToolContribution] = []

    def register(self, tool: ToolContribution) -> None:
        self._items.append(tool)

    def all(self) -> list[ToolContribution]:
        return list(self._items)

    def names(self) -> list[str]:
        return [item.name for item in self._items]
