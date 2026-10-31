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

- un Azure DevOps de laboratoire en Rust (`azdo`), organisation `azurz`, projet `AzurzDemo`, PAT `azurz-lab-pat`, avec page de connexion, hubs du portail et contrôles d'environnement (approbation, branche, verrou) et approbations qui retiennent une exécution, fichiers sécurisés de la bibliothèque, artefacts de build téléchargeables, complétion automatique des demandes de tirage, connexions de service, groupes de variables repris par une build avec le secret masqué puis retirés, variables de définition qui recouvrent le groupe, déclencheur CI et un horaire du jeudi à minuit qui mettent une build en file, colonnes et lignes de tableau modifiables, zone, itération, assigné, étiquettes, état et transition d'un work item, corbeille qui restaure ou détruit un élément supprimé, discussion qui révise ou retire un commentaire, fils de demande de tirage qui se résolvent, pages de wiki déplacées ou supprimées, suites de test renommées ou supprimées, versions abandonnées, fichiers de dépôt révisés ou supprimés, tableaux de bord renommés, paquets Maven, PyPI, npm, universels et NuGet retirés, abonnements désactivés, membre d'équipe retiré, branche retirée qui laisse la voisine, itération d'équipe retirée qui laisse la voisine, requête retirée qui laisse la voisine, exécution de test retirée qui laisse la voisine, relecteur retiré qui laisse le voisin, pipeline retiré qui laisse le voisin, flux retiré qui laisse le voisin et ses paquets, définition de version retirée qui laisse la voisine, plan de test retiré qui laisse le voisin, tableau de bord retiré qui laisse le voisin, pool retiré qui laisse le voisin, son agent, sa demande et sa file et refuse le pool par défaut, une file retirée qui laisse la voisine, le pool, son agent et la file de l'autre projet, et refuse la file par défaut, une étiquette de build retirée qui laisse la voisine et l'étiquette de l'autre build, une conservation de build retirée qui laisse la voisine et la conservation de l'autre build, une configuration de test retirée qui laisse la voisine et la configuration de l'autre projet, un groupe de tâches retiré qui laisse le voisin et le groupe de l'autre projet, un groupe de déploiement retiré qui laisse le voisin et le groupe de l'autre projet, une variable de test retirée qui laisse la voisine et la variable de l'autre projet, une étiquette de work item retirée qui laisse la voisine et l'étiquette de l'autre projet, un dossier de pipeline retiré qui laisse le voisin, le pipeline et le dossier de l'autre projet et emporte son enfant, une cible de déploiement retirée qui laisse la voisine, le groupe et la cible de l'autre projet, une vue de flux retirée qui laisse la voisine, les paquets et la vue de l'autre projet, un flux créé après une suppression qui laisse l'identifiant du flux encore présent, un dossier de version retiré qui laisse le voisin, la définition et le dossier de l'autre projet et emporte son enfant, un commentaire de page de wiki retiré qui laisse le voisin, la page et le commentaire de l'autre projet, un statut de demande de tirage retiré qui laisse le voisin, la demande et le statut de l'autre projet, un commentaire de commit retiré qui laisse le voisin, le commit et le commentaire de l'autre projet, une réaction de commentaire retirée qui laisse la voisine, le commentaire et la réaction de l'autre projet, une source amont retirée qui laisse la voisine, le flux et la source de l'autre projet, une permission de flux retirée qui laisse la voisine, le flux et la permission de l'autre projet, une politique de conservation de flux retirée qui laisse la voisine, le flux et la politique de l'autre projet, un paquet retiré de la corbeille d'un flux qui laisse le voisin, le paquet encore publié, le flux et la corbeille de l'autre projet, une notification retirée qui laisse la voisine, l'abonnement de service hook et la notification de l'autre projet, un favori retiré qui laisse le voisin, la notification et le favori de l'autre projet, un droit retiré qui laisse le voisin, le favori et le droit de l'autre projet, un groupe retiré qui laisse le voisin, le droit et le groupe de l'autre projet, une étagère retirée qui laisse la voisine, le groupe et l'étagère de l'autre projet, une étiquette TFVC retirée qui laisse la voisine, l'étagère et l'étiquette TFVC de l'autre projet, un plan de livraison retiré qui laisse le voisin, l'étiquette TFVC et le plan de livraison de l'autre projet, une liste de sélection retirée qui laisse la voisine, le plan de livraison et la liste de sélection de l'autre projet, un champ retiré qui laisse le voisin, la liste de sélection et le champ de l'autre projet, une appartenance retirée qui laisse la voisine, le champ et l'appartenance de l'autre projet, un flux d'audit retiré qui laisse le voisin, l'appartenance et le flux d'audit de l'autre projet, une extension retirée qui laisse la voisine, le flux d'audit et l'extension de l'autre projet, un droit de groupe retiré qui laisse le voisin, l'extension et le droit de groupe de l'autre projet, un processus hérité retiré qui laisse le voisin, le droit de groupe, le processus système et le processus de l'autre projet, un type retiré qui laisse le voisin, le processus hérité, le type système et le type de l'autre projet, un état retiré qui laisse le voisin, le type, l'état système et l'état de l'autre projet, une règle retirée qui laisse la voisine, l'état et la règle de l'autre projet, un champ de type retiré qui laisse le voisin, la règle et le champ de l'autre projet, un comportement retiré qui laisse le voisin, le champ et le comportement de l'autre projet, une page retirée qui laisse la voisine, le comportement et la page de l'autre projet, un groupe retiré qui laisse le voisin, la page et le groupe de l'autre projet, un contrôle retiré qui laisse le voisin, le groupe et le contrôle de l'autre projet, un comportement de type retiré qui laisse le voisin, le contrôle et le comportement de l'autre projet, un modèle retiré qui laisse le voisin, le comportement et le modèle de l'autre projet, une machine retirée qui laisse la voisine, le modèle et la machine de l'autre projet, un cluster retiré qui laisse le voisin, la machine et le cluster de l'autre projet, un paramètre partagé retiré qui laisse le voisin, le cluster et le paramètre de l'autre projet, une pièce d'exécution retirée qui laisse la voisine, le paramètre et la pièce de l'autre projet, une pièce de résultat retirée qui laisse la voisine, la pièce d'exécution et la pièce de l'autre projet, un bogue de résultat retiré qui laisse le voisin, la pièce de résultat et le bogue de l'autre projet, une itération de résultat retirée qui laisse la voisine, le bogue de résultat et l'itération de l'autre projet, une action de résultat retirée qui laisse la voisine, l'itération de résultat et l'action de l'autre projet, une pièce d'itération retirée qui laisse la voisine, l'action de résultat et la pièce de l'autre projet, un paramètre d'itération retiré qui laisse le voisin, la pièce d'itération et le paramètre de l'autre projet, une pièce d'action retirée qui laisse la voisine, le paramètre d'itération et la pièce de l'autre projet, un paramètre d'action retiré qui laisse le voisin, la pièce d'action et le paramètre de l'autre projet, un journal de résultat retiré qui laisse le voisin, le paramètre d'action et le journal de l'autre projet, un sous-résultat retiré qui laisse le voisin, le journal de résultat et le sous-résultat de l'autre projet, une pièce de sous-résultat retirée qui laisse la voisine, le sous-résultat et la pièce de l'autre projet, un lien de méthode retiré qui laisse le voisin, la pièce de sous-résultat et le lien de l'autre projet, un fichier du magasin retiré qui laisse le voisin, le lien de méthode et le fichier de l'autre projet, une étiquette de test retirée qui laisse la voisine, le fichier du magasin et l'étiquette de l'autre projet, une couverture retirée qui laisse la voisine, l'étiquette de test et la couverture de l'autre projet, un type d'échec retiré qui laisse le voisin, la couverture et le type de l'autre projet, une métadonnée retirée qui laisse la voisine, le type d'échec et la métadonnée de l'autre projet, un champ de test retiré qui laisse le voisin, la métadonnée et le champ de l'autre projet, un champ de résultat retiré qui laisse le voisin, le champ de test et le champ de l'autre projet, un élément de résultat retiré qui laisse le voisin, le champ de résultat et l'élément de l'autre projet, un cas de test retiré qui laisse le voisin, l'élément de résultat et le cas de l'autre projet, une pièce de wiki retirée qui laisse la voisine, le cas de test et la pièce de l'autre projet, un résultat de test retiré qui laisse le voisin, la pièce de wiki et le résultat de l'autre projet, une propriété de build retirée qui laisse la voisine, le résultat de test et la propriété de l'autre projet, une propriété de définition retirée qui laisse la voisine, la propriété de build et la propriété de l'autre projet, une étiquette de définition retirée qui laisse la voisine, la propriété de définition et l'étiquette de l'autre projet, une révision de définition retirée qui laisse la voisine, l'étiquette de définition et la révision de l'autre projet, une métrique de définition retirée qui laisse la voisine, la révision de définition et la métrique de l'autre projet, un changement de build retiré qui laisse le voisin, la métrique de définition et le changement de l'autre projet, un élément de build retiré qui laisse le voisin, le changement de build et l'élément de l'autre projet, une pièce de build retirée qui laisse le voisin, l'élément de build et la pièce de l'autre projet, une étape de build retirée qui laisse le voisin, la pièce de build et l'étape de l'autre projet, une phase de build retirée qui laisse le voisin, l'étape de build et la phase de l'autre projet, un travail de build retiré qui laisse le voisin, la phase de build et le travail de l'autre projet, une tâche de build retirée qui laisse le voisin, le travail de build et la tâche de l'autre projet, un jalon de build retiré qui laisse le voisin, la tâche de build et le jalon de l'autre projet, une anomalie de build retirée qui laisse le voisin, le jalon de build et l'anomalie de l'autre projet, une tentative de build retirée qui laisse le voisin, l'anomalie de build et la tentative de l'autre projet, un avertissement de build retiré qui laisse le voisin, la tentative de build et l'avertissement de l'autre projet, une annotation de build retirée qui laisse le voisin, l'avertissement de build et l'annotation de l'autre projet, une note de build retirée qui laisse le voisin, l'annotation de build et la note de l'autre projet, un signal de build retiré qui laisse le voisin, la note de build et le signal de l'autre projet, une trace de build retirée qui laisse le voisin, le signal de build et la trace de l'autre projet, un constat de build retiré qui laisse le voisin, la trace de build et le constat de l'autre projet, une mesure de build retirée qui laisse le voisin, le constat de build et la mesure de l'autre projet, un repère de build retiré qui laisse le voisin, la mesure de build et le repère de l'autre projet, une marque de build retirée qui laisse le voisin, le repère de build et la marque de l'autre projet, une empreinte de build retirée qui laisse le voisin, la marque de build et l'empreinte de l'autre projet, une preuve de build retirée qui laisse le voisin, l'empreinte de build et la preuve de l'autre projet, un échantillon de build retiré qui laisse le voisin, la preuve de build et l'échantillon de l'autre projet, une observation de build retirée qui laisse le voisin, l'échantillon de build et l'observation de l'autre projet, un indice de build retiré qui laisse le voisin, l'observation de build et l'indice de l'autre projet, un seuil de build retiré qui laisse le voisin, l'indice de build et le seuil de l'autre projet, une borne de build retirée qui laisse la voisine, le seuil de build et la borne de l'autre projet, un compteur de build retiré qui laisse le voisin, la borne de build et le compteur de l'autre projet, une capture de build retirée qui laisse la voisine, le compteur de build et la capture de l'autre projet, un relevé de build retiré qui laisse le voisin, la capture de build et le relevé de l'autre projet, un témoin de build retiré qui laisse le voisin, le relevé de build et le témoin de l'autre projet, un palier de build retiré qui laisse le voisin, le témoin de build et le palier de l'autre projet, une cadence de build retirée qui laisse la voisine, le palier de build et la cadence de l'autre projet, un intervalle de build retiré qui laisse le voisin, la cadence de build et l'intervalle de l'autre projet, un rythme de build retiré qui laisse le voisin, l'intervalle de build et le rythme de l'autre projet, un cycle de build retiré qui laisse le voisin, le rythme de build et le cycle de l'autre projet, une période de build retirée qui laisse la voisine, le cycle de build et la période de l'autre projet, une fenêtre de build retirée qui laisse la voisine, la période de build et la fenêtre de l'autre projet, un créneau de build retiré qui laisse le voisin, la fenêtre de build et le créneau de l'autre projet, une séance de build retirée qui laisse la voisine, le créneau de build et la séance de l'autre projet, demande retirée qui laisse la voisine et son agent, fichier sécurisé retiré qui laisse le voisin et son contenu masqué, paquet retiré qui laisse le voisin et sa version, connexion retirée qui laisse la voisine et son secret masqué, dépôt retiré qui laisse le voisin et son commit, wiki retiré qui laisse le voisin et sa page, abonnement retiré qui laisse le voisin et sa notification, stratégie retirée qui laisse la voisine et son évaluation, pipeline YAML retiré qui laisse le voisin et son exécution, agent retiré qui laisse le voisin, environnement retiré qui laisse le voisin, fusion git qui conserve les fichiers de la source ;
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
