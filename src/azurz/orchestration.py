"""Hôte de traitement. Il ne connaît aucun métier : seulement le registre."""

from __future__ import annotations

from collections.abc import Sequence

from azurz.domain.models import AgentResult, Demand
from azurz.domain.registry import Agent, PolicyRegistry


def select_agents(
    agents: Sequence[Agent],
    policies: PolicyRegistry,
    capability: str,
) -> list[Agent]:
    required = policies.required(capability)
    eligible = [
        agent
        for agent in agents
        if capability in agent.descriptor.capabilities and required <= agent.descriptor.prerogatives
    ]
    return sorted(eligible, key=lambda agent: agent.descriptor.priority)


def rejection_reason(agents: Sequence[Agent], policies: PolicyRegistry, capability: str) -> str:
    required = policies.required(capability)
    holders = [agent for agent in agents if capability in agent.descriptor.capabilities]
    if not holders:
        return f"Aucun agent n'a la capacité {capability or 'non précisée'}"
    parts: list[str] = []
    for agent in holders:
        missing = sorted(required - agent.descriptor.prerogatives)
        if missing:
            joined = ", ".join(missing)
            parts.append(f"{agent.descriptor.id} n'a pas les prérogatives {joined}")
    if not parts:
        return f"Aucun agent éligible pour {capability}"
    return "; ".join(parts)


def apply_result(demand: Demand, agent_id: str, result: AgentResult) -> Demand:
    demand.state = result.state
    demand.agent_id = agent_id
    demand.reason = result.reason
    if result.proposal:
        demand.proposal = result.proposal
    if result.report is not None:
        demand.report = result.report
    if result.artifact_ids:
        for artifact_id in result.artifact_ids:
            if artifact_id not in demand.artifact_ids:
                demand.artifact_ids.append(artifact_id)
    if result.comment:
        demand.journal.append(result.comment)
    demand.journal.append(f"Traitement par {agent_id} : {result.state}")
    return demand
