# language: fr
Fonctionnalité: Comparaison de véhicules
  En tant qu'utilisateur
  Je veux comparer plusieurs véhicules
  Afin de choisir le plus économique

  Contexte:
    Étant donné qu'il existe un véhicule "Renault Clio" à 22000 euros
    Et qu'il existe un véhicule "Peugeot 308" à 30000 euros
    Et que j'ai créé une simulation pour chaque véhicule

  Scénario: Créer une comparaison
    Quand je crée une comparaison "Ma comparaison"
    Alors la comparaison est créée avec 2 véhicules
    Et le meilleur TCO est identifié

  Scénario: Comparaison nécessite au moins 2 simulations
    Quand je tente de créer une comparaison avec 1 seule simulation
    Alors une erreur est retournée
