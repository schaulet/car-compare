"""Modèles SQLAlchemy."""

import uuid
from datetime import datetime
from decimal import Decimal

from sqlalchemy import JSON, DateTime, Enum, Integer, Numeric, String, Uuid
from sqlalchemy.orm import Mapped, mapped_column

from car_compare.domain.enums import FuelType, PurchaseType, TransmissionType

from .database import Base


class VehicleModel(Base):
    """Table des véhicules."""

    __tablename__ = "vehicles"

    id: Mapped[uuid.UUID] = mapped_column(Uuid, primary_key=True, default=uuid.uuid4)
    brand: Mapped[str] = mapped_column(String(100), nullable=False)
    model: Mapped[str] = mapped_column(String(100), nullable=False)
    version: Mapped[str] = mapped_column(String(100), nullable=False)
    year: Mapped[int] = mapped_column(Integer, nullable=False)
    fuel_type: Mapped[FuelType] = mapped_column(Enum(FuelType), nullable=False)
    price: Mapped[Decimal] = mapped_column(Numeric(12, 2), nullable=False)
    consumption: Mapped[Decimal] = mapped_column(Numeric(6, 2), nullable=False)
    co2_emissions: Mapped[int] = mapped_column(Integer, nullable=False)
    power_hp: Mapped[int] = mapped_column(Integer, nullable=False)
    transmission: Mapped[TransmissionType] = mapped_column(Enum(TransmissionType), nullable=False)
    doors: Mapped[int] = mapped_column(Integer, default=5)
    seats: Mapped[int] = mapped_column(Integer, default=5)
    trunk_volume: Mapped[int] = mapped_column(Integer, default=0)


class SimulationModel(Base):
    """Table des simulations."""

    __tablename__ = "simulations"

    id: Mapped[uuid.UUID] = mapped_column(Uuid, primary_key=True, default=uuid.uuid4)
    vehicle_id: Mapped[uuid.UUID] = mapped_column(Uuid, nullable=False)
    duration_months: Mapped[int] = mapped_column(Integer, nullable=False)
    annual_km: Mapped[int] = mapped_column(Integer, nullable=False)
    fuel_price: Mapped[Decimal] = mapped_column(Numeric(6, 3), nullable=False)
    insurance_monthly: Mapped[Decimal] = mapped_column(Numeric(10, 2), nullable=False)
    maintenance_annual: Mapped[Decimal] = mapped_column(Numeric(10, 2), nullable=False)
    purchase_type: Mapped[PurchaseType] = mapped_column(Enum(PurchaseType), nullable=False)
    monthly_payment: Mapped[Decimal | None] = mapped_column(Numeric(10, 2), nullable=True)
    deposit: Mapped[Decimal] = mapped_column(Numeric(12, 2), default=Decimal("0"))
    residual_value: Mapped[Decimal | None] = mapped_column(Numeric(12, 2), nullable=True)


class ComparisonModel(Base):
    """Table des comparaisons."""

    __tablename__ = "comparisons"

    id: Mapped[uuid.UUID] = mapped_column(Uuid, primary_key=True, default=uuid.uuid4)
    name: Mapped[str] = mapped_column(String(200), nullable=False)
    simulation_ids: Mapped[list] = mapped_column(JSON, nullable=False)
    weights: Mapped[dict] = mapped_column(JSON, default=dict)
    created_at: Mapped[datetime] = mapped_column(DateTime, default=datetime.utcnow)
