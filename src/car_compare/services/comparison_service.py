"""Service pour les comparaisons."""

from dataclasses import dataclass
from uuid import UUID

from car_compare.domain.calculator import TCOCalculator
from car_compare.domain.models import Comparison, SimulationResult, Vehicle
from car_compare.infrastructure.repositories import (
    ComparisonRepository,
    SimulationRepository,
    VehicleRepository,
)


@dataclass
class ComparisonItem:
    """Élément d'une comparaison."""

    vehicle: Vehicle
    result: SimulationResult


@dataclass
class ComparisonResult:
    """Résultat d'une comparaison."""

    comparison: Comparison
    items: list[ComparisonItem]
    best_tco_index: int
    best_monthly_index: int


class ComparisonService:
    """Service applicatif pour les comparaisons."""

    def __init__(
        self,
        comparison_repo: ComparisonRepository,
        simulation_repo: SimulationRepository,
        vehicle_repo: VehicleRepository,
        calculator: TCOCalculator | None = None,
    ):
        self.comparison_repo = comparison_repo
        self.simulation_repo = simulation_repo
        self.vehicle_repo = vehicle_repo
        self.calculator = calculator or TCOCalculator()

    async def create_comparison(self, comparison: Comparison) -> Comparison:
        """Crée une nouvelle comparaison."""
        for sim_id in comparison.simulation_ids:
            simulation = await self.simulation_repo.get(sim_id)
            if not simulation:
                raise ValueError(f"Simulation {sim_id} not found")
        return await self.comparison_repo.save(comparison)

    async def get_comparison(self, id: UUID) -> Comparison | None:
        """Récupère une comparaison par son ID."""
        return await self.comparison_repo.get(id)

    async def get_comparison_result(self, id: UUID) -> ComparisonResult | None:
        """Calcule le résultat complet d'une comparaison."""
        comparison = await self.comparison_repo.get(id)
        if not comparison:
            return None

        items: list[ComparisonItem] = []
        for sim_id in comparison.simulation_ids:
            simulation = await self.simulation_repo.get(sim_id)
            if not simulation:
                continue

            vehicle = await self.vehicle_repo.get(simulation.vehicle_id)
            if not vehicle:
                continue

            result = self.calculator.calculate(vehicle, simulation)
            items.append(ComparisonItem(vehicle=vehicle, result=result))

        if not items:
            return None

        best_tco_index = min(range(len(items)), key=lambda i: items[i].result.tco)
        best_monthly_index = min(range(len(items)), key=lambda i: items[i].result.monthly_cost)

        return ComparisonResult(
            comparison=comparison,
            items=items,
            best_tco_index=best_tco_index,
            best_monthly_index=best_monthly_index,
        )

    async def list_comparisons(self, limit: int = 100, offset: int = 0) -> list[Comparison]:
        """Liste les comparaisons."""
        return await self.comparison_repo.list(limit, offset)

    async def delete_comparison(self, id: UUID) -> bool:
        """Supprime une comparaison."""
        return await self.comparison_repo.remove(id)
