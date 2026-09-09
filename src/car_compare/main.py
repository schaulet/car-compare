"""Point d'entrée de l'application FastAPI."""

from contextlib import asynccontextmanager

from fastapi import FastAPI

from car_compare.api.v1 import router as api_v1_router
from car_compare.config import settings
from car_compare.infrastructure.database import init_db


@asynccontextmanager
async def lifespan(app: FastAPI):
    """Initialisation et nettoyage de l'application."""
    await init_db()
    yield


app = FastAPI(
    title="Car Compare API",
    description="API pour comparer les véhicules et calculer le TCO",
    version="0.1.0",
    lifespan=lifespan,
)

app.include_router(api_v1_router, prefix=settings.api_prefix)


@app.get("/health")
async def health_check():
    """Endpoint de santé."""
    return {"status": "healthy"}


def main():
    """Point d'entrée CLI."""
    import uvicorn

    uvicorn.run("car_compare.main:app", host="0.0.0.0", port=8000, reload=settings.debug)


if __name__ == "__main__":
    main()
