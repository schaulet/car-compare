"""Tests d'intégration pour l'API."""

import pytest
from httpx import AsyncClient


@pytest.mark.asyncio
class TestHealthEndpoint:
    """Tests pour l'endpoint de santé."""

    async def test_health_check(self, client: AsyncClient):
        """L'endpoint de santé doit retourner 200."""
        response = await client.get("/health")

        assert response.status_code == 200
        assert response.json() == {"status": "healthy"}


@pytest.mark.asyncio
class TestVehiclesAPI:
    """Tests pour l'API des véhicules."""

    async def test_create_vehicle(self, client: AsyncClient):
        """Créer un véhicule doit retourner 201."""
        vehicle_data = {
            "brand": "Renault",
            "model": "Clio",
            "version": "Intens",
            "year": 2024,
            "fuel_type": "essence",
            "price": "22000.00",
            "consumption": "5.5",
            "co2_emissions": 125,
            "power_hp": 100,
            "transmission": "manuelle",
            "doors": 5,
            "seats": 5,
            "trunk_volume": 391,
        }

        response = await client.post("/api/v1/vehicles", json=vehicle_data)

        assert response.status_code == 201
        data = response.json()
        assert data["brand"] == "Renault"
        assert data["model"] == "Clio"
        assert "id" in data

    async def test_list_vehicles_empty(self, client: AsyncClient):
        """Lister les véhicules sans données doit retourner une liste vide."""
        response = await client.get("/api/v1/vehicles")

        assert response.status_code == 200
        assert response.json() == []

    async def test_get_vehicle_not_found(self, client: AsyncClient):
        """Récupérer un véhicule inexistant doit retourner 404."""
        response = await client.get("/api/v1/vehicles/00000000-0000-0000-0000-000000000000")

        assert response.status_code == 404

    async def test_delete_vehicle_not_found(self, client: AsyncClient):
        """Supprimer un véhicule inexistant doit retourner 404."""
        response = await client.delete("/api/v1/vehicles/00000000-0000-0000-0000-000000000000")

        assert response.status_code == 404

    async def test_create_and_get_vehicle(self, client: AsyncClient):
        """Créer puis récupérer un véhicule doit fonctionner."""
        vehicle_data = {
            "brand": "Peugeot",
            "model": "308",
            "version": "GT",
            "year": 2024,
            "fuel_type": "diesel",
            "price": "35000.00",
            "consumption": "4.5",
            "co2_emissions": 118,
            "power_hp": 130,
            "transmission": "automatique",
        }

        create_response = await client.post("/api/v1/vehicles", json=vehicle_data)
        vehicle_id = create_response.json()["id"]

        get_response = await client.get(f"/api/v1/vehicles/{vehicle_id}")

        assert get_response.status_code == 200
        assert get_response.json()["brand"] == "Peugeot"

    async def test_create_and_delete_vehicle(self, client: AsyncClient):
        """Créer puis supprimer un véhicule doit fonctionner."""
        vehicle_data = {
            "brand": "Tesla",
            "model": "Model 3",
            "version": "Long Range",
            "year": 2024,
            "fuel_type": "electrique",
            "price": "45000.00",
            "consumption": "15.0",
            "co2_emissions": 0,
            "power_hp": 450,
            "transmission": "automatique",
        }

        create_response = await client.post("/api/v1/vehicles", json=vehicle_data)
        vehicle_id = create_response.json()["id"]

        delete_response = await client.delete(f"/api/v1/vehicles/{vehicle_id}")

        assert delete_response.status_code == 204

        get_response = await client.get(f"/api/v1/vehicles/{vehicle_id}")
        assert get_response.status_code == 404


@pytest.mark.asyncio
class TestSimulationsAPI:
    """Tests pour l'API des simulations."""

    async def test_create_simulation_vehicle_not_found(self, client: AsyncClient):
        """Créer une simulation avec un véhicule inexistant doit retourner 404."""
        simulation_data = {
            "vehicle_id": "00000000-0000-0000-0000-000000000000",
            "duration_months": 60,
            "annual_km": 15000,
            "fuel_price": "1.85",
            "insurance_monthly": "50.00",
            "maintenance_annual": "500.00",
            "purchase_type": "cash",
        }

        response = await client.post("/api/v1/simulations", json=simulation_data)

        assert response.status_code == 404

    async def test_create_simulation_success(self, client: AsyncClient):
        """Créer une simulation valide doit retourner 201 avec le résultat."""
        vehicle_data = {
            "brand": "Renault",
            "model": "Clio",
            "version": "Intens",
            "year": 2024,
            "fuel_type": "essence",
            "price": "22000.00",
            "consumption": "5.5",
            "co2_emissions": 125,
            "power_hp": 100,
            "transmission": "manuelle",
        }
        vehicle_response = await client.post("/api/v1/vehicles", json=vehicle_data)
        vehicle_id = vehicle_response.json()["id"]

        simulation_data = {
            "vehicle_id": vehicle_id,
            "duration_months": 60,
            "annual_km": 15000,
            "fuel_price": "1.85",
            "insurance_monthly": "50.00",
            "maintenance_annual": "500.00",
            "purchase_type": "cash",
        }

        response = await client.post("/api/v1/simulations", json=simulation_data)

        assert response.status_code == 201
        data = response.json()
        assert "simulation" in data
        assert "result" in data
        assert data["result"]["tco"] is not None
        assert float(data["result"]["tco"]) > 0


@pytest.mark.asyncio
class TestComparisonsAPI:
    """Tests pour l'API des comparaisons."""

    async def test_create_comparison_insufficient_simulations(self, client: AsyncClient):
        """Créer une comparaison avec moins de 2 simulations doit retourner 400."""
        comparison_data = {
            "name": "Test comparison",
            "simulation_ids": ["00000000-0000-0000-0000-000000000000"],
        }

        response = await client.post("/api/v1/comparisons", json=comparison_data)

        assert response.status_code == 400

    async def test_create_comparison_simulation_not_found(self, client: AsyncClient):
        """Créer une comparaison avec des simulations inexistantes doit retourner 404."""
        comparison_data = {
            "name": "Test comparison",
            "simulation_ids": [
                "00000000-0000-0000-0000-000000000000",
                "00000000-0000-0000-0000-000000000001",
            ],
        }

        response = await client.post("/api/v1/comparisons", json=comparison_data)

        assert response.status_code == 404
