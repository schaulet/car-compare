# Documentation générale

## Azurz — pont Dust, Azure DevOps et Ignition

Azurz relie un chat Dust (dust.tt) à Azure DevOps et à des agents capables de produire un module Ignition, de le prouver par des tests Cucumber, puis de le déposer sur les passerelles choisies.

Le même processus sert le chat et l'interface web : analyser un message, créer ou enrichir **une** demande, la traiter quand elle est prête, publier un artefact versionné, le déployer vers une sélection d'instances.

### Ce que le lab montre

1. Le chat (ou l'écran « Chat » du pont, qui appelle les mêmes services que les outils MCP) décrit un module.
2. Azure DevOps reçoit un work item. Un second message enrichit **ce** ticket.
3. L'opérateur coche « Traitement automatique » ou clique « Traiter » sur la carte.
4. L'agent module propose une implémentation, écrit les tests Cucumber, les exécute et publie le rapport avec les preuves numériques.
5. L'opérateur choisit la version et les instances. L'agent de déploiement n'écrit le projet que là.

### Capacités livrées

| Capacité | Agent | Prérogatives exigées |
| --- | --- | --- |
| `ignition.module.build` | Générateur de modules Ignition | `workitem.read`, `artifact.publish`, `tests.run` |
| `artifact.deploy` | Déploiement des artefacts | `artifact.read`, `ignition.deploy` |

Un agent sans la prérogative exigée ne traite pas le ticket : la carte reste bloquée et le motif nomme la prérogative manquante.

### Où brancher Dust

Le serveur MCP est exposé en Streamable HTTP sur `http://<hôte>:8080/mcp` (le slash final est accepté aussi). Authentification : jeton Bearer `dust-lab-token` dans le lab. Dans Dust : Spaces → Tools → Add Tools, transport Streamable HTTP, jeton Bearer (pas le flux OAuth).

Outils : `analyser_demande`, `creer_demande_azure_devops`, `enrichir_demande`, `consulter_demande`, `lister_demandes`, `lister_artefacts`, `lister_instances_ignition`, `deployer_artefact`.

### Lab

`docker compose -f lab/docker-compose.yml up --build` démarre :

- un Azure DevOps de laboratoire en Rust (`azdo`), organisation `azurz`, projet `AzurzDemo`, PAT `azurz-lab-pat`, avec page de connexion, hubs du portail et contrôles d'environnement (approbation, branche, verrou) et approbations qui retiennent une exécution, fichiers sécurisés de la bibliothèque ;
- le pont (UI `http://localhost:8080`, API Azure DevOps du lab republished sur `http://localhost:8081`) ;
- trois passerelles Ignition 8.1 : Ligne A `:8088`, Ligne B `:8089`, Supervision `:8090` (compte `admin` / `azurz-lab`).

Le contenu exécutable déposé dans Ignition est un **projet de scripts** 8.1. Le paquet `.modl` est l'artefact téléchargeable. Ignition n'exécute un module binaire que s'il est signé ou compilé avec le SDK ; le lab ne prétend pas fournir cette signature. Le drapeau `allowunsignedmodules` est tout de même posé pour un futur module signé localement.

## Annexe — Car Compare

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
- **Task runner** : [xc](https://xcfile.dev)

## Utilisation des tâches xc

Les tâches du projet sont définies dans le `README.md` et peuvent être exécutées avec la commande `xc <nom_tâche>`.

Pour lister toutes les tâches disponibles :
```bash
xc
```

Pour exécuter une tâche :
```bash
xc setup    # Installer les dépendances
xc test     # Lancer les tests
xc dev      # Démarrer le développement
```
