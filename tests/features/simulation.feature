# language: fr
Fonctionnalité: Simulation de coût de possession
  En tant qu'utilisateur
  Je veux simuler le coût de possession d'un véhicule
  Afin de connaître mon budget réel

  Contexte:
    Étant donné qu'il existe un véhicule "Renault Clio" à 22000 euros consommant 5.5 L/100km

  Scénario: Calculer le TCO pour un achat comptant
    Quand je crée une simulation pour 60 mois avec 15000 km/an
    Alors le TCO est calculé
    Et le coût mensuel est positif

  Scénario: Impact de la consommation sur le coût
    Étant donné qu'il existe un véhicule "SUV Gros" à 22000 euros consommant 12.0 L/100km
    Quand je crée une simulation pour le "SUV Gros" sur 60 mois avec 15000 km/an
    Et que je crée une simulation pour la "Renault Clio" sur 60 mois avec 15000 km/an
    Alors le TCO du "SUV Gros" est supérieur à celui de la "Renault Clio"
