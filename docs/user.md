# Guide Utilisateur - Car Compare

## Introduction

Bienvenue dans **Car Compare**, votre assistant pour choisir le véhicule idéal. Cette application vous permet de comparer différentes voitures et de calculer leur coût total de possession.

## Démarrage rapide

### Prérequis

- Python 3.11 ou supérieur
- [uv](https://github.com/astral-sh/uv) (gestionnaire de paquets Python)
- [xc](https://xcfile.dev) (task runner, optionnel)

### Installation

```bash
# Cloner le projet
git clone <repository-url>
cd car-compare

# Installer les dépendances
xc setup
# ou manuellement : uv sync --all-extras

# Lancer l'application
xc dev
# ou manuellement : uv run uvicorn car_compare.main:app --reload
```

L'API est accessible sur `http://localhost:8000`

### Documentation interactive

- Swagger UI : `http://localhost:8000/docs`
- ReDoc : `http://localhost:8000/redoc`

## Commandes disponibles (xc)

| Commande | Description |
|----------|-------------|
| `xc setup` | Installer les dépendances |
| `xc dev` | Démarrer le serveur de développement |
| `xc test` | Lancer tous les tests |
| `xc test-unit` | Lancer les tests unitaires |
| `xc test-integration` | Lancer les tests d'intégration |
| `xc test-bdd` | Lancer les tests BDD |
| `xc lint` | Vérifier le code |
| `xc format` | Formater le code |
| `xc build` | Construire le package |
| `xc clean` | Nettoyer les fichiers générés |

> **Note** : xc est optionnel. Les commandes peuvent être exécutées manuellement en copiant les scripts depuis le README.md.

## Utilisation de l'API

### 1. Ajouter un véhicule

```bash
curl -X POST "http://localhost:8000/api/v1/vehicles" \
  -H "Content-Type: application/json" \
  -d '{
    "brand": "Renault",
    "model": "Clio",
    "version": "Intens",
    "year": 2024,
    "fuel_type": "essence",
    "price": 22000,
    "consumption": 5.5,
    "co2_emissions": 125,
    "power_hp": 100,
    "transmission": "manuelle",
    "doors": 5,
    "seats": 5,
    "trunk_volume": 391
  }'
```

### 2. Lister les véhicules

```bash
curl "http://localhost:8000/api/v1/vehicles"
```

### 3. Créer une simulation

```bash
curl -X POST "http://localhost:8000/api/v1/simulations" \
  -H "Content-Type: application/json" \
  -d '{
    "vehicle_id": "<vehicle-uuid>",
    "duration_months": 60,
    "annual_km": 15000,
    "fuel_price": 1.85,
    "insurance_monthly": 50,
    "maintenance_annual": 500,
    "purchase_type": "cash"
  }'
```

### 4. Comparer des véhicules

```bash
curl -X POST "http://localhost:8000/api/v1/comparisons" \
  -H "Content-Type: application/json" \
  -d '{
    "name": "Ma comparaison",
    "simulation_ids": ["<simulation-1-uuid>", "<simulation-2-uuid>"]
  }'
```

## Comprendre les résultats

### TCO (Total Cost of Ownership)

Le TCO représente le coût total de possession d'un véhicule sur une durée donnée. Il inclut :

- **Prix d'achat** : Le coût initial du véhicule
- **Carburant** : Basé sur votre kilométrage annuel et la consommation
- **Assurance** : Coût mensuel × durée
- **Entretien** : Révisions, pneus, réparations
- **Dépréciation** : Perte de valeur du véhicule

### Coût mensuel

Le coût mensuel est simplement le TCO divisé par le nombre de mois de possession.

### Comparaison

La comparaison affiche :
- Les caractéristiques côte à côte
- Les différences de coût
- Un score global basé sur vos critères

## Conseils d'utilisation

1. **Soyez réaliste** sur votre kilométrage annuel
2. **Comparez des véhicules similaires** (même segment)
3. **Incluez tous les coûts** dans vos estimations
4. **Considérez la revente** si vous changez souvent de voiture

## FAQ

### Q: Comment installer xc ?

Voir https://xcfile.dev/getting-started/#installation

### Q: xc est-il obligatoire ?

Non, les commandes peuvent être exécutées manuellement en copiant les scripts depuis le README.md.

### Q: Comment sont calculées les émissions CO2 ?

Les émissions CO2 sont celles déclarées par le constructeur selon le cycle WLTP.

### Q: Puis-je comparer plus de 2 véhicules ?

Oui, vous pouvez comparer autant de véhicules que vous le souhaitez.

### Q: Les prix sont-ils mis à jour automatiquement ?

Non, les prix sont saisis manuellement. Vérifiez les tarifs actuels auprès des concessionnaires.

## Support

Pour toute question ou suggestion, veuillez créer une issue sur le repository GitHub.
