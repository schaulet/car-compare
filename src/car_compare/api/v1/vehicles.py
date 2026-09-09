"""API endpoints pour les véhicules."""

from decimal import Decimal
from uuid import UUID

from fastapi import APIRouter, Depends, HTTPException, status
from pydantic import BaseModel

from car_compare.api.deps import get_vehicle_service
from car_compare.domain.enums import FuelType, TransmissionType
from car_compare.domain.models import Vehicle
from car_compare.services import VehicleService

router = APIRouter()


class VehicleCreate(BaseModel):
    """Schéma de création d'un véhicule."""

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


class VehicleResponse(BaseModel):
    """Schéma de réponse d'un véhicule."""

    id: UUID
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
    doors: int
    seats: int
    trunk_volume: int

    model_config = {"from_attributes": True}


@router.post("", response_model=VehicleResponse, status_code=status.HTTP_201_CREATED)
async def create_vehicle(
    data: VehicleCreate,
    service: VehicleService = Depends(get_vehicle_service),
) -> VehicleResponse:
    """Crée un nouveau véhicule."""
    vehicle = Vehicle(
        brand=data.brand,
        model=data.model,
        version=data.version,
        year=data.year,
        fuel_type=data.fuel_type,
        price=data.price,
        consumption=data.consumption,
        co2_emissions=data.co2_emissions,
        power_hp=data.power_hp,
        transmission=data.transmission,
        doors=data.doors,
        seats=data.seats,
        trunk_volume=data.trunk_volume,
    )
    created = await service.create_vehicle(vehicle)
    return VehicleResponse(
        id=created.id,
        brand=created.brand,
        model=created.model,
        version=created.version,
        year=created.year,
        fuel_type=created.fuel_type,
        price=created.price,
        consumption=created.consumption,
        co2_emissions=created.co2_emissions,
        power_hp=created.power_hp,
        transmission=created.transmission,
        doors=created.doors,
        seats=created.seats,
        trunk_volume=created.trunk_volume,
    )


@router.get("", response_model=list[VehicleResponse])
async def list_vehicles(
    limit: int = 100,
    offset: int = 0,
    service: VehicleService = Depends(get_vehicle_service),
) -> list[VehicleResponse]:
    """Liste tous les véhicules."""
    vehicles = await service.list_vehicles(limit, offset)
    return [
        VehicleResponse(
            id=v.id,
            brand=v.brand,
            model=v.model,
            version=v.version,
            year=v.year,
            fuel_type=v.fuel_type,
            price=v.price,
            consumption=v.consumption,
            co2_emissions=v.co2_emissions,
            power_hp=v.power_hp,
            transmission=v.transmission,
            doors=v.doors,
            seats=v.seats,
            trunk_volume=v.trunk_volume,
        )
        for v in vehicles
    ]


@router.get("/{vehicle_id}", response_model=VehicleResponse)
async def get_vehicle(
    vehicle_id: UUID,
    service: VehicleService = Depends(get_vehicle_service),
) -> VehicleResponse:
    """Récupère un véhicule par son ID."""
    vehicle = await service.get_vehicle(vehicle_id)
    if not vehicle:
        raise HTTPException(status_code=404, detail="Vehicle not found")
    return VehicleResponse(
        id=vehicle.id,
        brand=vehicle.brand,
        model=vehicle.model,
        version=vehicle.version,
        year=vehicle.year,
        fuel_type=vehicle.fuel_type,
        price=vehicle.price,
        consumption=vehicle.consumption,
        co2_emissions=vehicle.co2_emissions,
        power_hp=vehicle.power_hp,
        transmission=vehicle.transmission,
        doors=vehicle.doors,
        seats=vehicle.seats,
        trunk_volume=vehicle.trunk_volume,
    )


@router.delete("/{vehicle_id}", status_code=status.HTTP_204_NO_CONTENT)
async def delete_vehicle(
    vehicle_id: UUID,
    service: VehicleService = Depends(get_vehicle_service),
) -> None:
    """Supprime un véhicule."""
    deleted = await service.delete_vehicle(vehicle_id)
    if not deleted:
        raise HTTPException(status_code=404, detail="Vehicle not found")
