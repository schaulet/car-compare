"""Repository pour les véhicules."""

from uuid import UUID

from sqlalchemy.ext.asyncio import AsyncSession

from car_compare.domain.models import Vehicle
from car_compare.infrastructure.models import VehicleModel

from .base import BaseRepository


class VehicleRepository(BaseRepository[VehicleModel]):
    """Repository pour les opérations sur les véhicules."""

    def __init__(self, session: AsyncSession):
        super().__init__(session, VehicleModel)

    def to_domain(self, model: VehicleModel) -> Vehicle:
        """Convertit un modèle DB en entité de domaine."""
        return Vehicle(
            id=model.id,
            brand=model.brand,
            model=model.model,
            version=model.version,
            year=model.year,
            fuel_type=model.fuel_type,
            price=model.price,
            consumption=model.consumption,
            co2_emissions=model.co2_emissions,
            power_hp=model.power_hp,
            transmission=model.transmission,
            doors=model.doors,
            seats=model.seats,
            trunk_volume=model.trunk_volume,
        )

    def to_model(self, entity: Vehicle) -> VehicleModel:
        """Convertit une entité de domaine en modèle DB."""
        return VehicleModel(
            id=entity.id,
            brand=entity.brand,
            model=entity.model,
            version=entity.version,
            year=entity.year,
            fuel_type=entity.fuel_type,
            price=entity.price,
            consumption=entity.consumption,
            co2_emissions=entity.co2_emissions,
            power_hp=entity.power_hp,
            transmission=entity.transmission,
            doors=entity.doors,
            seats=entity.seats,
            trunk_volume=entity.trunk_volume,
        )

    async def get(self, id: UUID) -> Vehicle | None:
        """Récupère un véhicule par son ID."""
        model = await self.get_by_id(id)
        return self.to_domain(model) if model else None

    async def list(self, limit: int = 100, offset: int = 0) -> list[Vehicle]:
        """Liste tous les véhicules."""
        models = await self.get_all(limit, offset)
        return [self.to_domain(m) for m in models]

    async def save(self, vehicle: Vehicle) -> Vehicle:
        """Sauvegarde un véhicule."""
        model = self.to_model(vehicle)
        saved = await self.create(model)
        return self.to_domain(saved)

    async def remove(self, id: UUID) -> bool:
        """Supprime un véhicule."""
        return await self.delete(id)
