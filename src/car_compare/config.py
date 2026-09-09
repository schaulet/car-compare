"""Configuration de l'application."""

from pydantic_settings import BaseSettings


class Settings(BaseSettings):
    """Configuration globale."""

    database_url: str = "sqlite+aiosqlite:///./car_compare.db"
    debug: bool = False
    api_prefix: str = "/api/v1"

    model_config = {"env_file": ".env", "extra": "ignore"}


settings = Settings()
