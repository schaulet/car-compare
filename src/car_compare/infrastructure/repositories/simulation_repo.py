"""Repository pour les simulations."""

from uuid import UUID

from sqlalchemy.ext.asyncio import AsyncSession

from car_compare.domain.models import Simulation
from car_compare.infrastructure.models import SimulationModel

from .base import BaseRepository


class SimulationRepository(BaseRepository[SimulationModel]):
    """Repository pour les opérations sur les simulations."""

    def __init__(self, session: AsyncSession):
        super().__init__(session, SimulationModel)

    def to_domain(self, model: SimulationModel) -> Simulation:
        """Convertit un modèle DB en entité de domaine."""
        return Simulation(
            id=model.id,
            vehicle_id=model.vehicle_id,
            duration_months=model.duration_months,
            annual_km=model.annual_km,
            fuel_price=model.fuel_price,
            insurance_monthly=model.insurance_monthly,
            maintenance_annual=model.maintenance_annual,
            purchase_type=model.purchase_type,
            monthly_payment=model.monthly_payment,
            deposit=model.deposit,
            residual_value=model.residual_value,
        )

    def to_model(self, entity: Simulation) -> SimulationModel:
        """Convertit une entité de domaine en modèle DB."""
        return SimulationModel(
            id=entity.id,
            vehicle_id=entity.vehicle_id,
            duration_months=entity.duration_months,
            annual_km=entity.annual_km,
            fuel_price=entity.fuel_price,
            insurance_monthly=entity.insurance_monthly,
            maintenance_annual=entity.maintenance_annual,
            purchase_type=entity.purchase_type,
            monthly_payment=entity.monthly_payment,
            deposit=entity.deposit,
            residual_value=entity.residual_value,
        )

    async def get(self, id: UUID) -> Simulation | None:
        """Récupère une simulation par son ID."""
        model = await self.get_by_id(id)
        return self.to_domain(model) if model else None

    async def list(self, limit: int = 100, offset: int = 0) -> list[Simulation]:
        """Liste toutes les simulations."""
        models = await self.get_all(limit, offset)
        return [self.to_domain(m) for m in models]

    async def save(self, simulation: Simulation) -> Simulation:
        """Sauvegarde une simulation."""
        model = self.to_model(simulation)
        saved = await self.create(model)
        return self.to_domain(saved)

    async def remove(self, id: UUID) -> bool:
        """Supprime une simulation."""
        return await self.delete(id)
