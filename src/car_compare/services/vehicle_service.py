"""Service pour la gestion des véhicules."""

from uuid import UUID

from car_compare.domain.models import Vehicle
from car_compare.infrastructure.repositories import VehicleRepository


class VehicleService:
    """Service applicatif pour les véhicules."""

    def __init__(self, repository: VehicleRepository):
        self.repository = repository

    async def create_vehicle(self, vehicle: Vehicle) -> Vehicle:
        """Crée un nouveau véhicule."""
        return await self.repository.save(vehicle)

    async def get_vehicle(self, id: UUID) -> Vehicle | None:
        """Récupère un véhicule par son ID."""
        return await self.repository.get(id)

    async def list_vehicles(self, limit: int = 100, offset: int = 0) -> list[Vehicle]:
        """Liste les véhicules avec pagination."""
        return await self.repository.list(limit, offset)

    async def delete_vehicle(self, id: UUID) -> bool:
        """Supprime un véhicule."""
        return await self.repository.remove(id)
