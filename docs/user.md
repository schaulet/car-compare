# Guide utilisateur

## Azurz, pour la démo

### Démarrer le lab

Prérequis : Docker, et assez de mémoire pour trois passerelles Ignition (compter environ 3 Go).

```bash
docker compose -f lab/docker-compose.yml up --build
```

- Pont et écran de suivi : http://localhost:8080
- Azure DevOps du lab : http://localhost:8081 (organisation `azurz`, projet `AzurzDemo`). La page d'accueil demande un nom d'utilisateur et un mot de passe. Le mot de passe est le PAT `azurz-lab-pat`. Le nom d'utilisateur peut rester vide ou prendre n'importe quelle valeur, par exemple `azurz`. Après connexion, l'écran reprend les hubs du portail : éléments de travail, sprints (capacité et tâches de l'itération), tableaux, fichiers du dépôt, commits (statut CI publié sur le commit, dernier statut par contexte), branches, étiquettes (création annotée et suppression), demandes de tirage (commits de la source, fichiers de l'itération, relecteurs, terminaison, work items liés), pipelines (mise en file d'attente, file d'agents, chronologie et journal), stratégies de branche (évaluation d'une demande de tirage), versions (définition, environnement et approbation), plans de test (suites, cas et exécution) et exécutions de test, artefacts (flux, publication Maven et PyPI), agents (enregistrement, mise en ligne, demande de travail), abonnements (livraison à la création d'un work item, à une poussée git et à la fin d'une build), tableau de bord (Markdown, requête, build), requêtes (dossiers, WIQL enregistré et exécuté), équipes (membres, bugs au backlog, jours ouvrés), zones (création, renommage qui déplace les enfants, suppression), wiki et bibliothèque de variables. Une poussée dont l'ancien identifiant de commit ne correspond plus est refusée. Un clic sur un commit affiche le diff de contenu avec la version parente. Un fichier peut être relu à un commit précis. Une build mise en file est lue ensuite comme terminée avec succès. Sa chronologie passe alors d'une étape en attente à une étape, une phase, un travail et une tâche réussis, avec le journal de la tâche. Un agent peut y écrire ses propres enregistrements.

```bash
curl -u azurz:azurz-lab-pat http://localhost:8081/azurz/_apis/projects
```

Sans identifiant, l'API répond encore `PAT refusé`. L'écran de connexion, lui, s'ouvre dans le navigateur.

Hors Docker :

```bash
cd azdo && cargo run
```
- Ignition Ligne A : http://localhost:8088
- Ignition Ligne B : http://localhost:8089
- Ignition Supervision : http://localhost:8090

Compte passerelle : `admin` / `azurz-lab`.

Si le pont répond et que la création de ticket expire, le noyau filtre le trafic entre conteneurs du même réseau. Sur l'hôte Linux :

```bash
sudo sysctl -w net.bridge.bridge-nf-call-iptables=0
sudo sysctl -w net.bridge.bridge-nf-call-ip6tables=0
```

Sans Docker, le pont seul garde les tickets en mémoire :

```bash
uv sync --extra dev
uv run azurz
```

### Scénario à montrer

1. Ouvrir http://localhost:8080. Cliquer **Exemple OEE**, puis **Envoyer**. La carte « OEE Ligne 1 » passe à « prête ».
2. Pour montrer l'itération : sélectionner la carte (**Enrichir celle-ci**) et ajouter par exemple `Ajoute la fonction rendement(bonnes, total) = bonnes / total et le critère rendement(90, 100) = 0.9`.
3. Soit cliquer **Traiter** sur la carte, soit cocher **Traitement automatique des demandes prêtes**.
4. La carte affiche la proposition et le rapport. `compute_oee(0.9, 0.8, 0.95) = 0.684` est la preuve du scénario nominal. Le lien **rapport** ouvre la page HTML, **télécharger** récupère le zip (module, tests, projet).
5. Cocher Ligne A et Supervision, puis **Déployer la sélection**. Ligne B ne reçoit rien.
6. Après redémarrage de la passerelle (fait par l'agent dans le lab), le projet `azurz_oee_ligne_1` est dans les données de la passerelle. Le concepteur Ignition le voit dans la bibliothèque de scripts.

Le script `lab/demo.sh` rejoue ce parcours contre le pont déjà démarré.

### Brancher Dust

Dans le workspace Dust : Spaces → Tools → Add Tools.

- URL : `http://<adresse joignable par Dust>:8080/mcp`
- Authentification : Bearer `dust-lab-token`
- Laisser l'agent appeler `creer_demande_azure_devops`, puis `enrichir_demande` avec l'identifiant renvoyé, puis `consulter_demande` pour lire la proposition et le rapport.

Dust doit pouvoir ouvrir cette URL. Sur un poste de démo, un tunnel ou une URL publique est nécessaire : le lab, lui, tourne en local.

### Forme d'une demande de module

```
Je veux un module Ignition nommé « OEE Ligne 1 » qui expose la fonction
compute_oee(availability, performance, quality) = availability * performance * quality
et un tag mémoire Plant/Ligne1/OEE.
```

Si le nom ou la fonction manque, la carte reste « à enrichir » et liste ce qu'il faut préciser. Le ticket existe déjà dans Azure DevOps.

### Ce que l'opérateur voit sur une carte

- l'état et la capacité ;
- les manques, tant que la demande n'est pas prête ;
- la proposition d'implémentation ;
- le rapport de tests, scénario par scénario, avec la preuve ;
- le bouton Traiter, seulement quand la demande est prête.

La case en haut s'applique à toutes les demandes prêtes. Elle ne déploie pas toute seule : le déploiement part du tableau d'artefacts, vers les cases cochées.

## Annexe — Car Compare

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
