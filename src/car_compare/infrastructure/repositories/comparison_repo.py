"""Repository pour les comparaisons."""

from uuid import UUID

from sqlalchemy.ext.asyncio import AsyncSession

from car_compare.domain.models import Comparison
from car_compare.infrastructure.models import ComparisonModel

from .base import BaseRepository


class ComparisonRepository(BaseRepository[ComparisonModel]):
    """Repository pour les opérations sur les comparaisons."""

    def __init__(self, session: AsyncSession):
        super().__init__(session, ComparisonModel)

    def to_domain(self, model: ComparisonModel) -> Comparison:
        """Convertit un modèle DB en entité de domaine."""
        return Comparison(
            id=model.id,
            name=model.name,
            simulation_ids=[UUID(sid) for sid in model.simulation_ids],
            weights=model.weights,
            created_at=model.created_at,
        )

    def to_model(self, entity: Comparison) -> ComparisonModel:
        """Convertit une entité de domaine en modèle DB."""
        return ComparisonModel(
            id=entity.id,
            name=entity.name,
            simulation_ids=[str(sid) for sid in entity.simulation_ids],
            weights=entity.weights,
            created_at=entity.created_at,
        )

    async def get(self, id: UUID) -> Comparison | None:
        """Récupère une comparaison par son ID."""
        model = await self.get_by_id(id)
        return self.to_domain(model) if model else None

    async def list(self, limit: int = 100, offset: int = 0) -> list[Comparison]:
        """Liste toutes les comparaisons."""
        models = await self.get_all(limit, offset)
        return [self.to_domain(m) for m in models]

    async def save(self, comparison: Comparison) -> Comparison:
        """Sauvegarde une comparaison."""
        model = self.to_model(comparison)
        saved = await self.create(model)
        return self.to_domain(saved)

    async def remove(self, id: UUID) -> bool:
        """Supprime une comparaison."""
        return await self.delete(id)
