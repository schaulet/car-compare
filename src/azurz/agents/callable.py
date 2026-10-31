"""Agent dont le comportement est une fonction fournie à l'enregistrement."""

from __future__ import annotations

from collections.abc import Callable
from typing import Any

from azurz.domain.models import AgentDescriptor, AgentResult, Demand


class FunctionAgent:
    def __init__(
        self,
        descriptor: AgentDescriptor,
        handler: Callable[[Demand, Any], AgentResult],
    ) -> None:
        self.descriptor = descriptor
        self._handler = handler

    def execute(self, demand: Demand, context: Any) -> AgentResult:
        return self._handler(demand, context)
