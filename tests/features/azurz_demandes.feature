# language: fr
Fonctionnalité: Demandes du chat vers Azure DevOps
  Le chat Dust analyse une demande et produit un work item Azure DevOps.
  L'utilisateur peut ensuite enrichir ce même ticket.

  Scénario: une demande de chat crée un work item
    Soit une plateforme Azurz raccordée à Azure DevOps
    Quand le chat envoie "Je veux un module Ignition nommé « OEE Ligne 1 » qui expose la fonction compute_oee(availability, performance, quality) = availability * performance * quality et un tag mémoire Plant/Ligne1/OEE."
    Alors une demande Azure DevOps existe avec le titre "OEE Ligne 1"
    Et la demande est prête à être traitée
    Et la demande porte la capacité "ignition.module.build"
    Et la spécification déclare le tag "Plant/Ligne1/OEE"
    Et les outils MCP exposés contiennent "creer_demande_azure_devops"
    Et les outils MCP exposés contiennent "enrichir_demande"

  Scénario: une demande incomplète reste à enrichir
    Soit une plateforme Azurz raccordée à Azure DevOps
    Quand le chat envoie "Il faudrait quelque chose pour l'atelier"
    Alors la demande n'est pas prête
    Et l'analyse signale des éléments manquants

  Scénario: l'itération enrichit la même demande
    Soit une plateforme Azurz raccordée à Azure DevOps
    Et le chat a créé une demande avec "Il faudrait un module Ignition pour la ligne"
    Quand le chat enrichit cette demande avec "Le module s'appelle « OEE Ligne 1 » et la fonction compute_oee(availability, performance, quality) = availability * performance * quality"
    Alors il n'existe qu'une seule demande
    Et la demande est prête à être traitée
    Et le texte "Il faudrait un module Ignition pour la ligne" reste dans l'historique
