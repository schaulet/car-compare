"""Évaluation d'expressions arithmétiques sans appel ni accès au runtime."""

from __future__ import annotations

import ast

_ALLOWED = (
    ast.Expression,
    ast.BinOp,
    ast.UnaryOp,
    ast.Name,
    ast.Load,
    ast.Constant,
    ast.Add,
    ast.Sub,
    ast.Mult,
    ast.Div,
    ast.Pow,
    ast.Mod,
    ast.USub,
    ast.UAdd,
)


class ExpressionError(ValueError):
    """L'expression n'est pas une formule arithmétique fermée."""


def evaluate(expression: str, variables: dict[str, float]) -> float:
    tree = ast.parse(expression, mode="eval")
    for node in ast.walk(tree):
        if not isinstance(node, _ALLOWED):
            raise ExpressionError(f"élément interdit dans l'expression : {type(node).__name__}")
        if isinstance(node, ast.Name) and node.id not in variables:
            raise ExpressionError(f"nom inconnu : {node.id}")
    value = eval(compile(tree, "<expression>", "eval"), {"__builtins__": {}}, variables)  # noqa: S307
    if isinstance(value, bool) or not isinstance(value, int | float):
        raise ExpressionError("l'expression doit produire un nombre")
    return float(value)


def validate_expression(expression: str, params: list[str]) -> None:
    sample = {name: 1.0 for name in params}
    evaluate(expression, sample)
