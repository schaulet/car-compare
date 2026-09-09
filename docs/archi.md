# Architecture Technique - Car Compare

## 1. Vue d'ensemble

```
┌─────────────────────────────────────────────────────────┐
│                      Client (Future)                     │
│                   (Web / Mobile / CLI)                   │
└─────────────────────────────────────────────────────────┘
                            │
                            ▼
┌─────────────────────────────────────────────────────────┐
│                       API Layer                          │
│                       (FastAPI)                          │
│  ┌─────────────┐ ┌─────────────┐ ┌─────────────┐       │
│  │  Vehicles   │ │ Simulations │ │ Comparisons │       │
│  │   Router    │ │   Router    │ │   Router    │       │
│  └─────────────┘ └─────────────┘ └─────────────┘       │
└─────────────────────────────────────────────────────────┘
                            │
                            ▼
┌─────────────────────────────────────────────────────────┐
│                     Domain Layer                         │
│  ┌─────────────┐ ┌─────────────┐ ┌─────────────┐       │
│  │  Vehicle    │ │ Simulation  │ │ Comparison  │       │
│  │  Service    │ │  Service    │ │  Service    │       │
│  └─────────────┘ └─────────────┘ └─────────────┘       │
│  ┌─────────────────────────────────────────────┐       │
│  │           TCO Calculator (Domain Pure)       │       │
│  └─────────────────────────────────────────────┘       │
└─────────────────────────────────────────────────────────┘
                            │
                            ▼
┌─────────────────────────────────────────────────────────┐
│                  Infrastructure Layer                    │
│  ┌─────────────┐ ┌─────────────┐ ┌─────────────┐       │
│  │  Vehicle    │ │ Simulation  │ │ Comparison  │       │
│  │   Repo      │ │    Repo     │ │    Repo     │       │
│  └─────────────┘ └─────────────┘ └─────────────┘       │
│  ┌─────────────────────────────────────────────┐       │
│  │              SQLAlchemy ORM                  │       │
│  └─────────────────────────────────────────────┘       │
└─────────────────────────────────────────────────────────┘
                            │
                            ▼
┌─────────────────────────────────────────────────────────┐
│                    Database Layer                        │
│                 SQLite / PostgreSQL                      │
└─────────────────────────────────────────────────────────┘
```

## 2. Structure des dossiers

```
car-compare/
├── docs/                    # Documentation
│   ├── journal.md          # Journal de suivi
│   ├── doc.md              # Documentation générale
│   ├── spec.md             # Spécifications fonctionnelles
│   ├── archi.md            # Architecture technique
│   └── user.md             # Documentation utilisateur
├── src/
│   └── car_compare/
│       ├── __init__.py
│       ├── main.py         # Point d'entrée FastAPI
│       ├── config.py       # Configuration
│       ├── domain/         # Couche métier (pure)
│       │   ├── __init__.py
│       │   ├── models.py   # Modèles de domaine
│       │   ├── enums.py    # Énumérations
│       │   └── calculator.py # Calculs TCO
│       ├── services/       # Services applicatifs
│       │   ├── __init__.py
│       │   ├── vehicle_service.py
│       │   ├── simulation_service.py
│       │   └── comparison_service.py
│       ├── api/            # Couche API
│       │   ├── __init__.py
│       │   ├── deps.py     # Dépendances (DI)
│       │   └── v1/
│       │       ├── __init__.py
│       │       ├── vehicles.py
│       │       ├── simulations.py
│       │       └── comparisons.py
│       └── infrastructure/ # Couche infrastructure
│           ├── __init__.py
│           ├── database.py # Configuration DB
│           ├── models.py   # Modèles SQLAlchemy
│           └── repositories/
│               ├── __init__.py
│               ├── base.py
│               ├── vehicle_repo.py
│               ├── simulation_repo.py
│               └── comparison_repo.py
├── tests/
│   ├── __init__.py
│   ├── conftest.py         # Fixtures pytest
│   ├── unit/               # Tests unitaires
│   │   ├── __init__.py
│   │   └── test_calculator.py
│   ├── integration/        # Tests d'intégration
│   │   ├── __init__.py
│   │   └── test_api.py
│   └── features/           # Tests BDD (Behave)
│       ├── vehicle.feature
│       ├── simulation.feature
│       ├── comparison.feature
│       └── steps/
│           ├── __init__.py
│           └── common_steps.py
├── pyproject.toml          # Configuration projet
├── README.md               # Documentation + tâches xc
└── .gitignore
```

## 3. Principes architecturaux

### 3.1 Clean Architecture

- **Domain Layer** : Logique métier pure, sans dépendance externe
- **Application Layer** : Services orchestrant les cas d'utilisation
- **Infrastructure Layer** : Implémentations concrètes (DB, API externes)
- **API Layer** : Interface HTTP (FastAPI)

### 3.2 Dependency Injection

Utilisation de FastAPI Depends pour l'injection de dépendances :
- Facilite les tests (mocks)
- Découple les couches
- Permet le remplacement d'implémentations

### 3.3 Repository Pattern

- Interface abstraite pour l'accès aux données
- Implémentation SQLAlchemy concrète
- Testable avec des repositories in-memory

### 3.4 Généricité (selon règles projet)

- **Process génériques** : Repository base, Service base
- **Points d'extension** : Stratégies de calcul, sources de données
- **Registry pattern** : Pour les types de carburant, types d'achat

## 4. Outils de développement

### Task runner : xc

Les tâches sont définies directement dans le `README.md` au format Markdown.
Voir https://xcfile.dev pour la documentation complète.

```bash
xc              # Liste les tâches disponibles
xc setup        # Installe les dépendances
xc dev          # Lance le serveur de développement
xc test         # Lance tous les tests
xc lint         # Vérifie le code
```

### Gestionnaire de packages : uv

```bash
uv venv           # Créer l'environnement virtuel
uv add <package>  # Ajouter une dépendance
uv run <command>  # Exécuter une commande dans l'environnement
```

### Linter/Formatter : ruff

Configuration dans `pyproject.toml`.

## 5. Configuration

### Variables d'environnement

| Variable | Description | Défaut |
|----------|-------------|--------|
| DATABASE_URL | URL de connexion DB | sqlite:///./car_compare.db |
| DEBUG | Mode debug | false |
| API_PREFIX | Préfixe API | /api/v1 |

## 6. Sécurité (Future)

- Authentification JWT
- Rate limiting
- Validation des entrées (Pydantic)
- CORS configuré

## 7. Performance

- Pagination des listes
- Indexes sur les colonnes recherchées
- Cache Redis (future)
