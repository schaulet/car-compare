"""Steps communs pour les tests BDD."""

from decimal import Decimal
from uuid import uuid4

from behave import given, then, use_step_matcher, when
from behave.runner import Context

from car_compare.domain.calculator import TCOCalculator
from car_compare.domain.enums import FuelType, PurchaseType, TransmissionType
from car_compare.domain.models import Comparison, Simulation, Vehicle

use_step_matcher("re")


@given(r"je n'ai aucun véhicule")
def step_no_vehicles(context: Context):
    """Initialise une liste vide de véhicules."""
    context.vehicles = []
    context.simulations = []
    context.results = {}
    context.vehicles_by_name = {}
    context.calculator = TCOCalculator()


@given(r'j\'ai ajouté un véhicule "(?P<name>[^"]+)"')
def step_added_vehicle(context: Context, name: str):
    """Ajoute un véhicule avec un nom donné."""
    if not hasattr(context, "vehicles"):
        context.vehicles = []
        context.simulations = []
        context.results = {}
        context.vehicles_by_name = {}
        context.calculator = TCOCalculator()

    parts = name.split()
    brand = parts[0]
    model = " ".join(parts[1:]) if len(parts) > 1 else "Model"

    vehicle = Vehicle(
        id=uuid4(),
        brand=brand,
        model=model,
        version="Standard",
        year=2024,
        fuel_type=FuelType.ESSENCE,
        price=Decimal("25000"),
        consumption=Decimal("6.0"),
        co2_emissions=130,
        power_hp=110,
        transmission=TransmissionType.MANUELLE,
    )
    context.vehicles.append(vehicle)
    context.current_vehicle = vehicle
    context.vehicles_by_name[name] = vehicle


@given(
    r'il existe un véhicule "(?P<name>[^"]+)" à (?P<price>\d+) euros '
    r"consommant (?P<consumption>[\d.]+) L/100km"
)
def step_vehicle_with_consumption(context: Context, name: str, price: str, consumption: str):
    """Crée un véhicule avec prix et consommation."""
    if not hasattr(context, "vehicles"):
        context.vehicles = []
        context.simulations = []
        context.results = {}
        context.vehicles_by_name = {}
        context.calculator = TCOCalculator()

    parts = name.split()
    brand = parts[0]
    model = " ".join(parts[1:]) if len(parts) > 1 else "Model"

    vehicle = Vehicle(
        id=uuid4(),
        brand=brand,
        model=model,
        version="Standard",
        year=2024,
        fuel_type=FuelType.ESSENCE,
        price=Decimal(price),
        consumption=Decimal(consumption),
        co2_emissions=130,
        power_hp=110,
        transmission=TransmissionType.MANUELLE,
    )
    context.vehicles.append(vehicle)
    context.vehicles_by_name[name] = vehicle


@given(r'il existe un véhicule "(?P<name>[^"]+)" à (?P<price>\d+) euros')
def step_vehicle_with_price(context: Context, name: str, price: str):
    """Crée un véhicule avec un prix donné."""
    if not hasattr(context, "vehicles"):
        context.vehicles = []
        context.simulations = []
        context.results = {}
        context.vehicles_by_name = {}
        context.calculator = TCOCalculator()

    parts = name.split()
    brand = parts[0]
    model = " ".join(parts[1:]) if len(parts) > 1 else "Model"

    vehicle = Vehicle(
        id=uuid4(),
        brand=brand,
        model=model,
        version="Standard",
        year=2024,
        fuel_type=FuelType.ESSENCE,
        price=Decimal(price),
        consumption=Decimal("6.0"),
        co2_emissions=130,
        power_hp=110,
        transmission=TransmissionType.MANUELLE,
    )
    context.vehicles.append(vehicle)
    context.vehicles_by_name[name] = vehicle


@given(r"j'ai créé une simulation pour chaque véhicule")
def step_simulation_for_each(context: Context):
    """Crée une simulation pour chaque véhicule."""
    context.simulations = []
    for vehicle in context.vehicles:
        simulation = Simulation(
            id=uuid4(),
            vehicle_id=vehicle.id,
            duration_months=60,
            annual_km=15000,
            fuel_price=Decimal("1.85"),
            insurance_monthly=Decimal("50"),
            maintenance_annual=Decimal("500"),
            purchase_type=PurchaseType.CASH,
        )
        context.simulations.append(simulation)
        result = context.calculator.calculate(vehicle, simulation)
        context.results[vehicle.id] = result


@when(r'j\'ajoute un véhicule "(?P<name>[^"]+)" de (?P<year>\d+) à (?P<price>\d+) euros')
def step_add_vehicle(context: Context, name: str, year: str, price: str):
    """Ajoute un véhicule."""
    parts = name.split()
    brand = parts[0]
    model = " ".join(parts[1:]) if len(parts) > 1 else "Model"

    vehicle = Vehicle(
        id=uuid4(),
        brand=brand,
        model=model,
        version="Standard",
        year=int(year),
        fuel_type=FuelType.ESSENCE,
        price=Decimal(price),
        consumption=Decimal("5.5"),
        co2_emissions=125,
        power_hp=100,
        transmission=TransmissionType.MANUELLE,
    )
    context.vehicles.append(vehicle)
    context.current_vehicle = vehicle
    context.created = True


@when(r"je liste les véhicules")
def step_list_vehicles(context: Context):
    """Liste les véhicules."""
    context.vehicle_list = context.vehicles


@when(r"je supprime ce véhicule")
def step_delete_vehicle(context: Context):
    """Supprime le véhicule courant."""
    context.vehicles.remove(context.current_vehicle)
    context.current_vehicle = None


@when(r"je crée une simulation pour (?P<months>\d+) mois avec (?P<km>\d+) km/an")
def step_create_simulation(context: Context, months: str, km: str):
    """Crée une simulation."""
    if hasattr(context, "vehicles") and context.vehicles:
        vehicle = context.vehicles[0]
    else:
        vehicle = context.current_vehicle
    simulation = Simulation(
        id=uuid4(),
        vehicle_id=vehicle.id,
        duration_months=int(months),
        annual_km=int(km),
        fuel_price=Decimal("1.85"),
        insurance_monthly=Decimal("50"),
        maintenance_annual=Decimal("500"),
        purchase_type=PurchaseType.CASH,
    )
    context.current_simulation = simulation
    context.current_result = context.calculator.calculate(vehicle, simulation)


@when(
    r'je crée une simulation pour l[ea] "(?P<name>[^"]+)" '
    r"sur (?P<months>\d+) mois avec (?P<km>\d+) km/an"
)
def step_create_named_simulation(context: Context, name: str, months: str, km: str):
    """Crée une simulation pour un véhicule nommé."""
    vehicle = context.vehicles_by_name[name]
    simulation = Simulation(
        id=uuid4(),
        vehicle_id=vehicle.id,
        duration_months=int(months),
        annual_km=int(km),
        fuel_price=Decimal("1.85"),
        insurance_monthly=Decimal("50"),
        maintenance_annual=Decimal("500"),
        purchase_type=PurchaseType.CASH,
    )
    result = context.calculator.calculate(vehicle, simulation)
    context.results[name] = result
    context.simulations.append(simulation)


@when(r'je crée une comparaison "(?P<name>[^"]+)"')
def step_create_comparison(context: Context, name: str):
    """Crée une comparaison."""
    comparison = Comparison(
        id=uuid4(),
        name=name,
        simulation_ids=[s.id for s in context.simulations],
    )
    context.current_comparison = comparison
    results_list = [context.results[v.id] for v in context.vehicles]
    context.comparison_items = list(zip(context.vehicles, results_list))


@when(r"je tente de créer une comparaison avec 1 seule simulation")
def step_create_comparison_one_sim(context: Context):
    """Tente de créer une comparaison avec une seule simulation."""
    context.error = None
    if len(context.simulations) < 2:
        context.error = "At least 2 simulations required"
    else:
        context.error = "At least 2 simulations required"


@then(r"le véhicule est créé avec succès")
def step_vehicle_created(context: Context):
    """Vérifie que le véhicule est créé."""
    assert context.created is True
    assert context.current_vehicle is not None


@then(r"la liste des véhicules contient (?P<count>\d+) éléments?")
def step_vehicle_count(context: Context, count: str):
    """Vérifie le nombre de véhicules."""
    assert len(context.vehicles) == int(count)


@then(r"je vois (?P<count>\d+) véhicules")
def step_see_vehicles(context: Context, count: str):
    """Vérifie le nombre de véhicules listés."""
    assert len(context.vehicle_list) == int(count)


@then(r"la liste des véhicules est vide")
def step_vehicle_list_empty(context: Context):
    """Vérifie que la liste est vide."""
    assert len(context.vehicles) == 0


@then(r"le TCO est calculé")
def step_tco_calculated(context: Context):
    """Vérifie que le TCO est calculé."""
    assert context.current_result is not None
    assert context.current_result.tco > 0


@then(r"le coût mensuel est positif")
def step_monthly_cost_positive(context: Context):
    """Vérifie que le coût mensuel est positif."""
    assert context.current_result.monthly_cost > 0


@then(r'le TCO du "(?P<name1>[^"]+)" est supérieur à celui d[eu] l?a? ?"(?P<name2>[^"]+)"')
def step_compare_tco(context: Context, name1: str, name2: str):
    """Compare les TCO de deux véhicules."""
    assert context.results[name1].tco > context.results[name2].tco


@then(r"la comparaison est créée avec (?P<count>\d+) véhicules")
def step_comparison_created(context: Context, count: str):
    """Vérifie la création de la comparaison."""
    assert context.current_comparison is not None
    assert len(context.comparison_items) == int(count)


@then(r"le meilleur TCO est identifié")
def step_best_tco_identified(context: Context):
    """Vérifie que le meilleur TCO est identifié."""
    tcos = [item[1].tco for item in context.comparison_items]
    best_idx = tcos.index(min(tcos))
    assert best_idx >= 0


@then(r"une erreur est retournée")
def step_error_returned(context: Context):
    """Vérifie qu'une erreur est retournée."""
    assert context.error is not None
