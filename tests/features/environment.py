"""Configuration de l'environnement Behave."""


def after_scenario(context, scenario):
    """Ferme le client HTTP du scénario s'il a été ouvert."""
    client = getattr(context, "http", None)
    if client is not None:
        client.close()


def before_scenario(context, scenario):
    """Initialise le contexte avant chaque scénario."""
    context.vehicles = []
    context.simulations = []
    context.results = {}
    context.vehicles_by_name = {}
    context.error = None

    from car_compare.domain.calculator import TCOCalculator
    context.calculator = TCOCalculator()
