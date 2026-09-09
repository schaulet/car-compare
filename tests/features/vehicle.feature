# language: fr
Fonctionnalité: Gestion des véhicules
  En tant qu'utilisateur
  Je veux pouvoir gérer ma liste de véhicules
  Afin de les comparer par la suite

  Scénario: Ajouter un véhicule
    Étant donné que je n'ai aucun véhicule
    Quand j'ajoute un véhicule "Renault Clio" de 2024 à 22000 euros
    Alors le véhicule est créé avec succès
    Et la liste des véhicules contient 1 élément

  Scénario: Lister les véhicules
    Étant donné que j'ai ajouté un véhicule "Renault Clio"
    Et que j'ai ajouté un véhicule "Peugeot 308"
    Quand je liste les véhicules
    Alors je vois 2 véhicules

  Scénario: Supprimer un véhicule
    Étant donné que j'ai ajouté un véhicule "Tesla Model 3"
    Quand je supprime ce véhicule
    Alors la liste des véhicules est vide
