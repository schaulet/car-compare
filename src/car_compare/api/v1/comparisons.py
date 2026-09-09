"""API endpoints pour les comparaisons."""

from datetime import datetime
from decimal import Decimal
from uuid import UUID

from fastapi import APIRouter, Depends, HTTPException, status
from pydantic import BaseModel

from car_compare.api.deps import get_comparison_service
from car_compare.domain.enums import FuelType, TransmissionType
from car_compare.domain.models import Comparison
from car_compare.services.comparison_service import ComparisonService

router = APIRouter()


class ComparisonCreate(BaseModel):
    """Schéma de création d'une comparaison."""

    name: str
    simulation_ids: list[UUID]
    weights: dict[str, float] = {}


class ComparisonResponse(BaseModel):
    """Schéma de réponse d'une comparaison."""

    id: UUID
    name: str
    simulation_ids: list[UUID]
    weights: dict[str, float]
    created_at: datetime

    model_config = {"from_attributes": True}


class VehicleSummary(BaseModel):
    """Résumé d'un véhicule pour la comparaison."""

    id: UUID
    brand: str
    model: str
    version: str
    year: int
    fuel_type: FuelType
    price: Decimal
    transmission: TransmissionType


class ResultSummary(BaseModel):
    """Résumé d'un résultat pour la comparaison."""

    tco: Decimal
    monthly_cost: Decimal
    fuel_cost_total: Decimal
    insurance_cost_total: Decimal
    maintenance_cost_total: Decimal
    depreciation: Decimal


class ComparisonItemResponse(BaseModel):
    """Élément de comparaison dans la réponse."""

    vehicle: VehicleSummary
    result: ResultSummary


class ComparisonResultResponse(BaseModel):
    """Réponse complète d'une comparaison."""

    comparison: ComparisonResponse
    items: list[ComparisonItemResponse]
    best_tco_index: int
    best_monthly_index: int


@router.post("", response_model=ComparisonResponse, status_code=status.HTTP_201_CREATED)
async def create_comparison(
    data: ComparisonCreate,
    service: ComparisonService = Depends(get_comparison_service),
) -> ComparisonResponse:
    """Crée une nouvelle comparaison."""
    if len(data.simulation_ids) < 2:
        raise HTTPException(
            status_code=400, detail="At least 2 simulations required for comparison"
        )

    comparison = Comparison(
        name=data.name,
        simulation_ids=data.simulation_ids,
        weights=data.weights,
    )
    try:
        created = await service.create_comparison(comparison)
    except ValueError as e:
        raise HTTPException(status_code=404, detail=str(e))

    return ComparisonResponse(
        id=created.id,
        name=created.name,
        simulation_ids=created.simulation_ids,
        weights=created.weights,
        created_at=created.created_at,
    )


@router.get("", response_model=list[ComparisonResponse])
async def list_comparisons(
    limit: int = 100,
    offset: int = 0,
    service: ComparisonService = Depends(get_comparison_service),
) -> list[ComparisonResponse]:
    """Liste toutes les comparaisons."""
    comparisons = await service.list_comparisons(limit, offset)
    return [
        ComparisonResponse(
            id=c.id,
            name=c.name,
            simulation_ids=c.simulation_ids,
            weights=c.weights,
            created_at=c.created_at,
        )
        for c in comparisons
    ]


@router.get("/{comparison_id}", response_model=ComparisonResultResponse)
async def get_comparison(
    comparison_id: UUID,
    service: ComparisonService = Depends(get_comparison_service),
) -> ComparisonResultResponse:
    """Récupère une comparaison avec ses résultats."""
    result = await service.get_comparison_result(comparison_id)
    if not result:
        raise HTTPException(status_code=404, detail="Comparison not found")

    return ComparisonResultResponse(
        comparison=ComparisonResponse(
            id=result.comparison.id,
            name=result.comparison.name,
            simulation_ids=result.comparison.simulation_ids,
            weights=result.comparison.weights,
            created_at=result.comparison.created_at,
        ),
        items=[
            ComparisonItemResponse(
                vehicle=VehicleSummary(
                    id=item.vehicle.id,
                    brand=item.vehicle.brand,
                    model=item.vehicle.model,
                    version=item.vehicle.version,
                    year=item.vehicle.year,
                    fuel_type=item.vehicle.fuel_type,
                    price=item.vehicle.price,
                    transmission=item.vehicle.transmission,
                ),
                result=ResultSummary(
                    tco=item.result.tco,
                    monthly_cost=item.result.monthly_cost,
                    fuel_cost_total=item.result.fuel_cost_total,
                    insurance_cost_total=item.result.insurance_cost_total,
                    maintenance_cost_total=item.result.maintenance_cost_total,
                    depreciation=item.result.depreciation,
                ),
            )
            for item in result.items
        ],
        best_tco_index=result.best_tco_index,
        best_monthly_index=result.best_monthly_index,
    )


@router.delete("/{comparison_id}", status_code=status.HTTP_204_NO_CONTENT)
async def delete_comparison(
    comparison_id: UUID,
    service: ComparisonService = Depends(get_comparison_service),
) -> None:
    """Supprime une comparaison."""
    deleted = await service.delete_comparison(comparison_id)
    if not deleted:
        raise HTTPException(status_code=404, detail="Comparison not found")
