# car-compare

Outil d'aide à l'achat/location de voiture.

## Installation

### Prérequis

- Python 3.11+
- [uv](https://github.com/astral-sh/uv) - Gestionnaire de packages Python
- [xc](https://xcfile.dev) - Task runner (optionnel)

### Démarrage rapide

```bash
xc setup
xc dev
```

## Tasks

### setup

Installe les dépendances du projet.

```bash
uv venv
uv sync
```

### dev

Démarre le serveur de développement.

Requires: setup

```bash
uv run python -m car_compare
```

### test

Lance les tests du projet.

Requires: setup

```bash
uv run pytest
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
rm -rf .venv dist __pycache__ .pytest_cache .ruff_cache
find . -type d -name "__pycache__" -exec rm -rf {} + 2>/dev/null || true
```
