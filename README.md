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
xc setup

# Lancer les tests
xc test

# Lancer l'application
xc dev
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
- [xc](https://xcfile.dev) - Task runner

## Tasks

### setup

Installe les dépendances du projet.

```bash
uv sync --all-extras
```

### dev

Démarre le serveur de développement.

Requires: setup

```bash
uv run uvicorn car_compare.main:app --reload
```

### test

Lance tous les tests du projet (pytest + behave).

Requires: setup

```bash
uv run pytest
uv run behave tests/features/
```

### test-unit

Lance uniquement les tests unitaires.

Requires: setup

```bash
uv run pytest tests/unit/
```

### test-integration

Lance uniquement les tests d'intégration.

Requires: setup

```bash
uv run pytest tests/integration/
```

### test-bdd

Lance les tests BDD avec Behave.

Requires: setup

```bash
uv run behave tests/features/
```

### lint

Vérifie la qualité du code avec ruff.

Requires: setup

```bash
uv run ruff check .
```

### format

Formate le code source avec ruff.

Requires: setup

```bash
uv run ruff format .
```

### build

Construit le package pour distribution.

Requires: setup

```bash
uv build
```

### clean

Nettoie les fichiers générés.

```bash
rm -rf .venv dist __pycache__ .pytest_cache .ruff_cache *.egg-info
find . -type d -name "__pycache__" -exec rm -rf {} + 2>/dev/null || true
find . -type d -name "*.egg-info" -exec rm -rf {} + 2>/dev/null || true
```
