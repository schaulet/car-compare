# Journal de suivi

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
