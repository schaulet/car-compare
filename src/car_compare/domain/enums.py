"""Énumérations du domaine."""

from enum import Enum


class FuelType(str, Enum):
    """Type de carburant."""

    ESSENCE = "essence"
    DIESEL = "diesel"
    ELECTRIQUE = "electrique"
    HYBRIDE = "hybride"
    HYBRIDE_RECHARGEABLE = "hybride_rechargeable"


class TransmissionType(str, Enum):
    """Type de transmission."""

    MANUELLE = "manuelle"
    AUTOMATIQUE = "automatique"


class PurchaseType(str, Enum):
    """Type d'achat."""

    CASH = "cash"
    CREDIT = "credit"
    LOA = "loa"
    LLD = "lld"
