"""Petites fonctions de texte partagées par les analyseurs et les générateurs."""

from __future__ import annotations

import re
import unicodedata


def slug(value: str) -> str:
    normalized = unicodedata.normalize("NFKD", value)
    ascii_text = normalized.encode("ascii", "ignore").decode("ascii")
    token = re.sub(r"[^A-Za-z0-9]+", "_", ascii_text).strip("_").lower()
    return token or "module"


def format_number(value: float) -> str:
    rounded = round(float(value), 6)
    text = f"{rounded:.6f}".rstrip("0").rstrip(".")
    return text or "0"
