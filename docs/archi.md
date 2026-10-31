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

Le service `azdo` est l'exécutable Rust `azdo` (`azdo/`). Il parle le sous-ensemble REST 7.1 utilisé pour tester une intégration : projets, work items en JSON Patch (champs, zone, itération, assigné, étiquettes validés contre la classification, corbeille avec restauration ou destruction, commentaires révisés ou retirés, relations, pièces jointes, mises à jour avec ancienne et nouvelle valeur), WIQL, requêtes enregistrées, commentaires révisés ou retirés, révisions, types, leurs états, leurs transitions et champs, nœuds de classification (zones et itérations, création, renommage qui réécrit le chemin des enfants, suppression), tableaux, leurs colonnes et leurs lignes, backlogs, équipes (membre retiré qui laisse le voisin, comportement des bugs, jours ouvrés) et capacité de sprint (une itération retirée qui laisse la voisine et l'itération courante, heures par jour, absences, jours off de l'équipe), dépôts git (branches dont une retirée laisse la voisine et la branche par défaut, étiquettes annotées et légères, commits dont l'identifiant dépend de tout le commentaire, poussées qui créent, révisent ou suppriment les fichiers, diff entre deux versions, lecture d'un fichier à un commit, statuts de commit avec historique et dernier statut par contexte, demandes de tirage, relecteurs dont un retiré laisse le voisin, complétion, fils, statuts, commits de la source absents de la cible, fichiers d'une itération figée et itération suivante quand la source ou la cible avance, fusion squash qui enregistre les fichiers nets et peut supprimer la branche source, fusion qui pose les fichiers de la source sur le commit lorsque la cible a avancé, rebase qui rejoue les commits sur la cible, fusion semi-linéaire, cherry-pick qui rejoue des commits sur une branche nouvelle, revert qui défait ces commits, étiquettes de demande de tirage, complétion automatique qui attend un relecteur obligatoire ou un vote négatif, work items liés par ArtifactLink), définitions, builds, chronologie (étape, phase, travail, tâche et journal, remplaçable par l'agent), files et pools d'agents dont un pool retiré laisse le voisin, son agent, sa demande et sa file, et refuse le pool par défaut, une file retirée qui laisse la voisine, le pool, son agent et la file de l'autre projet, et refuse la file par défaut, une étiquette de build retirée qui laisse la voisine et l'étiquette de l'autre build, une conservation de build retirée qui laisse la voisine et la conservation de l'autre build, une configuration de test retirée qui laisse la voisine et la configuration de l'autre projet, un groupe de tâches retiré qui laisse le voisin et le groupe de l'autre projet, un groupe de déploiement retiré qui laisse le voisin et le groupe de l'autre projet, une variable de test retirée qui laisse la voisine et la variable de l'autre projet, une étiquette de work item retirée qui laisse la voisine et l'étiquette de l'autre projet, un dossier de pipeline retiré qui laisse le voisin, le pipeline et le dossier de l'autre projet et emporte son enfant, une cible de déploiement retirée qui laisse la voisine, le groupe et la cible de l'autre projet, une vue de flux retirée qui laisse la voisine, les paquets et la vue de l'autre projet, un flux créé après une suppression qui laisse l'identifiant du flux encore présent, un dossier de version retiré qui laisse le voisin, la définition et le dossier de l'autre projet et emporte son enfant, un commentaire de page de wiki retiré qui laisse le voisin, la page et le commentaire de l'autre projet, un statut de demande de tirage retiré qui laisse le voisin, la demande et le statut de l'autre projet, un commentaire de commit retiré qui laisse le voisin, le commit et le commentaire de l'autre projet, une réaction de commentaire retirée qui laisse la voisine, le commentaire et la réaction de l'autre projet, une source amont retirée qui laisse la voisine, le flux et la source de l'autre projet, une permission de flux retirée qui laisse la voisine, le flux et la permission de l'autre projet, une politique de conservation de flux retirée qui laisse la voisine, le flux et la politique de l'autre projet, un paquet retiré de la corbeille d'un flux qui laisse le voisin, le paquet encore publié, le flux et la corbeille de l'autre projet, une notification retirée qui laisse la voisine, l'abonnement de service hook et la notification de l'autre projet, un favori retiré qui laisse le voisin, la notification et le favori de l'autre projet, un droit retiré qui laisse le voisin, le favori et le droit de l'autre projet, un groupe retiré qui laisse le voisin, le droit et le groupe de l'autre projet, une étagère retirée qui laisse la voisine, le groupe et l'étagère de l'autre projet, une étiquette TFVC retirée qui laisse la voisine, l'étagère et l'étiquette TFVC de l'autre projet, un plan de livraison retiré qui laisse le voisin, l'étiquette TFVC et le plan de livraison de l'autre projet, une liste de sélection retirée qui laisse la voisine, le plan de livraison et la liste de sélection de l'autre projet, un champ retiré qui laisse le voisin, la liste de sélection et le champ de l'autre projet, une appartenance retirée qui laisse la voisine, le champ et l'appartenance de l'autre projet, un flux d'audit retiré qui laisse le voisin, l'appartenance et le flux d'audit de l'autre projet, une extension retirée qui laisse la voisine, le flux d'audit et l'extension de l'autre projet, un droit de groupe retiré qui laisse le voisin, l'extension et le droit de groupe de l'autre projet, un processus hérité retiré qui laisse le voisin, le droit de groupe, le processus système et le processus de l'autre projet, un type retiré qui laisse le voisin, le processus hérité, le type système et le type de l'autre projet, un état retiré qui laisse le voisin, le type, l'état système et l'état de l'autre projet, une règle retirée qui laisse la voisine, l'état et la règle de l'autre projet, un champ de type retiré qui laisse le voisin, la règle et le champ de l'autre projet, un comportement retiré qui laisse le voisin, le champ et le comportement de l'autre projet, une page retirée qui laisse la voisine, le comportement et la page de l'autre projet, un groupe retiré qui laisse le voisin, la page et le groupe de l'autre projet, un contrôle retiré qui laisse le voisin, le groupe et le contrôle de l'autre projet, un comportement de type retiré qui laisse le voisin, le contrôle et le comportement de l'autre projet, un modèle retiré qui laisse le voisin, le comportement et le modèle de l'autre projet, une machine retirée qui laisse la voisine, le modèle et la machine de l'autre projet, un cluster retiré qui laisse le voisin, la machine et le cluster de l'autre projet, un paramètre partagé retiré qui laisse le voisin, le cluster et le paramètre de l'autre projet, une pièce d'exécution retirée qui laisse la voisine, le paramètre et la pièce de l'autre projet, une pièce de résultat retirée qui laisse la voisine, la pièce d'exécution et la pièce de l'autre projet, un bogue de résultat retiré qui laisse le voisin, la pièce de résultat et le bogue de l'autre projet, une itération de résultat retirée qui laisse la voisine, le bogue de résultat et l'itération de l'autre projet, une action de résultat retirée qui laisse la voisine, l'itération de résultat et l'action de l'autre projet, une pièce d'itération retirée qui laisse la voisine, l'action de résultat et la pièce de l'autre projet, un paramètre d'itération retiré qui laisse le voisin, la pièce d'itération et le paramètre de l'autre projet, une pièce d'action retirée qui laisse la voisine, le paramètre d'itération et la pièce de l'autre projet, un paramètre d'action retiré qui laisse le voisin, la pièce d'action et le paramètre de l'autre projet, un journal de résultat retiré qui laisse le voisin, le paramètre d'action et le journal de l'autre projet, un sous-résultat retiré qui laisse le voisin, le journal de résultat et le sous-résultat de l'autre projet, une pièce de sous-résultat retirée qui laisse la voisine, le sous-résultat et la pièce de l'autre projet, un lien de méthode retiré qui laisse le voisin, la pièce de sous-résultat et le lien de l'autre projet, un fichier du magasin retiré qui laisse le voisin, le lien de méthode et le fichier de l'autre projet, une étiquette de test retirée qui laisse la voisine, le fichier du magasin et l'étiquette de l'autre projet, une couverture retirée qui laisse la voisine, l'étiquette de test et la couverture de l'autre projet, un type d'échec retiré qui laisse le voisin, la couverture et le type de l'autre projet, une métadonnée retirée qui laisse la voisine, le type d'échec et la métadonnée de l'autre projet, un champ de test retiré qui laisse le voisin, la métadonnée et le champ de l'autre projet, un champ de résultat retiré qui laisse le voisin, le champ de test et le champ de l'autre projet, un élément de résultat retiré qui laisse le voisin, le champ de résultat et l'élément de l'autre projet, un cas de test retiré qui laisse le voisin, l'élément de résultat et le cas de l'autre projet, une pièce de wiki retirée qui laisse la voisine, le cas de test et la pièce de l'autre projet, un résultat de test retiré qui laisse le voisin, la pièce de wiki et le résultat de l'autre projet, une propriété de build retirée qui laisse la voisine, le résultat de test et la propriété de l'autre projet, une propriété de définition retirée qui laisse la voisine, la propriété de build et la propriété de l'autre projet, une étiquette de définition retirée qui laisse la voisine, la propriété de définition et l'étiquette de l'autre projet, une révision de définition retirée qui laisse la voisine, l'étiquette de définition et la révision de l'autre projet, une métrique de définition retirée qui laisse la voisine, la révision de définition et la métrique de l'autre projet, un changement de build retiré qui laisse le voisin, la métrique de définition et le changement de l'autre projet, un élément de build retiré qui laisse le voisin, le changement de build et l'élément de l'autre projet, une pièce de build retirée qui laisse le voisin, l'élément de build et la pièce de l'autre projet, une étape de build retirée qui laisse le voisin, la pièce de build et l'étape de l'autre projet, une phase de build retirée qui laisse le voisin, l'étape de build et la phase de l'autre projet, un travail de build retiré qui laisse le voisin, la phase de build et le travail de l'autre projet, une tâche de build retirée qui laisse le voisin, le travail de build et la tâche de l'autre projet, un jalon de build retiré qui laisse le voisin, la tâche de build et le jalon de l'autre projet, une anomalie de build retirée qui laisse le voisin, le jalon de build et l'anomalie de l'autre projet, une tentative de build retirée qui laisse le voisin, l'anomalie de build et la tentative de l'autre projet, un avertissement de build retiré qui laisse le voisin, la tentative de build et l'avertissement de l'autre projet, une annotation de build retirée qui laisse le voisin, l'avertissement de build et l'annotation de l'autre projet, une note de build retirée qui laisse le voisin, l'annotation de build et la note de l'autre projet, un signal de build retiré qui laisse le voisin, la note de build et le signal de l'autre projet, une trace de build retirée qui laisse le voisin, le signal de build et la trace de l'autre projet, un constat de build retiré qui laisse le voisin, la trace de build et le constat de l'autre projet, une mesure de build retirée qui laisse le voisin, le constat de build et la mesure de l'autre projet, un repère de build retiré qui laisse le voisin, la mesure de build et le repère de l'autre projet, une marque de build retirée qui laisse le voisin, le repère de build et la marque de l'autre projet, une empreinte de build retirée qui laisse le voisin, la marque de build et l'empreinte de l'autre projet, une preuve de build retirée qui laisse le voisin, l'empreinte de build et la preuve de l'autre projet, un échantillon de build retiré qui laisse le voisin, la preuve de build et l'échantillon de l'autre projet, une observation de build retirée qui laisse le voisin, l'échantillon de build et l'observation de l'autre projet, un indice de build retiré qui laisse le voisin, l'observation de build et l'indice de l'autre projet, un seuil de build retiré qui laisse le voisin, l'indice de build et le seuil de l'autre projet, une borne de build retirée qui laisse la voisine, le seuil de build et la borne de l'autre projet, un compteur de build retiré qui laisse le voisin, la borne de build et le compteur de l'autre projet, une capture de build retirée qui laisse la voisine, le compteur de build et la capture de l'autre projet, un relevé de build retiré qui laisse le voisin, la capture de build et le relevé de l'autre projet, un témoin de build retiré qui laisse le voisin, le relevé de build et le témoin de l'autre projet, un palier de build retiré qui laisse le voisin, le témoin de build et le palier de l'autre projet, une cadence de build retirée qui laisse la voisine, le palier de build et la cadence de l'autre projet, un intervalle de build retiré qui laisse le voisin, la cadence de build et l'intervalle de l'autre projet, un rythme de build retiré qui laisse le voisin, l'intervalle de build et le rythme de l'autre projet, un cycle de build retiré qui laisse le voisin, le rythme de build et le cycle de l'autre projet, une période de build retirée qui laisse la voisine, le cycle de build et la période de l'autre projet, une fenêtre de build retirée qui laisse la voisine, la période de build et la fenêtre de l'autre projet, un créneau de build retiré qui laisse le voisin, la fenêtre de build et le créneau de l'autre projet, une séance de build retirée qui laisse la voisine, le créneau de build et la séance de l'autre projet, une pause de build retirée qui laisse la voisine, la séance de build et la pause de l'autre projet, une halte de build retirée qui laisse la voisine, la pause de build et la halte de l'autre projet, une accalmie de build retirée qui laisse la voisine, la halte de build et l'accalmie de l'autre projet, journaux et artefacts de build (contenu masqué jusqu'au téléchargement), pipelines dont un retiré laisse le voisin et ses exécutions, définitions de version dont une retirée laisse la voisine et ses versions, avec environnements, approbations et abandon, environnements dont un retiré laisse le voisin et ses contrôles, contrôles d'environnement (approbation, contrôle de branche, verrou exclusif), approbations de pipeline qui retiennent l'exécution, fichiers sécurisés (dépôt, lecture masquée, téléchargement), stratégies de branche et leur évaluation sur une demande de tirage, plans de test (un plan retiré qui laisse le voisin et ses suites, suite renommée, suite supprimée avec ses cas, points et résultats, une exécution retirée qui laisse la voisine et ses résultats), pages de wiki déplacées ou supprimées, flux d'artefacts dont un retiré laisse le voisin et ses paquets, un paquet Maven, PyPI ou une version npm se retire, tableaux de bord dont un retiré laisse le voisin et ses widgets, et dont un widget se retire, connexions de service (catalogue de types, secret masqué, mise à jour), groupes de variables (secret masqué, mise à jour, révélation, reprise par une build au moment de la file, recouverts par les variables de la définition, puis par un paramètre de file si allowOverride est vrai, retrait qui prévient library.variablegroup.deleted avec le secret masqué), espaces de noms de sécurité, ACL et évaluation de permissions, changesets TFVC, recherche de code et de work items, index NuGet v3, conteneur plat NuGet dont une version se retire, registre npm dont une version se retire, dépôt Maven (POM et maven-metadata.xml), index simple PyPI, paquets universels dont une version se retire, agents de pool (enregistrement, statut, demandes de travail, un pool retiré qui laisse le voisin, son agent, sa demande et sa file, et refuse le pool par défaut, une file retirée qui laisse la voisine, le pool, son agent et la file de l'autre projet, et refuse la file par défaut, une étiquette de build retirée qui laisse la voisine et l'étiquette de l'autre build, une conservation de build retirée qui laisse la voisine et la conservation de l'autre build, une configuration de test retirée qui laisse la voisine et la configuration de l'autre projet, un groupe de tâches retiré qui laisse le voisin et le groupe de l'autre projet, un groupe de déploiement retiré qui laisse le voisin et le groupe de l'autre projet, une variable de test retirée qui laisse la voisine et la variable de l'autre projet, une étiquette de work item retirée qui laisse la voisine et l'étiquette de l'autre projet, un dossier de pipeline retiré qui laisse le voisin, le pipeline et le dossier de l'autre projet et emporte son enfant, une cible de déploiement retirée qui laisse la voisine, le groupe et la cible de l'autre projet, une vue de flux retirée qui laisse la voisine, les paquets et la vue de l'autre projet, un flux créé après une suppression qui laisse l'identifiant du flux encore présent, un dossier de version retiré qui laisse le voisin, la définition et le dossier de l'autre projet et emporte son enfant, un commentaire de page de wiki retiré qui laisse le voisin, la page et le commentaire de l'autre projet, un statut de demande de tirage retiré qui laisse le voisin, la demande et le statut de l'autre projet, un commentaire de commit retiré qui laisse le voisin, le commit et le commentaire de l'autre projet, une réaction de commentaire retirée qui laisse la voisine, le commentaire et la réaction de l'autre projet, une source amont retirée qui laisse la voisine, le flux et la source de l'autre projet, une permission de flux retirée qui laisse la voisine, le flux et la permission de l'autre projet, une politique de conservation de flux retirée qui laisse la voisine, le flux et la politique de l'autre projet, un paquet retiré de la corbeille d'un flux qui laisse le voisin, le paquet encore publié, le flux et la corbeille de l'autre projet, une notification retirée qui laisse la voisine, l'abonnement de service hook et la notification de l'autre projet, un favori retiré qui laisse le voisin, la notification et le favori de l'autre projet, un droit retiré qui laisse le voisin, le favori et le droit de l'autre projet, un groupe retiré qui laisse le voisin, le droit et le groupe de l'autre projet, une étagère retirée qui laisse la voisine, le groupe et l'étagère de l'autre projet, une étiquette TFVC retirée qui laisse la voisine, l'étagère et l'étiquette TFVC de l'autre projet, un plan de livraison retiré qui laisse le voisin, l'étiquette TFVC et le plan de livraison de l'autre projet, une liste de sélection retirée qui laisse la voisine, le plan de livraison et la liste de sélection de l'autre projet, un champ retiré qui laisse le voisin, la liste de sélection et le champ de l'autre projet, une appartenance retirée qui laisse la voisine, le champ et l'appartenance de l'autre projet, un flux d'audit retiré qui laisse le voisin, l'appartenance et le flux d'audit de l'autre projet, une extension retirée qui laisse la voisine, le flux d'audit et l'extension de l'autre projet, un droit de groupe retiré qui laisse le voisin, l'extension et le droit de groupe de l'autre projet, un processus hérité retiré qui laisse le voisin, le droit de groupe, le processus système et le processus de l'autre projet, un type retiré qui laisse le voisin, le processus hérité, le type système et le type de l'autre projet, un état retiré qui laisse le voisin, le type, l'état système et l'état de l'autre projet, une règle retirée qui laisse la voisine, l'état et la règle de l'autre projet, un champ de type retiré qui laisse le voisin, la règle et le champ de l'autre projet, un comportement retiré qui laisse le voisin, le champ et le comportement de l'autre projet, une page retirée qui laisse la voisine, le comportement et la page de l'autre projet, un groupe retiré qui laisse le voisin, la page et le groupe de l'autre projet, un contrôle retiré qui laisse le voisin, le groupe et le contrôle de l'autre projet, un comportement de type retiré qui laisse le voisin, le contrôle et le comportement de l'autre projet, un modèle retiré qui laisse le voisin, le comportement et le modèle de l'autre projet, une machine retirée qui laisse la voisine, le modèle et la machine de l'autre projet, un cluster retiré qui laisse le voisin, la machine et le cluster de l'autre projet, un paramètre partagé retiré qui laisse le voisin, le cluster et le paramètre de l'autre projet, une pièce d'exécution retirée qui laisse la voisine, le paramètre et la pièce de l'autre projet, une pièce de résultat retirée qui laisse la voisine, la pièce d'exécution et la pièce de l'autre projet, un bogue de résultat retiré qui laisse le voisin, la pièce de résultat et le bogue de l'autre projet, une itération de résultat retirée qui laisse la voisine, le bogue de résultat et l'itération de l'autre projet, une action de résultat retirée qui laisse la voisine, l'itération de résultat et l'action de l'autre projet, une pièce d'itération retirée qui laisse la voisine, l'action de résultat et la pièce de l'autre projet, un paramètre d'itération retiré qui laisse le voisin, la pièce d'itération et le paramètre de l'autre projet, une pièce d'action retirée qui laisse la voisine, le paramètre d'itération et la pièce de l'autre projet, un paramètre d'action retiré qui laisse le voisin, la pièce d'action et le paramètre de l'autre projet, un journal de résultat retiré qui laisse le voisin, le paramètre d'action et le journal de l'autre projet, un sous-résultat retiré qui laisse le voisin, le journal de résultat et le sous-résultat de l'autre projet, une pièce de sous-résultat retirée qui laisse la voisine, le sous-résultat et la pièce de l'autre projet, un lien de méthode retiré qui laisse le voisin, la pièce de sous-résultat et le lien de l'autre projet, un fichier du magasin retiré qui laisse le voisin, le lien de méthode et le fichier de l'autre projet, une étiquette de test retirée qui laisse la voisine, le fichier du magasin et l'étiquette de l'autre projet, une couverture retirée qui laisse la voisine, l'étiquette de test et la couverture de l'autre projet, un type d'échec retiré qui laisse le voisin, la couverture et le type de l'autre projet, une métadonnée retirée qui laisse la voisine, le type d'échec et la métadonnée de l'autre projet, un champ de test retiré qui laisse le voisin, la métadonnée et le champ de l'autre projet, un champ de résultat retiré qui laisse le voisin, le champ de test et le champ de l'autre projet, un élément de résultat retiré qui laisse le voisin, le champ de résultat et l'élément de l'autre projet, un cas de test retiré qui laisse le voisin, l'élément de résultat et le cas de l'autre projet, une pièce de wiki retirée qui laisse la voisine, le cas de test et la pièce de l'autre projet, un résultat de test retiré qui laisse le voisin, la pièce de wiki et le résultat de l'autre projet, une propriété de build retirée qui laisse la voisine, le résultat de test et la propriété de l'autre projet, une propriété de définition retirée qui laisse la voisine, la propriété de build et la propriété de l'autre projet, une étiquette de définition retirée qui laisse la voisine, la propriété de définition et l'étiquette de l'autre projet, une révision de définition retirée qui laisse la voisine, l'étiquette de définition et la révision de l'autre projet, une métrique de définition retirée qui laisse la voisine, la révision de définition et la métrique de l'autre projet, un changement de build retiré qui laisse le voisin, la métrique de définition et le changement de l'autre projet, un élément de build retiré qui laisse le voisin, le changement de build et l'élément de l'autre projet, une pièce de build retirée qui laisse le voisin, l'élément de build et la pièce de l'autre projet, une étape de build retirée qui laisse le voisin, la pièce de build et l'étape de l'autre projet, une phase de build retirée qui laisse le voisin, l'étape de build et la phase de l'autre projet, un travail de build retiré qui laisse le voisin, la phase de build et le travail de l'autre projet, une tâche de build retirée qui laisse le voisin, le travail de build et la tâche de l'autre projet, un jalon de build retiré qui laisse le voisin, la tâche de build et le jalon de l'autre projet, une anomalie de build retirée qui laisse le voisin, le jalon de build et l'anomalie de l'autre projet, une tentative de build retirée qui laisse le voisin, l'anomalie de build et la tentative de l'autre projet, un avertissement de build retiré qui laisse le voisin, la tentative de build et l'avertissement de l'autre projet, une annotation de build retirée qui laisse le voisin, l'avertissement de build et l'annotation de l'autre projet, une note de build retirée qui laisse le voisin, l'annotation de build et la note de l'autre projet, un signal de build retiré qui laisse le voisin, la note de build et le signal de l'autre projet, une trace de build retirée qui laisse le voisin, le signal de build et la trace de l'autre projet, un constat de build retiré qui laisse le voisin, la trace de build et le constat de l'autre projet, une mesure de build retirée qui laisse le voisin, le constat de build et la mesure de l'autre projet, un repère de build retiré qui laisse le voisin, la mesure de build et le repère de l'autre projet, une marque de build retirée qui laisse le voisin, le repère de build et la marque de l'autre projet, une empreinte de build retirée qui laisse le voisin, la marque de build et l'empreinte de l'autre projet, une preuve de build retirée qui laisse le voisin, l'empreinte de build et la preuve de l'autre projet, un échantillon de build retiré qui laisse le voisin, la preuve de build et l'échantillon de l'autre projet, une observation de build retirée qui laisse le voisin, l'échantillon de build et l'observation de l'autre projet, un indice de build retiré qui laisse le voisin, l'observation de build et l'indice de l'autre projet, un seuil de build retiré qui laisse le voisin, l'indice de build et le seuil de l'autre projet, une borne de build retirée qui laisse la voisine, le seuil de build et la borne de l'autre projet, un compteur de build retiré qui laisse le voisin, la borne de build et le compteur de l'autre projet, une capture de build retirée qui laisse la voisine, le compteur de build et la capture de l'autre projet, un relevé de build retiré qui laisse le voisin, la capture de build et le relevé de l'autre projet, un témoin de build retiré qui laisse le voisin, le relevé de build et le témoin de l'autre projet, un palier de build retiré qui laisse le voisin, le témoin de build et le palier de l'autre projet, une cadence de build retirée qui laisse la voisine, le palier de build et la cadence de l'autre projet, un intervalle de build retiré qui laisse le voisin, la cadence de build et l'intervalle de l'autre projet, un rythme de build retiré qui laisse le voisin, l'intervalle de build et le rythme de l'autre projet, un cycle de build retiré qui laisse le voisin, le rythme de build et le cycle de l'autre projet, une période de build retirée qui laisse la voisine, le cycle de build et la période de l'autre projet, une fenêtre de build retirée qui laisse la voisine, la période de build et la fenêtre de l'autre projet, un créneau de build retiré qui laisse le voisin, la fenêtre de build et le créneau de l'autre projet, une séance de build retirée qui laisse la voisine, le créneau de build et la séance de l'autre projet, une pause de build retirée qui laisse la voisine, la séance de build et la pause de l'autre projet, une halte de build retirée qui laisse la voisine, la pause de build et la halte de l'autre projet, une accalmie de build retirée qui laisse la voisine, la halte de build et l'accalmie de l'autre projet), abonnements de service hooks dont un statut disabled reste silencieux, livrés au consommateur à la création et à la mise à jour d'un work item, à une poussée git, à la création, la mise à jour et la fusion d'une demande de tirage, et à la fin d'une build, tableaux de bord et widgets (Markdown, compte de work items, dernière build), requêtes WIQL en dossiers (Shared Queries, My Queries, une requête retirée qui laisse la voisine, un dossier retiré qui emporte ses enfants), mise à jour et suppression, groupes de variables, versions, abonnements `hooks`. Les autres zones restent un magasin JSON générique. Une build ou une exécution de pipeline créée en `notStarted` / `inProgress` est lue ensuite comme `completed` / `succeeded`. L'interface reprend les hubs du portail (connexion par nom d'utilisateur et PAT, recherche sur la vue d'ensemble, fiche d'élément dont les transitions viennent du type, avec discussion et historique des champs, tableau par glisser-déposer, fichiers du dépôt, commits (cherry-pick et revert) et leurs statuts, branches, étiquettes, poussées, TFVC, discussion de demande de tirage résolue, avec relecteurs dont un retiré laisse le voisin, étiquettes, commits, itérations, fusion squash, fusion qui conserve les fichiers de la source, rebase, complétion automatique et work items liés, journal, chronologie, variables du groupe lié, variables de définition, déclencheurs CI, de demande de tirage et horaire, et artefacts de pipeline, stratégies de branche, plans de test et exécutions, flux d'artefacts avec publication Maven, PyPI et npm et retrait d'une version, demandes de travail dont une retirée laisse la voisine et son agent, fichiers sécurisés dont un retiré laisse le voisin et son contenu masqué, paquets d'un flux dont un retiré laisse le voisin et sa version, connexions de service dont une retirée laisse la voisine et son secret masqué, dépôts git dont un retiré laisse le voisin et son commit, wikis dont un retiré laisse le voisin et sa page, abonnements dont un retiré laisse le voisin et sa notification, stratégies de branche dont une retirée laisse la voisine et son évaluation, pipelines YAML dont un retiré laisse le voisin et son exécution, agents du pool dont un retiré laisse le voisin et sa demande, abonnements de service hooks, tableau de bord à widgets, requêtes WIQL en dossiers, équipes du projet, zones et itérations, éditeur de wiki qui déplace ou supprime une page, écran des versions qui abandonne une version, éditeur de fichier du dépôt, écran du tableau de bord qui renomme le tableau et retire un widget, écran des artefacts qui retire un paquet, une version npm ou une version NuGet, bibliothèque qui retire un groupe de variables). Le binaire persiste dans `AZDO_DATA`. L'objectif reste la couverture complète des API publiées et la parité visuelle avec le portail Microsoft.

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
