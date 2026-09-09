# Spécifications Fonctionnelles - Car Compare

## 1. Entités métier

### 1.1 Véhicule (Vehicle)

| Attribut | Type | Description |
|----------|------|-------------|
| id | UUID | Identifiant unique |
| brand | string | Marque (ex: Renault, Peugeot) |
| model | string | Modèle (ex: Clio, 308) |
| version | string | Version/finition |
| year | int | Année de mise en circulation |
| fuel_type | enum | Type de carburant (essence, diesel, électrique, hybride) |
| price | decimal | Prix catalogue |
| consumption | decimal | Consommation (L/100km ou kWh/100km) |
| co2_emissions | int | Émissions CO2 (g/km) |
| power_hp | int | Puissance (ch) |
| transmission | enum | Boîte de vitesse (manuelle, automatique) |
| doors | int | Nombre de portes |
| seats | int | Nombre de places |
| trunk_volume | int | Volume du coffre (L) |

### 1.2 Simulation

| Attribut | Type | Description |
|----------|------|-------------|
| id | UUID | Identifiant unique |
| vehicle_id | UUID | Référence au véhicule |
| duration_months | int | Durée de possession (mois) |
| annual_km | int | Kilométrage annuel prévu |
| fuel_price | decimal | Prix du carburant (€/L ou €/kWh) |
| insurance_monthly | decimal | Coût assurance mensuel |
| maintenance_annual | decimal | Coût entretien annuel |
| purchase_type | enum | Type d'achat (cash, crédit, LOA, LLD) |
| monthly_payment | decimal | Mensualité (si crédit/LOA/LLD) |
| deposit | decimal | Apport initial |
| residual_value | decimal | Valeur résiduelle estimée |

### 1.3 Comparaison

| Attribut | Type | Description |
|----------|------|-------------|
| id | UUID | Identifiant unique |
| name | string | Nom de la comparaison |
| simulations | list[UUID] | Liste des simulations à comparer |
| created_at | datetime | Date de création |
| weights | dict | Pondération des critères |

## 2. Règles métier

### 2.1 Calcul du TCO (Total Cost of Ownership)

```
TCO = Prix d'achat 
    + (Consommation × km_annuel × prix_carburant × durée_années)
    + (Assurance_mensuelle × durée_mois)
    + (Entretien_annuel × durée_années)
    - Valeur_résiduelle
```

### 2.2 Calcul du coût mensuel

```
Coût_mensuel = TCO / durée_mois
```

### 2.3 Dépréciation standard

| Année | Dépréciation |
|-------|--------------|
| 1 | 20% |
| 2 | 15% |
| 3 | 10% |
| 4-5 | 7%/an |
| 6+ | 5%/an |

## 3. Cas d'utilisation

### UC1: Ajouter un véhicule
- **Acteur**: Utilisateur
- **Précondition**: Aucune
- **Scénario principal**:
  1. L'utilisateur saisit les caractéristiques du véhicule
  2. Le système valide les données
  3. Le système enregistre le véhicule
- **Postcondition**: Le véhicule est disponible pour simulation

### UC2: Créer une simulation
- **Acteur**: Utilisateur
- **Précondition**: Au moins un véhicule existe
- **Scénario principal**:
  1. L'utilisateur sélectionne un véhicule
  2. L'utilisateur saisit les paramètres de simulation
  3. Le système calcule le TCO et le coût mensuel
- **Postcondition**: La simulation est créée avec les résultats

### UC3: Comparer des véhicules
- **Acteur**: Utilisateur
- **Précondition**: Au moins 2 simulations existent
- **Scénario principal**:
  1. L'utilisateur sélectionne les simulations à comparer
  2. Le système affiche la comparaison côte à côte
  3. Le système calcule les différences et le scoring
- **Postcondition**: La comparaison est affichée

## 4. API Endpoints

### Véhicules
- `GET /api/v1/vehicles` - Liste des véhicules
- `GET /api/v1/vehicles/{id}` - Détail d'un véhicule
- `POST /api/v1/vehicles` - Créer un véhicule
- `PUT /api/v1/vehicles/{id}` - Modifier un véhicule
- `DELETE /api/v1/vehicles/{id}` - Supprimer un véhicule

### Simulations
- `GET /api/v1/simulations` - Liste des simulations
- `GET /api/v1/simulations/{id}` - Détail d'une simulation
- `POST /api/v1/simulations` - Créer une simulation
- `DELETE /api/v1/simulations/{id}` - Supprimer une simulation

### Comparaisons
- `GET /api/v1/comparisons` - Liste des comparaisons
- `GET /api/v1/comparisons/{id}` - Détail d'une comparaison
- `POST /api/v1/comparisons` - Créer une comparaison
- `DELETE /api/v1/comparisons/{id}` - Supprimer une comparaison
