"""API endpoints pour les simulations."""

from decimal import Decimal
from uuid import UUID

from fastapi import APIRouter, Depends, HTTPException, status
from pydantic import BaseModel

from car_compare.api.deps import get_simulation_service
from car_compare.domain.enums import PurchaseType
from car_compare.domain.models import Simulation
from car_compare.services import SimulationService

router = APIRouter()


class SimulationCreate(BaseModel):
    """Schéma de création d'une simulation."""

    vehicle_id: UUID
    duration_months: int
    annual_km: int
    fuel_price: Decimal
    insurance_monthly: Decimal
    maintenance_annual: Decimal
    purchase_type: PurchaseType
    monthly_payment: Decimal | None = None
    deposit: Decimal = Decimal("0")
    residual_value: Decimal | None = None


class SimulationResponse(BaseModel):
    """Schéma de réponse d'une simulation."""

    id: UUID
    vehicle_id: UUID
    duration_months: int
    annual_km: int
    fuel_price: Decimal
    insurance_monthly: Decimal
    maintenance_annual: Decimal
    purchase_type: PurchaseType
    monthly_payment: Decimal | None
    deposit: Decimal
    residual_value: Decimal | None

    model_config = {"from_attributes": True}


class SimulationResultResponse(BaseModel):
    """Schéma de réponse d'un résultat de simulation."""

    simulation_id: UUID
    vehicle_id: UUID
    tco: Decimal
    monthly_cost: Decimal
    fuel_cost_total: Decimal
    insurance_cost_total: Decimal
    maintenance_cost_total: Decimal
    depreciation: Decimal


class SimulationWithResultResponse(BaseModel):
    """Schéma de réponse combinant simulation et résultat."""

    simulation: SimulationResponse
    result: SimulationResultResponse


@router.post("", response_model=SimulationWithResultResponse, status_code=status.HTTP_201_CREATED)
async def create_simulation(
    data: SimulationCreate,
    service: SimulationService = Depends(get_simulation_service),
) -> SimulationWithResultResponse:
    """Crée une nouvelle simulation et calcule le TCO."""
    simulation = Simulation(
        vehicle_id=data.vehicle_id,
        duration_months=data.duration_months,
        annual_km=data.annual_km,
        fuel_price=data.fuel_price,
        insurance_monthly=data.insurance_monthly,
        maintenance_annual=data.maintenance_annual,
        purchase_type=data.purchase_type,
        monthly_payment=data.monthly_payment,
        deposit=data.deposit,
        residual_value=data.residual_value,
    )
    try:
        created, result = await service.create_simulation(simulation)
    except ValueError as e:
        raise HTTPException(status_code=404, detail=str(e))

    return SimulationWithResultResponse(
        simulation=SimulationResponse(
            id=created.id,
            vehicle_id=created.vehicle_id,
            duration_months=created.duration_months,
            annual_km=created.annual_km,
            fuel_price=created.fuel_price,
            insurance_monthly=created.insurance_monthly,
            maintenance_annual=created.maintenance_annual,
            purchase_type=created.purchase_type,
            monthly_payment=created.monthly_payment,
            deposit=created.deposit,
            residual_value=created.residual_value,
        ),
        result=SimulationResultResponse(
            simulation_id=result.simulation_id,
            vehicle_id=result.vehicle_id,
            tco=result.tco,
            monthly_cost=result.monthly_cost,
            fuel_cost_total=result.fuel_cost_total,
            insurance_cost_total=result.insurance_cost_total,
            maintenance_cost_total=result.maintenance_cost_total,
            depreciation=result.depreciation,
        ),
    )


@router.get("", response_model=list[SimulationResponse])
async def list_simulations(
    limit: int = 100,
    offset: int = 0,
    service: SimulationService = Depends(get_simulation_service),
) -> list[SimulationResponse]:
    """Liste toutes les simulations."""
    simulations = await service.list_simulations(limit, offset)
    return [
        SimulationResponse(
            id=s.id,
            vehicle_id=s.vehicle_id,
            duration_months=s.duration_months,
            annual_km=s.annual_km,
            fuel_price=s.fuel_price,
            insurance_monthly=s.insurance_monthly,
            maintenance_annual=s.maintenance_annual,
            purchase_type=s.purchase_type,
            monthly_payment=s.monthly_payment,
            deposit=s.deposit,
            residual_value=s.residual_value,
        )
        for s in simulations
    ]


@router.get("/{simulation_id}", response_model=SimulationWithResultResponse)
async def get_simulation(
    simulation_id: UUID,
    service: SimulationService = Depends(get_simulation_service),
) -> SimulationWithResultResponse:
    """Récupère une simulation et son résultat."""
    simulation = await service.get_simulation(simulation_id)
    if not simulation:
        raise HTTPException(status_code=404, detail="Simulation not found")

    result = await service.get_simulation_result(simulation_id)
    if not result:
        raise HTTPException(status_code=404, detail="Could not calculate result")

    return SimulationWithResultResponse(
        simulation=SimulationResponse(
            id=simulation.id,
            vehicle_id=simulation.vehicle_id,
            duration_months=simulation.duration_months,
            annual_km=simulation.annual_km,
            fuel_price=simulation.fuel_price,
            insurance_monthly=simulation.insurance_monthly,
            maintenance_annual=simulation.maintenance_annual,
            purchase_type=simulation.purchase_type,
            monthly_payment=simulation.monthly_payment,
            deposit=simulation.deposit,
            residual_value=simulation.residual_value,
        ),
        result=SimulationResultResponse(
            simulation_id=result.simulation_id,
            vehicle_id=result.vehicle_id,
            tco=result.tco,
            monthly_cost=result.monthly_cost,
            fuel_cost_total=result.fuel_cost_total,
            insurance_cost_total=result.insurance_cost_total,
            maintenance_cost_total=result.maintenance_cost_total,
            depreciation=result.depreciation,
        ),
    )


@router.delete("/{simulation_id}", status_code=status.HTTP_204_NO_CONTENT)
async def delete_simulation(
    simulation_id: UUID,
    service: SimulationService = Depends(get_simulation_service),
) -> None:
    """Supprime une simulation."""
    deleted = await service.delete_simulation(simulation_id)
    if not deleted:
        raise HTTPException(status_code=404, detail="Simulation not found")
