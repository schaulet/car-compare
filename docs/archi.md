# Architecture technique

## Azurz

```
Chat Dust ──MCP Streamable HTTP /mcp──┐
UI web ──── HTTP /api ───────────────┤
                                      ▼
                         Plateforme (intake, tick, deploy)
                                      │
          ┌───────────────────────────┼───────────────────────────┐
          ▼                           ▼                           ▼
   Registre d'analyseurs      Hôte de traitement            Ports
   (score, pas de switch)     select_agents + politiques     work items
          │                           │                      artefacts
          │                           ▼                      passerelles
          │                    Agent enregistré
          ▼                     ignition-module
   IgnitionModuleAnalyzer       deploiement
   FallbackAnalyzer             (et tout agent ajouté)
```

L'hôte `azurz.orchestration` ne cite aucun métier. Une capacité nouvelle s'ajoute en enregistrant un analyseur, une politique de prérogatives, un agent et, si le chat doit la voir, une contribution d'outil MCP. Le montage MCP parcourt le catalogue : il ne liste pas les outils un par un.

### Contrats

- `WorkItemPort` : mémoire, ou client REST Azure DevOps (`application/json-patch+json`, PAT Basic). Le lab implémente le sous-ensemble projets, work items, WIQL, commentaires.
- `ArtifactPort` : répertoire versionné (sources, `module.xml`, `.modl`, `rapport.html`, projet gateway).
- `GatewayPort` : copie `gateway_project/` vers `{racine}/{instance}/projects/{projet}` et le `.modl` vers `azurz-artifacts/`. Option de redémarrage Docker si l'instance a un nom de conteneur.

### Processus génériques

| Processus | Hôte | Spécialisation |
| --- | --- | --- |
| Analyse | `AnalyzerRegistry` | `IgnitionModuleAnalyzer`, repli |
| Éligibilité | `select_agents` + `PolicyRegistry` | descripteurs d'agents |
| Exécution | `Platform.trigger` | `execute` de l'agent |
| Outils chat | `ToolCatalog` → `MCPServer.add_tool` | demandes, livraison |
| Persistance ticket | `WorkItemPort` | mémoire ou Azure DevOps |

Le domaine (expressions, analyse, sélection) ne fait pas d'I/O. Les tests Cucumber du produit pilotent `Platform` en mémoire. Les tests générés pour chaque module sont un second cycle Cucumber, lancé dans un sous-processus, et voyagent avec l'artefact.

### Interface

FastAPI sert `/`, `/static` et `/api`. Le serveur MCP est monté sur `/mcp`. Les deux partagent la plateforme. Un worker appelle `tick` toutes les deux secondes lorsque le traitement automatique est armé.

Le projet Python publie deux modules dans le même wheel (`car_compare` et `azurz`). Les fichiers de l'interface (`web/static`) voyagent avec le module, y compris dans l'image du lab installée sans mode éditable.

### Lab Docker

`lab/docker-compose.yml` : services `azdo`, `bridge`, `ignition-ligne-a`, `ignition-ligne-b`, `ignition-supervision`. Les volumes de données Ignition sont les mêmes que la racine lue par le pont, donc le dépôt de projet est visible par la passerelle après redémarrage.

Le service `azdo` est l'exécutable Rust `azdo` (`azdo/`). Il parle le sous-ensemble REST 7.1 utilisé pour tester une intégration : projets, work items en JSON Patch (champs, zone, itération, assigné, étiquettes validés contre la classification, corbeille avec restauration ou destruction, commentaires révisés ou retirés, relations, pièces jointes, mises à jour avec ancienne et nouvelle valeur), WIQL, requêtes enregistrées, commentaires révisés ou retirés, révisions, types, leurs états, leurs transitions et champs, nœuds de classification (zones et itérations, création, renommage qui réécrit le chemin des enfants, suppression), tableaux, leurs colonnes et leurs lignes, backlogs, équipes (membre retiré qui laisse le voisin, comportement des bugs, jours ouvrés) et capacité de sprint (une itération retirée qui laisse la voisine et l'itération courante, heures par jour, absences, jours off de l'équipe), dépôts git (branches dont une retirée laisse la voisine et la branche par défaut, étiquettes annotées et légères, commits dont l'identifiant dépend de tout le commentaire, poussées qui créent, révisent ou suppriment les fichiers, diff entre deux versions, lecture d'un fichier à un commit, statuts de commit avec historique et dernier statut par contexte, demandes de tirage, relecteurs dont un retiré laisse le voisin, complétion, fils, statuts, commits de la source absents de la cible, fichiers d'une itération figée et itération suivante quand la source ou la cible avance, fusion squash qui enregistre les fichiers nets et peut supprimer la branche source, fusion qui pose les fichiers de la source sur le commit lorsque la cible a avancé, rebase qui rejoue les commits sur la cible, fusion semi-linéaire, cherry-pick qui rejoue des commits sur une branche nouvelle, revert qui défait ces commits, étiquettes de demande de tirage, complétion automatique qui attend un relecteur obligatoire ou un vote négatif, work items liés par ArtifactLink), définitions, builds, chronologie (étape, phase, travail, tâche et journal, remplaçable par l'agent), files et pools d'agents dont un pool retiré laisse le voisin, son agent, sa demande et sa file, et refuse le pool par défaut, une file retirée qui laisse la voisine, le pool, son agent et la file de l'autre projet, et refuse la file par défaut, une étiquette de build retirée qui laisse la voisine et l'étiquette de l'autre build, une conservation de build retirée qui laisse la voisine et la conservation de l'autre build, une configuration de test retirée qui laisse la voisine et la configuration de l'autre projet, un groupe de tâches retiré qui laisse le voisin et le groupe de l'autre projet, un groupe de déploiement retiré qui laisse le voisin et le groupe de l'autre projet, une variable de test retirée qui laisse la voisine et la variable de l'autre projet, une étiquette de work item retirée qui laisse la voisine et l'étiquette de l'autre projet, un dossier de pipeline retiré qui laisse le voisin, le pipeline et le dossier de l'autre projet et emporte son enfant, une cible de déploiement retirée qui laisse la voisine, le groupe et la cible de l'autre projet, une vue de flux retirée qui laisse la voisine, les paquets et la vue de l'autre projet, un flux créé après une suppression qui laisse l'identifiant du flux encore présent, un dossier de version retiré qui laisse le voisin, la définition et le dossier de l'autre projet et emporte son enfant, un commentaire de page de wiki retiré qui laisse le voisin, la page et le commentaire de l'autre projet, un statut de demande de tirage retiré qui laisse le voisin, la demande et le statut de l'autre projet, un commentaire de commit retiré qui laisse le voisin, le commit et le commentaire de l'autre projet, une réaction de commentaire retirée qui laisse la voisine, le commentaire et la réaction de l'autre projet, une source amont retirée qui laisse la voisine, le flux et la source de l'autre projet, une permission de flux retirée qui laisse la voisine, le flux et la permission de l'autre projet, une politique de conservation de flux retirée qui laisse la voisine, le flux et la politique de l'autre projet, un paquet retiré de la corbeille d'un flux qui laisse le voisin, le paquet encore publié, le flux et la corbeille de l'autre projet, une notification retirée qui laisse la voisine, l'abonnement de service hook et la notification de l'autre projet, un favori retiré qui laisse le voisin, la notification et le favori de l'autre projet, un droit retiré qui laisse le voisin, le favori et le droit de l'autre projet, un groupe retiré qui laisse le voisin, le droit et le groupe de l'autre projet, une étagère retirée qui laisse la voisine, le groupe et l'étagère de l'autre projet, une étiquette TFVC retirée qui laisse la voisine, l'étagère et l'étiquette TFVC de l'autre projet, journaux et artefacts de build (contenu masqué jusqu'au téléchargement), pipelines dont un retiré laisse le voisin et ses exécutions, définitions de version dont une retirée laisse la voisine et ses versions, avec environnements, approbations et abandon, environnements dont un retiré laisse le voisin et ses contrôles, contrôles d'environnement (approbation, contrôle de branche, verrou exclusif), approbations de pipeline qui retiennent l'exécution, fichiers sécurisés (dépôt, lecture masquée, téléchargement), stratégies de branche et leur évaluation sur une demande de tirage, plans de test (un plan retiré qui laisse le voisin et ses suites, suite renommée, suite supprimée avec ses cas, points et résultats, une exécution retirée qui laisse la voisine et ses résultats), pages de wiki déplacées ou supprimées, flux d'artefacts dont un retiré laisse le voisin et ses paquets, un paquet Maven, PyPI ou une version npm se retire, tableaux de bord dont un retiré laisse le voisin et ses widgets, et dont un widget se retire, connexions de service (catalogue de types, secret masqué, mise à jour), groupes de variables (secret masqué, mise à jour, révélation, reprise par une build au moment de la file, recouverts par les variables de la définition, puis par un paramètre de file si allowOverride est vrai, retrait qui prévient library.variablegroup.deleted avec le secret masqué), espaces de noms de sécurité, ACL et évaluation de permissions, changesets TFVC, recherche de code et de work items, index NuGet v3, conteneur plat NuGet dont une version se retire, registre npm dont une version se retire, dépôt Maven (POM et maven-metadata.xml), index simple PyPI, paquets universels dont une version se retire, agents de pool (enregistrement, statut, demandes de travail, un pool retiré qui laisse le voisin, son agent, sa demande et sa file, et refuse le pool par défaut, une file retirée qui laisse la voisine, le pool, son agent et la file de l'autre projet, et refuse la file par défaut, une étiquette de build retirée qui laisse la voisine et l'étiquette de l'autre build, une conservation de build retirée qui laisse la voisine et la conservation de l'autre build, une configuration de test retirée qui laisse la voisine et la configuration de l'autre projet, un groupe de tâches retiré qui laisse le voisin et le groupe de l'autre projet, un groupe de déploiement retiré qui laisse le voisin et le groupe de l'autre projet, une variable de test retirée qui laisse la voisine et la variable de l'autre projet, une étiquette de work item retirée qui laisse la voisine et l'étiquette de l'autre projet, un dossier de pipeline retiré qui laisse le voisin, le pipeline et le dossier de l'autre projet et emporte son enfant, une cible de déploiement retirée qui laisse la voisine, le groupe et la cible de l'autre projet, une vue de flux retirée qui laisse la voisine, les paquets et la vue de l'autre projet, un flux créé après une suppression qui laisse l'identifiant du flux encore présent, un dossier de version retiré qui laisse le voisin, la définition et le dossier de l'autre projet et emporte son enfant, un commentaire de page de wiki retiré qui laisse le voisin, la page et le commentaire de l'autre projet, un statut de demande de tirage retiré qui laisse le voisin, la demande et le statut de l'autre projet, un commentaire de commit retiré qui laisse le voisin, le commit et le commentaire de l'autre projet, une réaction de commentaire retirée qui laisse la voisine, le commentaire et la réaction de l'autre projet, une source amont retirée qui laisse la voisine, le flux et la source de l'autre projet, une permission de flux retirée qui laisse la voisine, le flux et la permission de l'autre projet, une politique de conservation de flux retirée qui laisse la voisine, le flux et la politique de l'autre projet, un paquet retiré de la corbeille d'un flux qui laisse le voisin, le paquet encore publié, le flux et la corbeille de l'autre projet, une notification retirée qui laisse la voisine, l'abonnement de service hook et la notification de l'autre projet, un favori retiré qui laisse le voisin, la notification et le favori de l'autre projet, un droit retiré qui laisse le voisin, le favori et le droit de l'autre projet, un groupe retiré qui laisse le voisin, le droit et le groupe de l'autre projet, une étagère retirée qui laisse la voisine, le groupe et l'étagère de l'autre projet, une étiquette TFVC retirée qui laisse la voisine, l'étagère et l'étiquette TFVC de l'autre projet), abonnements de service hooks dont un statut disabled reste silencieux, livrés au consommateur à la création et à la mise à jour d'un work item, à une poussée git, à la création, la mise à jour et la fusion d'une demande de tirage, et à la fin d'une build, tableaux de bord et widgets (Markdown, compte de work items, dernière build), requêtes WIQL en dossiers (Shared Queries, My Queries, une requête retirée qui laisse la voisine, un dossier retiré qui emporte ses enfants), mise à jour et suppression, groupes de variables, versions, abonnements `hooks`. Les autres zones restent un magasin JSON générique. Une build ou une exécution de pipeline créée en `notStarted` / `inProgress` est lue ensuite comme `completed` / `succeeded`. L'interface reprend les hubs du portail (connexion par nom d'utilisateur et PAT, recherche sur la vue d'ensemble, fiche d'élément dont les transitions viennent du type, avec discussion et historique des champs, tableau par glisser-déposer, fichiers du dépôt, commits (cherry-pick et revert) et leurs statuts, branches, étiquettes, poussées, TFVC, discussion de demande de tirage résolue, avec relecteurs dont un retiré laisse le voisin, étiquettes, commits, itérations, fusion squash, fusion qui conserve les fichiers de la source, rebase, complétion automatique et work items liés, journal, chronologie, variables du groupe lié, variables de définition, déclencheurs CI, de demande de tirage et horaire, et artefacts de pipeline, stratégies de branche, plans de test et exécutions, flux d'artefacts avec publication Maven, PyPI et npm et retrait d'une version, demandes de travail dont une retirée laisse la voisine et son agent, fichiers sécurisés dont un retiré laisse le voisin et son contenu masqué, paquets d'un flux dont un retiré laisse le voisin et sa version, connexions de service dont une retirée laisse la voisine et son secret masqué, dépôts git dont un retiré laisse le voisin et son commit, wikis dont un retiré laisse le voisin et sa page, abonnements dont un retiré laisse le voisin et sa notification, stratégies de branche dont une retirée laisse la voisine et son évaluation, pipelines YAML dont un retiré laisse le voisin et son exécution, agents du pool dont un retiré laisse le voisin et sa demande, abonnements de service hooks, tableau de bord à widgets, requêtes WIQL en dossiers, équipes du projet, zones et itérations, éditeur de wiki qui déplace ou supprime une page, écran des versions qui abandonne une version, éditeur de fichier du dépôt, écran du tableau de bord qui renomme le tableau et retire un widget, écran des artefacts qui retire un paquet, une version npm ou une version NuGet, bibliothèque qui retire un groupe de variables). Le binaire persiste dans `AZDO_DATA`. L'objectif reste la couverture complète des API publiées et la parité visuelle avec le portail Microsoft.

## Annexe — Car Compare

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
