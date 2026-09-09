"""Tests unitaires pour le calculateur TCO."""

from decimal import Decimal
from uuid import uuid4

import pytest

from car_compare.domain.calculator import TCOCalculator
from car_compare.domain.enums import FuelType, PurchaseType, TransmissionType
from car_compare.domain.models import Simulation, Vehicle


@pytest.fixture
def calculator():
    """Fixture pour le calculateur."""
    return TCOCalculator()


@pytest.fixture
def sample_vehicle():
    """Fixture pour un véhicule de test."""
    return Vehicle(
        id=uuid4(),
        brand="Renault",
        model="Clio",
        version="Intens",
        year=2024,
        fuel_type=FuelType.ESSENCE,
        price=Decimal("22000"),
        consumption=Decimal("5.5"),
        co2_emissions=125,
        power_hp=100,
        transmission=TransmissionType.MANUELLE,
        doors=5,
        seats=5,
        trunk_volume=391,
    )


@pytest.fixture
def sample_simulation(sample_vehicle):
    """Fixture pour une simulation de test."""
    return Simulation(
        id=uuid4(),
        vehicle_id=sample_vehicle.id,
        duration_months=60,
        annual_km=15000,
        fuel_price=Decimal("1.85"),
        insurance_monthly=Decimal("50"),
        maintenance_annual=Decimal("500"),
        purchase_type=PurchaseType.CASH,
    )


class TestTCOCalculator:
    """Tests pour le calculateur TCO."""

    def test_calculate_returns_result(self, calculator, sample_vehicle, sample_simulation):
        """Le calculateur doit retourner un résultat."""
        result = calculator.calculate(sample_vehicle, sample_simulation)

        assert result is not None
        assert result.simulation_id == sample_simulation.id
        assert result.vehicle_id == sample_vehicle.id

    def test_calculate_tco_positive(self, calculator, sample_vehicle, sample_simulation):
        """Le TCO doit être positif."""
        result = calculator.calculate(sample_vehicle, sample_simulation)

        assert result.tco > 0

    def test_calculate_monthly_cost_positive(self, calculator, sample_vehicle, sample_simulation):
        """Le coût mensuel doit être positif."""
        result = calculator.calculate(sample_vehicle, sample_simulation)

        assert result.monthly_cost > 0

    def test_calculate_fuel_cost(self, calculator, sample_vehicle, sample_simulation):
        """Le coût carburant doit être calculé correctement."""
        result = calculator.calculate(sample_vehicle, sample_simulation)

        duration_years = Decimal("60") / Decimal("12")
        total_km = Decimal("15000") * duration_years
        expected_fuel_cost = (Decimal("5.5") / Decimal("100")) * total_km * Decimal("1.85")

        assert result.fuel_cost_total == expected_fuel_cost.quantize(Decimal("0.01"))

    def test_calculate_insurance_cost(self, calculator, sample_vehicle, sample_simulation):
        """Le coût assurance doit être calculé correctement."""
        result = calculator.calculate(sample_vehicle, sample_simulation)

        expected_insurance = Decimal("50") * Decimal("60")

        assert result.insurance_cost_total == expected_insurance.quantize(Decimal("0.01"))

    def test_calculate_maintenance_cost(self, calculator, sample_vehicle, sample_simulation):
        """Le coût entretien doit être calculé correctement."""
        result = calculator.calculate(sample_vehicle, sample_simulation)

        duration_years = Decimal("60") / Decimal("12")
        expected_maintenance = Decimal("500") * duration_years

        assert result.maintenance_cost_total == expected_maintenance.quantize(Decimal("0.01"))

    def test_depreciation_first_year(self, calculator):
        """La dépréciation première année doit être 20%."""
        depreciation = calculator._calculate_depreciation(Decimal("20000"), 12)

        expected = Decimal("20000") * Decimal("0.20")
        assert depreciation == expected.quantize(Decimal("0.01"))

    def test_depreciation_multiple_years(self, calculator):
        """La dépréciation sur plusieurs années doit être cumulative."""
        depreciation = calculator._calculate_depreciation(Decimal("20000"), 24)

        year1 = Decimal("20000") * Decimal("0.20")
        remaining_after_year1 = Decimal("20000") - year1
        year2 = remaining_after_year1 * Decimal("0.15")
        expected = year1 + year2

        assert depreciation == expected.quantize(Decimal("0.01"))

    def test_with_custom_residual_value(self, calculator, sample_vehicle, sample_simulation):
        """Le résultat avec valeur résiduelle personnalisée doit être correct."""
        sample_simulation.residual_value = Decimal("10000")

        result = calculator.calculate(sample_vehicle, sample_simulation)

        assert result.tco < sample_vehicle.price + result.fuel_cost_total

    def test_different_fuel_consumption_impacts_cost(
        self, calculator, sample_vehicle, sample_simulation
    ):
        """Une consommation différente doit impacter le coût."""
        result1 = calculator.calculate(sample_vehicle, sample_simulation)

        high_consumption_vehicle = Vehicle(
            id=uuid4(),
            brand="SUV",
            model="Big",
            version="V8",
            year=2024,
            fuel_type=FuelType.ESSENCE,
            price=Decimal("22000"),
            consumption=Decimal("12.0"),
            co2_emissions=250,
            power_hp=300,
            transmission=TransmissionType.AUTOMATIQUE,
        )
        simulation2 = Simulation(
            id=uuid4(),
            vehicle_id=high_consumption_vehicle.id,
            duration_months=60,
            annual_km=15000,
            fuel_price=Decimal("1.85"),
            insurance_monthly=Decimal("50"),
            maintenance_annual=Decimal("500"),
            purchase_type=PurchaseType.CASH,
        )
        result2 = calculator.calculate(high_consumption_vehicle, simulation2)

        assert result2.fuel_cost_total > result1.fuel_cost_total
