# Guide utilisateur

## Azurz, pour la démo

### Démarrer le lab

Prérequis : Docker, et assez de mémoire pour trois passerelles Ignition (compter environ 3 Go).

```bash
docker compose -f lab/docker-compose.yml up --build
```

- Pont et écran de suivi : http://localhost:8080
- Azure DevOps du lab : http://localhost:8081 (organisation `azurz`, projet `AzurzDemo`). La page d'accueil demande un nom d'utilisateur et un mot de passe. Le mot de passe est le PAT `azurz-lab-pat`. Le nom d'utilisateur peut rester vide ou prendre n'importe quelle valeur, par exemple `azurz`. Après connexion, l'écran reprend les hubs du portail : éléments de travail (historique des champs, ancienne et nouvelle valeur, zone, itération, assigné, étiquettes, états du type et transitions depuis l'état courant, corbeille, discussion), sprints (une itération retirée qui laisse la voisine et l'itération courante, capacité et tâches), tableaux (colonne ajoutée avant la sortie, ligne ajoutée sous la ligne par défaut, limite visuelle), fichiers du dépôt (création, révision et suppression), commits (identifiant distinct même si les commentaires commencent pareil, statut CI publié sur le commit, dernier statut par contexte, cherry-pick et revert sur une branche), branches (une branche retirée qui laisse la voisine et la branche par défaut), étiquettes (création annotée et suppression), demandes de tirage (discussion résolue, commits de la source, itérations qui avancent avec la source ou la cible, fichiers de la dernière itération, fusion squash avec suppression de la source, fusion qui garde le fichier de la source quand la cible a avancé, rebase et fusion semi-linéaire, étiquettes partagées, relecteurs (un relecteur retiré qui laisse le voisin), complétion automatique qui attend un relecteur obligatoire, terminaison, work items liés), pipelines (un pipeline retiré qui laisse le voisin et ses exécutions, mise en file d'attente, déclencheur CI sur main, horaire du jeudi à minuit, groupe de variables lié, variables propres au pipeline qui recouvrent le groupe, paramètre de file, variables figées sur la build avec le secret masqué, file d'agents, chronologie, journal, artefact publié puis téléchargé), stratégies de branche (évaluation d'une demande de tirage), versions (une définition retirée qui laisse la voisine et ses versions, environnement, approbation et abandon), environnements de pipeline (un environnement retiré qui laisse le voisin et ses contrôles, approbation, contrôle de branche, verrou exclusif, déploiement qui attend l'approbation), bibliothèque (groupes de variables et fichiers sécurisés), connexions de service (secret masqué, mise à jour de l'URL), groupes de variables (secret masqué, ajout d'une variable, retrait), plans de test (un plan retiré qui laisse le voisin et ses suites, suite renommée, suite supprimée avec ses cas, exécution) et exécutions de test (une exécution retirée qui laisse la voisine et ses résultats), artefacts (un flux retiré qui laisse le voisin et ses paquets, publication Maven, PyPI, npm, paquet universel et NuGet, version retirée), demandes (une demande retirée qui laisse la voisine et son agent), fichiers sécurisés (un fichier sécurisé retiré qui laisse le voisin et son contenu masqué), paquets (un paquet retiré qui laisse le voisin et sa version), connexions (une connexion retirée qui laisse la voisine et son secret masqué), dépôts (un dépôt retiré qui laisse le voisin et son commit), wikis (un wiki retiré qui laisse le voisin et sa page), abonnements (un abonnement retiré qui laisse le voisin et sa notification), stratégies de branche (une stratégie retirée qui laisse la voisine et son évaluation), pipelines YAML (un pipeline YAML retiré qui laisse le voisin et son exécution), agents (un pool retiré qui laisse le voisin, son agent, sa demande et sa file, et refuse le pool par défaut, une file retirée qui laisse la voisine, le pool, son agent et la file de l'autre projet, et refuse la file par défaut, une étiquette de build retirée qui laisse la voisine et l'étiquette de l'autre build, une conservation de build retirée qui laisse la voisine et la conservation de l'autre build, une configuration de test retirée qui laisse la voisine et la configuration de l'autre projet, un groupe de tâches retiré qui laisse le voisin et le groupe de l'autre projet, un groupe de déploiement retiré qui laisse le voisin et le groupe de l'autre projet, une variable de test retirée qui laisse la voisine et la variable de l'autre projet, une étiquette de work item retirée qui laisse la voisine et l'étiquette de l'autre projet, un dossier de pipeline retiré qui laisse le voisin, le pipeline et le dossier de l'autre projet et emporte son enfant, une cible de déploiement retirée qui laisse la voisine, le groupe et la cible de l'autre projet, une vue de flux retirée qui laisse la voisine, les paquets et la vue de l'autre projet, un flux créé après une suppression qui laisse l'identifiant du flux encore présent, un dossier de version retiré qui laisse le voisin, la définition et le dossier de l'autre projet et emporte son enfant, un commentaire de page de wiki retiré qui laisse le voisin, la page et le commentaire de l'autre projet, un statut de demande de tirage retiré qui laisse le voisin, la demande et le statut de l'autre projet, un commentaire de commit retiré qui laisse le voisin, le commit et le commentaire de l'autre projet, une réaction de commentaire retirée qui laisse la voisine, le commentaire et la réaction de l'autre projet, une source amont retirée qui laisse la voisine, le flux et la source de l'autre projet, une permission de flux retirée qui laisse la voisine, le flux et la permission de l'autre projet, une politique de conservation de flux retirée qui laisse la voisine, le flux et la politique de l'autre projet, un paquet retiré de la corbeille d'un flux qui laisse le voisin, le paquet encore publié, le flux et la corbeille de l'autre projet, une notification retirée qui laisse la voisine, l'abonnement de service hook et la notification de l'autre projet, un favori retiré qui laisse le voisin, la notification et le favori de l'autre projet, un droit retiré qui laisse le voisin, le favori et le droit de l'autre projet, un groupe retiré qui laisse le voisin, le droit et le groupe de l'autre projet, une étagère retirée qui laisse la voisine, le groupe et l'étagère de l'autre projet, une étiquette TFVC retirée qui laisse la voisine, l'étagère et l'étiquette TFVC de l'autre projet, un plan de livraison retiré qui laisse le voisin, l'étiquette TFVC et le plan de livraison de l'autre projet, une liste de sélection retirée qui laisse la voisine, le plan de livraison et la liste de sélection de l'autre projet, un champ retiré qui laisse le voisin, la liste de sélection et le champ de l'autre projet, une appartenance retirée qui laisse la voisine, le champ et l'appartenance de l'autre projet, un flux d'audit retiré qui laisse le voisin, l'appartenance et le flux d'audit de l'autre projet, une extension retirée qui laisse la voisine, le flux d'audit et l'extension de l'autre projet, un droit de groupe retiré qui laisse le voisin, l'extension et le droit de groupe de l'autre projet, un processus hérité retiré qui laisse le voisin, le droit de groupe, le processus système et le processus de l'autre projet, un type retiré qui laisse le voisin, le processus hérité, le type système et le type de l'autre projet, un état retiré qui laisse le voisin, le type, l'état système et l'état de l'autre projet, une règle retirée qui laisse la voisine, l'état et la règle de l'autre projet, un champ de type retiré qui laisse le voisin, la règle et le champ de l'autre projet, un comportement retiré qui laisse le voisin, le champ et le comportement de l'autre projet, une page retirée qui laisse la voisine, le comportement et la page de l'autre projet, un groupe retiré qui laisse le voisin, la page et le groupe de l'autre projet, un contrôle retiré qui laisse le voisin, le groupe et le contrôle de l'autre projet, un comportement de type retiré qui laisse le voisin, le contrôle et le comportement de l'autre projet, un modèle retiré qui laisse le voisin, le comportement et le modèle de l'autre projet, une machine retirée qui laisse la voisine, le modèle et la machine de l'autre projet, un cluster retiré qui laisse le voisin, la machine et le cluster de l'autre projet, un paramètre partagé retiré qui laisse le voisin, le cluster et le paramètre de l'autre projet, une pièce d'exécution retirée qui laisse la voisine, le paramètre et la pièce de l'autre projet, une pièce de résultat retirée qui laisse la voisine, la pièce d'exécution et la pièce de l'autre projet, un bogue de résultat retiré qui laisse le voisin, la pièce de résultat et le bogue de l'autre projet, une itération de résultat retirée qui laisse la voisine, le bogue de résultat et l'itération de l'autre projet, une action de résultat retirée qui laisse la voisine, l'itération de résultat et l'action de l'autre projet, une pièce d'itération retirée qui laisse la voisine, l'action de résultat et la pièce de l'autre projet, un paramètre d'itération retiré qui laisse le voisin, la pièce d'itération et le paramètre de l'autre projet, une pièce d'action retirée qui laisse la voisine, le paramètre d'itération et la pièce de l'autre projet, un paramètre d'action retiré qui laisse le voisin, la pièce d'action et le paramètre de l'autre projet, un journal de résultat retiré qui laisse le voisin, le paramètre d'action et le journal de l'autre projet, un sous-résultat retiré qui laisse le voisin, le journal de résultat et le sous-résultat de l'autre projet, une pièce de sous-résultat retirée qui laisse la voisine, le sous-résultat et la pièce de l'autre projet, un agent retiré qui laisse le voisin et sa demande, enregistrement, mise en ligne, demande de travail), abonnements (statut désactivé silencieux, livraison à la création d'un work item, à une poussée git, à une demande de tirage créée, mise à jour ou fusionnée, à une page de wiki déplacée ou supprimée, à une suite de test supprimée, à une version abandonnée, à un tableau de bord renommé, à un paquet retiré, à une version npm retirée, à un groupe de variables retiré, et à la fin d'une build), tableau de bord (un tableau retiré qui laisse le voisin et ses widgets, nom révisé, widget retiré, Markdown, requête, build), requêtes (une requête retirée qui laisse la voisine et les dossiers racine, dossiers, WIQL enregistré et exécuté), équipes (membre retiré qui laisse le voisin, bugs au backlog, jours ouvrés), zones (création, renommage qui déplace les enfants, suppression), wiki (page déplacée avec ses sous-pages, page supprimée) et bibliothèque de variables dont un groupe se retire. Une poussée dont l'ancien identifiant de commit ne correspond plus est refusée. Un clic sur un commit affiche le diff de contenu avec la version parente. Un fichier peut être relu à un commit précis. Une build mise en file est lue ensuite comme terminée avec succès. Sa chronologie passe alors d'une étape en attente à une étape, une phase, un travail et une tâche réussis, avec le journal de la tâche. Un agent peut y écrire ses propres enregistrements.

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
