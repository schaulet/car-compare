"""Configuration de la base de données."""

from sqlalchemy.ext.asyncio import AsyncSession, async_sessionmaker, create_async_engine
from sqlalchemy.orm import DeclarativeBase

from car_compare.config import settings


class Base(DeclarativeBase):
    """Base class for SQLAlchemy models."""

    pass


engine = create_async_engine(settings.database_url, echo=settings.debug)
async_session = async_sessionmaker(engine, class_=AsyncSession, expire_on_commit=False)


async def get_db():
    """Dependency pour obtenir une session DB."""
    async with async_session() as session:
        try:
            yield session
        finally:
            await session.close()


async def init_db():
    """Initialise la base de données."""
    async with engine.begin() as conn:
        await conn.run_sync(Base.metadata.create_all)
