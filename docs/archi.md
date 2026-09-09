# Documentation technique - car-compare

## Architecture

```
car-compare/
├── README.md          # Documentation + tâches xc
├── pyproject.toml     # Configuration projet Python (uv)
├── src/
│   └── car_compare/   # Code source principal
├── tests/             # Tests unitaires et d'intégration
└── docs/              # Documentation
    ├── journal.md     # Journal des modifications
    ├── doc.md         # Documentation générale
    ├── spec.md        # Spécification fonctionnelle
    ├── archi.md       # Documentation technique (ce fichier)
    └── user.md        # Guide utilisateur
```

## Outils de développement

### Gestionnaire de packages : uv

```bash
uv venv           # Créer l'environnement virtuel
uv add <package>  # Ajouter une dépendance
uv run <command>  # Exécuter une commande dans l'environnement
```

### Task runner : xc

Les tâches sont définies directement dans le `README.md` au format Markdown.
Voir https://xcfile.dev pour la documentation complète.

### Linter/Formatter : ruff

Configuration dans `pyproject.toml`.

## Tests

Framework : pytest

```bash
xc test  # Exécute les tests via xc
```
