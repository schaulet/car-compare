"""Service pour les simulations."""

from uuid import UUID

from car_compare.domain.calculator import TCOCalculator
from car_compare.domain.models import Simulation, SimulationResult
from car_compare.infrastructure.repositories import SimulationRepository, VehicleRepository


class SimulationService:
    """Service applicatif pour les simulations."""

    def __init__(
        self,
        simulation_repo: SimulationRepository,
        vehicle_repo: VehicleRepository,
        calculator: TCOCalculator | None = None,
    ):
        self.simulation_repo = simulation_repo
        self.vehicle_repo = vehicle_repo
        self.calculator = calculator or TCOCalculator()

    async def create_simulation(
        self, simulation: Simulation
    ) -> tuple[Simulation, SimulationResult]:
        """Crée une simulation et calcule le TCO."""
        vehicle = await self.vehicle_repo.get(simulation.vehicle_id)
        if not vehicle:
            raise ValueError(f"Vehicle {simulation.vehicle_id} not found")

        saved_simulation = await self.simulation_repo.save(simulation)
        result = self.calculator.calculate(vehicle, saved_simulation)
        return saved_simulation, result

    async def get_simulation(self, id: UUID) -> Simulation | None:
        """Récupère une simulation par son ID."""
        return await self.simulation_repo.get(id)

    async def get_simulation_result(self, id: UUID) -> SimulationResult | None:
        """Calcule le résultat d'une simulation existante."""
        simulation = await self.simulation_repo.get(id)
        if not simulation:
            return None

        vehicle = await self.vehicle_repo.get(simulation.vehicle_id)
        if not vehicle:
            return None

        return self.calculator.calculate(vehicle, simulation)

    async def list_simulations(self, limit: int = 100, offset: int = 0) -> list[Simulation]:
        """Liste les simulations."""
        return await self.simulation_repo.list(limit, offset)

    async def delete_simulation(self, id: UUID) -> bool:
        """Supprime une simulation."""
        return await self.simulation_repo.remove(id)
