# language: fr
Fonctionnalité: Traitement des demandes par un agent
  Le traitement est manuel sur la carte, ou automatique si la case globale est cochée.
  L'agent ne part que s'il a la capacité et les prérogatives.

  Scénario: le traitement manuel produit le module, les tests et l'artefact
    Soit une plateforme Azurz raccordée à Azure DevOps
    Et le chat a créé une demande avec "Je veux un module Ignition nommé « OEE Ligne 1 » qui expose la fonction compute_oee(availability, performance, quality) = availability * performance * quality et un tag mémoire Plant/Ligne1/OEE."
    Quand l'opérateur déclenche le traitement de la demande
    Alors la demande est terminée
    Et la proposition décrit "compute_oee"
    Et un artefact versionné est disponible
    Et le rapport de tests est réussi
    Et le rapport mentionne "0.684"

  Scénario: la case globale déclenche le traitement
    Soit une plateforme Azurz raccordée à Azure DevOps
    Et le traitement automatique est désactivé
    Et le chat a créé une demande avec "Je veux un module Ignition nommé « OEE Ligne 1 » qui expose la fonction compute_oee(availability, performance, quality) = availability * performance * quality"
    Quand l'horloge de traitement avance
    Alors la demande reste prête
    Quand l'opérateur active le traitement automatique
    Et l'horloge de traitement avance
    Alors la demande est terminée

  Scénario: un agent sans prérogative ne traite pas le ticket
    Soit une plateforme Azurz raccordée à Azure DevOps
    Et l'agent "ignition-module" n'a pas la prérogative "artifact.publish"
    Et le chat a créé une demande avec "Je veux un module Ignition nommé « OEE Ligne 1 » qui expose la fonction compute_oee(availability, performance, quality) = availability * performance * quality"
    Quand l'opérateur déclenche le traitement de la demande
    Alors la demande n'est pas terminée
    Et le motif mentionne "artifact.publish"

  Scénario: l'API expose la case globale et les cartes
    Soit le serveur HTTP Azurz
    Quand le chat HTTP envoie une demande OEE complète
    Et l'API active le traitement automatique
    Alors l'API liste une demande terminée avec un artefact
