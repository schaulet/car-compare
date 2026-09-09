"""Modèles de domaine."""

from dataclasses import dataclass, field
from datetime import datetime
from decimal import Decimal
from typing import Optional
from uuid import UUID, uuid4

from .enums import FuelType, PurchaseType, TransmissionType


@dataclass
class Vehicle:
    """Représente un véhicule."""

    brand: str
    model: str
    version: str
    year: int
    fuel_type: FuelType
    price: Decimal
    consumption: Decimal
    co2_emissions: int
    power_hp: int
    transmission: TransmissionType
    doors: int = 5
    seats: int = 5
    trunk_volume: int = 0
    id: UUID = field(default_factory=uuid4)

    def __post_init__(self):
        if isinstance(self.price, (int, float)):
            self.price = Decimal(str(self.price))
        if isinstance(self.consumption, (int, float)):
            self.consumption = Decimal(str(self.consumption))


@dataclass
class Simulation:
    """Paramètres d'une simulation de coût."""

    vehicle_id: UUID
    duration_months: int
    annual_km: int
    fuel_price: Decimal
    insurance_monthly: Decimal
    maintenance_annual: Decimal
    purchase_type: PurchaseType
    monthly_payment: Optional[Decimal] = None
    deposit: Decimal = Decimal("0")
    residual_value: Optional[Decimal] = None
    id: UUID = field(default_factory=uuid4)

    def __post_init__(self):
        for attr in ["fuel_price", "insurance_monthly", "maintenance_annual", "deposit"]:
            val = getattr(self, attr)
            if isinstance(val, (int, float)):
                setattr(self, attr, Decimal(str(val)))
        if self.monthly_payment is not None and isinstance(self.monthly_payment, (int, float)):
            self.monthly_payment = Decimal(str(self.monthly_payment))
        if self.residual_value is not None and isinstance(self.residual_value, (int, float)):
            self.residual_value = Decimal(str(self.residual_value))


@dataclass
class SimulationResult:
    """Résultat d'une simulation TCO."""

    simulation_id: UUID
    vehicle_id: UUID
    tco: Decimal
    monthly_cost: Decimal
    fuel_cost_total: Decimal
    insurance_cost_total: Decimal
    maintenance_cost_total: Decimal
    depreciation: Decimal


@dataclass
class Comparison:
    """Comparaison de plusieurs simulations."""

    name: str
    simulation_ids: list[UUID]
    weights: dict[str, float] = field(default_factory=dict)
    id: UUID = field(default_factory=uuid4)
    created_at: datetime = field(default_factory=datetime.utcnow)
