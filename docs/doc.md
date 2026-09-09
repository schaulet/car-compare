# Documentation Générale - Car Compare

## Présentation

**Car Compare** est un outil d'aide à la décision pour l'achat ou la location de véhicules. Il permet de comparer différentes voitures selon plusieurs critères (prix, consommation, coût total de possession, etc.) et de simuler les coûts sur différentes durées.

## Objectifs

1. **Comparer des véhicules** : Permettre la comparaison côte à côte de plusieurs voitures
2. **Calculer le TCO** (Total Cost of Ownership) : Estimer le coût total de possession incluant :
   - Prix d'achat ou loyer mensuel
   - Consommation de carburant/électricité
   - Assurance
   - Entretien
   - Dépréciation
3. **Simuler des scénarios** : Comparer achat vs location sur différentes durées
4. **Recommander** : Suggérer le meilleur choix selon les critères de l'utilisateur

## Fonctionnalités principales

### 1. Gestion des véhicules
- Ajout/modification/suppression de véhicules
- Base de données de véhicules avec caractéristiques techniques
- Import de données depuis des sources externes

### 2. Comparaison
- Comparaison multi-critères
- Visualisation des différences
- Scoring personnalisable

### 3. Simulation financière
- Calcul du coût mensuel
- Projection sur plusieurs années
- Comparaison achat/location/LOA

### 4. API REST
- Endpoints pour toutes les opérations CRUD
- Documentation OpenAPI/Swagger
- Authentification (future version)

## Stack technique

- **Langage** : Python 3.11+
- **Framework API** : FastAPI
- **Base de données** : SQLite (dev) / PostgreSQL (prod)
- **ORM** : SQLAlchemy
- **Tests** : pytest + behave (BDD)
- **Gestion de dépendances** : uv
