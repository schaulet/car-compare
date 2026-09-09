"""Configuration de l'environnement Behave."""


def before_scenario(context, scenario):
    """Initialise le contexte avant chaque scénario."""
    context.vehicles = []
    context.simulations = []
    context.results = {}
    context.vehicles_by_name = {}
    context.error = None

    from car_compare.domain.calculator import TCOCalculator
    context.calculator = TCOCalculator()
