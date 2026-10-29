# Journal de suivi

## 2026-10-29 — Azure DevOps en Rust pour Linux

- **Demande**: réimplémenter Azure DevOps en Rust, avec l'ensemble des API et une interface visuelle identique, pour tester des intégrations depuis Linux, puis l'utiliser dans le lab.
- **Actions**: exécutable `azdo` (connexion PAT, work items JSON Patch avec relations, pièces jointes, WIQL et requêtes enregistrées, commentaires, tableaux, backlogs et capacité de sprint, dépôts git, fils et statuts de demande de tirage, définitions, builds, chronologie (étape, phase, travail, tâche, journal), écriture de chronologie par l'agent, files d'agents, journaux et artefacts, exécutions de pipelines, plans et résultats de test, wiki, flux, tableaux de bord, connexions de service, espaces de noms de sécurité, ACL et évaluation de permissions, changesets TFVC, recherche de work items et de code, index NuGet v3 et conteneur plat, registre npm, dépôt Maven (POM et maven-metadata.xml), index simple PyPI, paquets universels, agents de pool (enregistrement, mise en ligne, demande de travail), livraison des service hooks (création et mise à jour de work item), tableaux de bord avec widgets Markdown, requête et build, requêtes WIQL dans les dossiers Shared Queries et My Queries, statuts de commit (historique et dernier statut par contexte), équipes (membres, bugs au backlog, jours ouvrés), liens d'artefact entre une demande de tirage et des work items, nœuds de classification (zones, itérations, renommage qui déplace les enfants), étiquettes git annotées et légères, groupes de variables, poussées git (commits, fichiers, conflit de référence, diff entre versions), relecteurs et complétion de demande de tirage, suites, cas et points de plan de test, définitions de version, environnements et approbations, stratégies de branche (relecteurs, build, statut, commentaires) et leur évaluation sur une demande de tirage, exécutions de test avec résultats, abonnements de service hooks, catalogue de types de work items). Le service `azdo` du lab Docker construit cette image. Les builds et les exécutions de pipeline passent à `completed` / `succeeded` à la lecture suivante, pour qu'un client qui interroge l'état termine. Vingt-neuf tests de contrat Rust couvrent l'auth, le cycle work item, une poussée git qui écrit les fichiers, une étiquette annotée qui pointe un commit puis se supprime, le diff entre deux commits et le contenu d'un fichier à une version, les statuts d'un commit, une équipe avec ses membres et ses réglages de backlog, les work items liés à une demande de tirage, un renommage de zone qui déplace les nœuds enfants, la capacité d'un sprint, la publication d'un paquet Maven et d'un paquet PyPI, un agent de pool qui passe en ligne et termine une demande de travail, la livraison d'un service hook au consommateur, un tableau de bord (Markdown, requête, build), une requête enregistrée dans un dossier, la fin d'une demande de tirage, les suites et points d'un plan de test, les définitions de version et leurs approbations, la chronologie d'une build (en attente puis réussie, avec une tâche écrite par l'agent), les stratégies de branche qui refusent puis acceptent une demande de tirage, ces formes REST et la page de connexion.
- **Reste à faire**: parité complète avec le catalogue REST Microsoft et avec chaque écran du portail. Les formes déjà présentes restent un sous-ensemble. Le but n'est pas encore atteint.

## 2026-10-28 — authentification du lab Azure DevOps

- **Demande**: Donner les identifiants des conteneurs et vérifier la configuration, le lab Azure DevOps répondant `{"detail":"PAT refusé"}`.
- **Constat**: le PAT configuré est `azurz-lab-pat`. Le contrôle n'acceptait que l'en-tête Basic encodé avec un nom d'utilisateur vide. Un nom saisi (navigateur, `curl -u azurz:…`) produisait ce refus alors que le mot de passe était le bon.
- **Action**: le lab accepte le PAT comme mot de passe, quel que soit le nom d'utilisateur, comme l'API Azure DevOps. Un mot de passe différent reste refusé.

## 2026-10-28

### Pont MCP Dust → Azure DevOps, agents Ignition, lab Docker
- **Demande**: Créer un serveur MCP utilisable depuis Dust pour analyser une demande de chat, produire et enrichir un ticket Azure DevOps, suivre les demandes dans une interface, les traiter manuellement ou via une case globale, générer un module Ignition (proposition, tests Cucumber, rapport, artefact) puis déployer la version choisie vers les instances sélectionnées. Fournir un lab Docker avec Azure DevOps préconfiguré et plusieurs passerelles Ignition.
- **Actions réalisées**:
  - Scénarios Cucumber du pont (demandes, traitement, déploiement, registre générique) avant l'implémentation
  - Hôte de traitement ignorant des métiers, analyseurs / agents / outils enregistrés
  - Agent module Ignition et agent de déploiement
  - API et interface web (cartes, case globale, artefacts, cibles)
  - Serveur MCP Streamable HTTP protégé par jeton Bearer, branchable dans Dust
  - Lab Azure DevOps compatible REST et composition Docker (3 passerelles Ignition 8.1)
  - Documentation : doc, spec, archi, user
- **Résultat**:
  - 26 tests pytest passants (Car Compare inclus), dont le wheel qui embarque Azurz et l'interface
  - 17 scénarios behave passants, 93 étapes, dont les parcours Azurz (demande, itération, traitement manuel, case globale, prérogative, API, déploiement ciblé, registre)
  - lint ruff OK sur le pont
  - rendu mobile revérifié (colonne unique, tableaux en blocs)
  - lab Docker démarré : Azure DevOps, pont, trois passerelles Ignition 8.1 en état RUNNING. `lab/demo.sh` crée le ticket 1 « OEE Ligne 1 », le passe à Resolved, publie l'artefact 1.0.0 et le déploie sur Ligne A et Supervision (projet `azurz_oee_ligne_1` présent, absent de Ligne B). Handshake MCP `initialize` en 200 avec le jeton, 401 sans jeton.
- **Limite connue**: un `.modl` non signé et sans classe Java du SDK n'est pas chargé comme module binaire par Ignition. Le lab dépose le projet de scripts (le contenu exécutable) dans `data/projects` et le paquet `.modl` dans les artefacts de l'instance. Le redémarrage de la passerelle est demandé par l'agent quand `AZURZ_RESTART_GATEWAYS=1`.

## 2026-09-09

### Ajout des commandes xc au README.md
- **Demande**: Ajouter des commandes xc dans le README.md dans une section `## Tasks`
- **Référence**: https://xcfile.dev
- **Actions réalisées**:
  - Ajout de la section `## Tasks` dans le README.md avec des tâches xc pour :
    - `setup` : Installation des dépendances avec uv
    - `dev` : Démarrage du serveur de développement FastAPI
    - `test` : Exécution de tous les tests (pytest + behave)
    - `test-unit` : Tests unitaires uniquement
    - `test-integration` : Tests d'intégration uniquement
    - `test-bdd` : Tests BDD avec Behave
    - `lint` : Vérification du code avec ruff
    - `format` : Formatage du code avec ruff
    - `build` : Construction du package
    - `clean` : Nettoyage des fichiers générés

### Initialisation complète du projet
- **Demande**: Création d'un outil d'aide à l'achat/location de voiture
- **Actions réalisées**:
  - Création de la structure de documentation (docs/)
    - `doc.md` : Documentation générale
    - `spec.md` : Spécifications fonctionnelles
    - `archi.md` : Architecture technique
    - `user.md` : Guide utilisateur
  - Initialisation du projet Python avec uv
  - Implémentation du domaine métier:
    - Modèles de données (Vehicle, Simulation, Comparison)
    - Calculateur TCO avec dépréciation
    - Énumérations (FuelType, TransmissionType, PurchaseType)
  - Implémentation de l'infrastructure:
    - Configuration SQLAlchemy async
    - Repositories (Vehicle, Simulation, Comparison)
    - Pattern Repository avec base générique
  - Création des services applicatifs
  - Développement de l'API REST FastAPI:
    - Endpoints véhicules (CRUD)
    - Endpoints simulations
    - Endpoints comparaisons
  - Tests:
    - Tests unitaires pytest (10 tests)
    - Tests d'intégration API (11 tests)
    - Tests BDD avec Behave (7 scénarios, 31 steps)
  - Qualité de code: Lint avec ruff (100% passant)
- **Résultat**: 
  - ✅ 21 tests pytest passants
  - ✅ 7 scénarios BDD passants
  - ✅ Lint 100% OK
- **Prochaines étapes potentielles**:
  - Interface utilisateur (CLI ou Web)
  - Authentification API
  - Import de données véhicules

---
