# Car Compare

Outil d'aide à l'achat/location de voiture - Comparateur de véhicules avec calcul du TCO (Total Cost of Ownership).

## Fonctionnalités

- **Gestion des véhicules** : Ajout, modification et suppression de véhicules
- **Simulation TCO** : Calcul du coût total de possession incluant carburant, assurance, entretien et dépréciation
- **Comparaison** : Comparaison côte à côte de plusieurs véhicules avec identification du meilleur choix

## Installation

```bash
# Cloner le projet
git clone <repository-url>
cd car-compare

# Installer les dépendances
uv sync --all-extras

# Lancer les tests
uv run pytest
uv run behave tests/features/

# Lancer l'application
uv run uvicorn car_compare.main:app --reload
```

## Documentation

- [Documentation générale](docs/doc.md)
- [Spécifications fonctionnelles](docs/spec.md)
- [Architecture technique](docs/archi.md)
- [Guide utilisateur](docs/user.md)

## API

L'API est disponible sur `http://localhost:8000`

- Documentation Swagger : `http://localhost:8000/docs`
- Documentation ReDoc : `http://localhost:8000/redoc`

## Stack technique

- Python 3.11+
- FastAPI
- SQLAlchemy (async)
- Pydantic
- pytest + behave (tests)
