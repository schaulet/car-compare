"""Base repository avec opérations génériques."""

from typing import Generic, TypeVar
from uuid import UUID

from sqlalchemy import select
from sqlalchemy.ext.asyncio import AsyncSession

from car_compare.infrastructure.database import Base

ModelType = TypeVar("ModelType", bound=Base)


class BaseRepository(Generic[ModelType]):
    """Repository de base avec opérations CRUD génériques."""

    def __init__(self, session: AsyncSession, model_class: type[ModelType]):
        self.session = session
        self.model_class = model_class

    async def get_by_id(self, id: UUID) -> ModelType | None:
        """Récupère une entité par son ID."""
        return await self.session.get(self.model_class, id)

    async def get_all(self, limit: int = 100, offset: int = 0) -> list[ModelType]:
        """Récupère toutes les entités avec pagination."""
        stmt = select(self.model_class).limit(limit).offset(offset)
        result = await self.session.execute(stmt)
        return list(result.scalars().all())

    async def create(self, entity: ModelType) -> ModelType:
        """Crée une nouvelle entité."""
        self.session.add(entity)
        await self.session.commit()
        await self.session.refresh(entity)
        return entity

    async def update(self, entity: ModelType) -> ModelType:
        """Met à jour une entité."""
        await self.session.commit()
        await self.session.refresh(entity)
        return entity

    async def delete(self, id: UUID) -> bool:
        """Supprime une entité par son ID."""
        entity = await self.get_by_id(id)
        if entity:
            await self.session.delete(entity)
            await self.session.commit()
            return True
        return False
