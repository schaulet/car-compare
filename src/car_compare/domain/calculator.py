"""Calculateur TCO (Total Cost of Ownership)."""

from decimal import Decimal

from .models import Simulation, SimulationResult, Vehicle


class TCOCalculator:
    """Calcule le coût total de possession d'un véhicule."""

    DEPRECIATION_RATES = {
        1: Decimal("0.20"),
        2: Decimal("0.15"),
        3: Decimal("0.10"),
        4: Decimal("0.07"),
        5: Decimal("0.07"),
    }
    DEFAULT_DEPRECIATION_RATE = Decimal("0.05")

    def calculate(self, vehicle: Vehicle, simulation: Simulation) -> SimulationResult:
        """Calcule le TCO pour un véhicule et une simulation donnés."""
        duration_years = Decimal(simulation.duration_months) / Decimal("12")
        total_km = Decimal(simulation.annual_km) * duration_years

        fuel_cost = self._calculate_fuel_cost(
            vehicle.consumption, total_km, simulation.fuel_price
        )
        insurance_cost = simulation.insurance_monthly * Decimal(simulation.duration_months)
        maintenance_cost = simulation.maintenance_annual * duration_years
        depreciation = self._calculate_depreciation(vehicle.price, simulation.duration_months)

        if simulation.residual_value is not None:
            final_residual = simulation.residual_value
        else:
            final_residual = vehicle.price - depreciation

        tco = (
            vehicle.price
            + fuel_cost
            + insurance_cost
            + maintenance_cost
            - final_residual
        )

        monthly_cost = tco / Decimal(simulation.duration_months)

        return SimulationResult(
            simulation_id=simulation.id,
            vehicle_id=vehicle.id,
            tco=tco.quantize(Decimal("0.01")),
            monthly_cost=monthly_cost.quantize(Decimal("0.01")),
            fuel_cost_total=fuel_cost.quantize(Decimal("0.01")),
            insurance_cost_total=insurance_cost.quantize(Decimal("0.01")),
            maintenance_cost_total=maintenance_cost.quantize(Decimal("0.01")),
            depreciation=depreciation.quantize(Decimal("0.01")),
        )

    def _calculate_fuel_cost(
        self, consumption: Decimal, total_km: Decimal, fuel_price: Decimal
    ) -> Decimal:
        """Calcule le coût total de carburant."""
        return (consumption / Decimal("100")) * total_km * fuel_price

    def _calculate_depreciation(self, price: Decimal, duration_months: int) -> Decimal:
        """Calcule la dépréciation totale sur la durée."""
        total_depreciation = Decimal("0")
        remaining_value = price
        years = duration_months // 12
        remaining_months = duration_months % 12

        for year in range(1, years + 1):
            rate = self.DEPRECIATION_RATES.get(year, self.DEFAULT_DEPRECIATION_RATE)
            yearly_depreciation = remaining_value * rate
            total_depreciation += yearly_depreciation
            remaining_value -= yearly_depreciation

        if remaining_months > 0:
            next_year = years + 1
            rate = self.DEPRECIATION_RATES.get(next_year, self.DEFAULT_DEPRECIATION_RATE)
            monthly_depreciation = (remaining_value * rate) / Decimal("12")
            total_depreciation += monthly_depreciation * Decimal(remaining_months)

        return total_depreciation
