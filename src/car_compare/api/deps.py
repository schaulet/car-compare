"""Dépendances pour l'injection."""


from fastapi import Depends
from sqlalchemy.ext.asyncio import AsyncSession

from car_compare.domain.calculator import TCOCalculator
from car_compare.infrastructure.database import get_db
from car_compare.infrastructure.repositories import (
    ComparisonRepository,
    SimulationRepository,
    VehicleRepository,
)
from car_compare.services import ComparisonService, SimulationService, VehicleService


async def get_vehicle_repository(
    db: AsyncSession = Depends(get_db),
) -> VehicleRepository:
    """Fournit une instance de VehicleRepository."""
    return VehicleRepository(db)


async def get_simulation_repository(
    db: AsyncSession = Depends(get_db),
) -> SimulationRepository:
    """Fournit une instance de SimulationRepository."""
    return SimulationRepository(db)


async def get_comparison_repository(
    db: AsyncSession = Depends(get_db),
) -> ComparisonRepository:
    """Fournit une instance de ComparisonRepository."""
    return ComparisonRepository(db)


async def get_vehicle_service(
    repo: VehicleRepository = Depends(get_vehicle_repository),
) -> VehicleService:
    """Fournit une instance de VehicleService."""
    return VehicleService(repo)


async def get_simulation_service(
    simulation_repo: SimulationRepository = Depends(get_simulation_repository),
    vehicle_repo: VehicleRepository = Depends(get_vehicle_repository),
) -> SimulationService:
    """Fournit une instance de SimulationService."""
    return SimulationService(simulation_repo, vehicle_repo, TCOCalculator())


async def get_comparison_service(
    comparison_repo: ComparisonRepository = Depends(get_comparison_repository),
    simulation_repo: SimulationRepository = Depends(get_simulation_repository),
    vehicle_repo: VehicleRepository = Depends(get_vehicle_repository),
) -> ComparisonService:
    """Fournit une instance de ComparisonService."""
    return ComparisonService(comparison_repo, simulation_repo, vehicle_repo, TCOCalculator())
