# language: fr
Fonctionnalité: Registre générique des agents
  Un nouvel agent s'ajoute par enregistrement. L'hôte de traitement
  ne connaît pas les métiers.

  Scénario: enregistrer un agent sans modifier l'hôte
    Soit une plateforme Azurz raccordée à Azure DevOps
    Quand j'enregistre un agent "audit" de capacité "quality.audit" et de prérogatives "workitem.read"
    Alors les agents éligibles pour "quality.audit" sont "audit"
    Et l'hôte de traitement ne référence pas "audit" en dur
    Et l'hôte de traitement ne référence pas "ignition" en dur
