# language: fr
Fonctionnalité: Mise à disposition et déploiement des artefacts
  L'opérateur choisit la version et les instances. L'agent de déploiement
  n'envoie l'artefact que vers cette sélection.

  Scénario: l'opérateur choisit les instances
    Soit une plateforme avec les instances "ligne-a" et "ligne-b"
    Et une demande module OEE a été traitée
    Quand l'opérateur déploie la dernière version vers "ligne-a"
    Alors l'instance "ligne-a" contient le projet du module
    Et l'instance "ligne-b" ne contient pas le projet du module
    Et la demande de déploiement est terminée
    Et la demande de déploiement a été traitée par l'agent "deploiement"

  Scénario: le déploiement exige la prérogative
    Soit une plateforme avec les instances "ligne-a" et "ligne-b"
    Et une demande module OEE a été traitée
    Et l'agent "deploiement" n'a pas la prérogative "ignition.deploy"
    Quand l'opérateur déploie la dernière version vers "ligne-a"
    Alors la demande de déploiement n'est pas terminée
    Et le motif de déploiement mentionne "ignition.deploy"
