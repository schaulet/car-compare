# Guide utilisateur - car-compare

## Installation

### Prérequis

- Python 3.11+
- [uv](https://github.com/astral-sh/uv) (gestionnaire de packages Python)
- [xc](https://xcfile.dev) (task runner, optionnel)

### Installation des dépendances

```bash
xc setup
# ou manuellement :
uv venv && uv sync
```

## Utilisation

### Démarrer l'application

```bash
xc dev
```

### Commandes disponibles

| Commande | Description |
|----------|-------------|
| `xc setup` | Installer les dépendances |
| `xc dev` | Démarrer le serveur de développement |
| `xc test` | Lancer les tests |
| `xc lint` | Vérifier le code |
| `xc format` | Formater le code |
| `xc build` | Construire l'application |

## FAQ

**Q: Comment installer xc ?**  
R: Voir https://xcfile.dev/getting-started/#installation

**Q: xc est-il obligatoire ?**  
R: Non, les commandes peuvent être exécutées manuellement en copiant les scripts depuis le README.md.
