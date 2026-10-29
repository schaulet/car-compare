# Journal de suivi

## 2026-10-29 — Azure DevOps en Rust pour Linux

- **Demande**: réimplémenter Azure DevOps en Rust, avec l'ensemble des API et une interface visuelle identique, pour tester des intégrations depuis Linux, puis l'utiliser dans le lab.
- **Actions**: exécutable `azdo` (connexion PAT, work items JSON Patch avec relations, pièces jointes, WIQL et requêtes enregistrées, commentaires révisés ou retirés, mises à jour de work item (ancienne et nouvelle valeur, liens ajoutés ou retirés), tableaux, leurs colonnes et leurs lignes, backlogs et capacité de sprint, dépôts git, fils de demande de tirage résolus ou rouverts et statuts, définitions dont une retirée laisse la voisine et ses exécutions, builds, chronologie (étape, phase, travail, tâche, journal), écriture de chronologie par l'agent, dépôts git dont un retiré laisse le voisin et son commit, wikis dont un retiré laisse le voisin et sa page, abonnements dont un retiré laisse le voisin et sa notification, stratégies de branche dont une retirée laisse la voisine et son évaluation, pipelines YAML dont un retiré laisse le voisin et son exécution, files d'agents dont un agent retiré laisse le voisin et sa demande, journaux et artefacts, exécutions de pipelines, plans de test dont un retiré laisse le voisin et ses suites, et dont une suite se renomme et disparaît avec ses cas, pages de wiki déplacées ou supprimées, flux dont un retiré laisse le voisin et ses paquets, tableaux de bord dont un retiré laisse le voisin et ses widgets, environnements dont un retiré laisse le voisin et ses contrôles, connexions de service, espaces de noms de sécurité, ACL et évaluation de permissions, changesets TFVC, recherche de work items et de code, index NuGet v3 et conteneur plat dont une version se retire, registre npm dont une version se retire, dépôt Maven (POM, maven-metadata.xml et paquet retiré), index simple PyPI dont un paquet se retire, paquets universels dont une version se retire et laisse la version voisine, agents de pool (enregistrement, mise en ligne, demande de travail), livraison des service hooks (création, mise à jour, suppression et restauration de work item, commentaire révisé, poussée git, demande de tirage créée, mise à jour, commentée ou fusionnée, page de wiki déplacée ou supprimée, suite de test renommée ou supprimée, version abandonnée, tableau de bord renommé ou widget retiré, paquet retiré d'un flux, version npm retirée, groupe de variables retiré, fin de build), tableaux de bord renommés dont un widget se retire, avec widgets Markdown, requête et build, requêtes WIQL dans les dossiers Shared Queries et My Queries, statuts de commit (historique et dernier statut par contexte), équipes (membre retiré qui laisse le voisin, bugs au backlog, jours ouvrés), liens d'artefact entre une demande de tirage et des work items, nœuds de classification (zones, itérations, renommage qui déplace les enfants), étiquettes git annotées et légères, commits d'une demande de tirage (ceux de la source absents de la cible), itérations de demande de tirage qui avancent quand la source ou la cible bouge, fichiers figés de chaque itération, groupes de variables, poussées git (commits, fichiers créés, révisés ou supprimés, conflit de référence, diff entre versions), relecteurs dont un retiré laisse le voisin, complétion de demande de tirage, suites, cas et points de plan de test, définitions de version dont une retirée laisse la voisine et ses versions, environnements et approbations, stratégies de branche (relecteurs, build, statut, commentaires) et leur évaluation sur une demande de tirage, exécutions de test avec résultats dont une retirée laisse la voisine et ses résultats, abonnements de service hooks dont un statut disabled reste silencieux, catalogue de types de work items, de leurs états New, Active, Resolved et Closed et des transitions entre ces états, contrôles d'environnement : approbation, contrôle de branche et verrou exclusif, approbation d'une exécution de pipeline qui attend ce contrôle, fichiers sécurisés de la bibliothèque, artefacts de build dont le contenu reste masqué jusqu'au téléchargement, complétion automatique d'une demande de tirage qui attend un relecteur obligatoire ou un vote négatif, connexions de service dont le secret reste masqué, groupes de variables dont le secret reste masqué tant qu'il n'est pas levé, une build qui reprend les variables du groupe lié à sa définition avec le secret masqué, une variable de définition qui recouvre ce groupe et un paramètre de file qui recouvre une variable seulement quand allowOverride est vrai, un déclencheur d'intégration continue, de demande de tirage et horaire du jeudi à minuit qui met une build en file). Le service `azdo` du lab Docker construit cette image. Les builds et les exécutions de pipeline passent à `completed` / `succeeded` à la lecture suivante, pour qu'un client qui interroge l'état termine. Quatre-vingt-huit tests de contrat Rust couvrent l'auth, un contrôle d'environnement (approbation, branche, verrou exclusif), une exécution qui attend l'approbation de l'environnement puis réussit ou échoue, un fichier sécurisé dont le contenu reste masqué jusqu'au téléchargement, un artefact de build publié puis téléchargé en zip, une complétion automatique qui attend le relecteur obligatoire puis fusionne, une connexion de service créée, mise à jour, et dont le mot de passe ne ressort pas, un groupe de variables dont le secret reste masqué puis réapparaît quand on lève le secret, une build qui fige les variables du groupe lié au moment de la file et garde le secret masqué, une variable de définition qui recouvre le groupe avec le secret masqué, et un paramètre de file refusé quand le JSON est illisible ou ignoré quand allowOverride est faux, un déclencheur CI qui met une build en file seulement pour la branche et le chemin retenus, et un déclencheur de demande de tirage qui ignore un brouillon, un horaire du jeudi à minuit qui met une seule build en file avec la raison schedule et laisse une définition désactivée silencieuse, des colonnes de tableau propres à une équipe, une ligne Urgent qui laisse l'autre équipe sur la ligne par défaut, une zone, une itération, un assigné et des étiquettes validés contre la classification, une suppression qui envoie l'élément dans la corbeille puis le restaure ou le détruit, un commentaire révisé puis retiré qui prévient workitem.commented, un fil de revue résolu puis un commentaire révisé qui prévient git.pullrequest.commented, une page de wiki déplacée avec ses sous-pages puis supprimée qui prévient wiki.page.moved et wiki.page.deleted, une suite de test renommée puis supprimée avec sa sous-suite qui prévient test.suite.deleted, une version abandonnée qui annule ses environnements ouverts et prévient release.abandoned, un fichier de dépôt révisé puis supprimé qui laisse le commit précédent intact, un tableau de bord renommé dont le widget retiré prévient dashboard.widget.deleted, un paquet Maven retiré du flux qui laisse la version voisine et prévient packaging.package.deleted, une version npm retirée qui laisse la version voisine, un groupe de variables retiré qui laisse le groupe voisin, conserve le lien sur la définition, garde les variables déjà copiées sur la build et prévient library.variablegroup.deleted avec le secret masqué, le cycle work item, une poussée git qui écrit les fichiers, une étiquette annotée qui pointe un commit puis se supprime, les commits d'une demande de tirage absents de la branche cible, les fichiers nets d'une itération figée, une itération suivante quand la source ou la cible avance, l'itération précédente qui conserve sa liste de fichiers, une fusion squash qui enregistre les fichiers nets et peut supprimer la branche source, une fusion qui garde les fichiers de la source sur le commit de fusion lorsque la cible a avancé, un rebase qui rejoue les commits sur la cible et une fusion semi-linéaire, un cherry-pick qui rejoue des commits choisis sur une branche nouvelle et un revert qui les défait, des étiquettes de demande de tirage partagées entre les demandes et retirables, des identifiants de commit distincts lorsque les commentaires partagent un préfixe, la livraison d'une demande de tirage créée, mise à jour puis fusionnée, la livraison d'un git.push et d'un build.complete, le diff entre deux commits et le contenu d'un fichier à une version, les statuts d'un commit, une équipe avec ses membres et ses réglages de backlog, les work items liés à une demande de tirage, un renommage de zone qui déplace les nœuds enfants, la capacité d'un sprint, la publication d'un paquet Maven et d'un paquet PyPI, un agent de pool qui passe en ligne et termine une demande de travail, la livraison d'un service hook au consommateur, un tableau de bord (Markdown, requête, build), une requête enregistrée dans un dossier puis retirée qui laisse la voisine et les dossiers racine, la fin d'une demande de tirage, les suites et points d'un plan de test, les définitions de version, leurs approbations et une version abandonnée, la chronologie d'une build (en attente puis réussie, avec une tâche écrite par l'agent), les stratégies de branche qui refusent puis acceptent une demande de tirage, un état canonique Active, un état inconnu refusé, un état En revue accepté pour la User Story mappée par la colonne et refusé pour le Bug, une transition d'En revue vers Bloqué refusée alors que chaque état mappé reste atteignable depuis Active, une version de paquet universel retirée qui laisse la version voisine et prévient packaging.package.deleted sans le contenu, une version NuGet retirée qui laisse la version voisine et prévient packaging.package.deleted sans le contenu, un abonnement désactivé qui reste silencieux pendant que l'abonnement voisin livre le work item, un membre d'équipe retiré qui laisse le voisin, une branche retirée qui laisse la voisine et la branche par défaut, une itération d'équipe retirée qui laisse la voisine et l'itération courante, une requête retirée qui laisse la voisine et les dossiers racine, une exécution de test retirée qui laisse la voisine et ses résultats, un relecteur retiré qui laisse le voisin, un pipeline retiré qui laisse le voisin et ses exécutions, un flux retiré qui laisse le voisin et ses paquets, une définition de version retirée qui laisse la voisine et ses versions, un plan de test retiré qui laisse le voisin et ses suites, un tableau de bord retiré qui laisse le voisin et ses widgets, un dépôt retiré qui laisse le voisin et son commit, un wiki retiré qui laisse le voisin et sa page, un abonnement retiré qui laisse le voisin et sa notification, une stratégie retirée qui laisse la voisine et son évaluation, un pipeline YAML retiré qui laisse le voisin et son exécution, un agent retiré qui laisse le voisin et sa demande, un environnement retiré qui laisse le voisin et ses contrôles, ces formes REST et la page de connexion.
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
