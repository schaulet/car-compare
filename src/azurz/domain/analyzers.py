"""Analyseurs enregistrables. L'hôte d'intake n'en connaît aucun."""

from __future__ import annotations

import re

from azurz.domain.expressions import ExpressionError, evaluate, validate_expression
from azurz.domain.models import Analysis
from azurz.domain.textutil import format_number, slug

_NAME = re.compile(
    r"(?:nommé|nomme|s['’]appelle|module)\s+[«\"]([^»\"]+)[»\"]",
    re.IGNORECASE,
)
_FUNCTION = re.compile(
    r"(?:fonction|function)\s+([A-Za-z_][A-Za-z0-9_]*)\s*"
    r"\(\s*([^)]*?)\s*\)\s*=\s*(.+?)"
    r"(?=\s+et\s+|\s+version\b|\s+crit[eè]re\b|\n|$)",
    re.IGNORECASE | re.DOTALL,
)
_TAG = re.compile(
    r"\btag(?:\s+(?:mémoire|memoire|memory))?\s+([A-Za-z0-9_./\-]+)",
    re.IGNORECASE,
)
_VERSION = re.compile(r"\bversion\s+(\d+\.\d+\.\d+)\b", re.IGNORECASE)
_CASE = re.compile(
    r"\b([A-Za-z_][A-Za-z0-9_]*)\s*\(\s*"
    r"([0-9]+(?:\.[0-9]+)?(?:\s*,\s*[0-9]+(?:\.[0-9]+)?)*)\s*\)\s*"
    r"(?:=|vaut)\s*([0-9]+(?:\.[0-9]+)?)"
)
_RATE_NAMES = {
    "availability",
    "performance",
    "quality",
    "rendement",
    "rate",
    "ratio",
    "oee",
}
_RATE_SAMPLES = (0.9, 0.8, 0.95, 0.7)
_COUNT_SAMPLES = (2.0, 3.0, 4.0, 5.0)


def _sample_args(params: list[str]) -> list[float]:
    rates = 0
    counts = 0
    args: list[float] = []
    for name in params:
        lowered = name.lower()
        if lowered in _RATE_NAMES or lowered.endswith("_rate"):
            args.append(_RATE_SAMPLES[rates % len(_RATE_SAMPLES)])
            rates += 1
        else:
            args.append(_COUNT_SAMPLES[counts % len(_COUNT_SAMPLES)])
            counts += 1
    return args


class IgnitionModuleAnalyzer:
    """Spécialisation : demande de module Ignition."""

    def score(self, text: str) -> float:
        lowered = text.lower()
        hits = sum(
            1
            for key in ("ignition", "inductive", "module", "oee", "perspective", "scada")
            if key in lowered
        )
        module_for_ignition = "module" in lowered and (
            "ignition" in lowered or "inductive" in lowered or "oee" in lowered
        )
        if module_for_ignition:
            return 10 + hits
        if "ignition" in lowered or "inductive automation" in lowered:
            return 5 + hits
        return 0

    def analyze(self, text: str) -> Analysis:
        missing: list[str] = []
        names = [match.group(1).strip() for match in _NAME.finditer(text)]
        module_name = names[-1] if names else ""
        if not module_name:
            missing.append("nom du module")

        versions = _VERSION.findall(text)
        version = versions[-1] if versions else "1.0.0"

        functions: dict[str, dict[str, object]] = {}
        for match in _FUNCTION.finditer(text):
            fname = match.group(1)
            params = [part.strip() for part in match.group(2).split(",") if part.strip()]
            expression = match.group(3).strip().rstrip(".")
            if any(not part.isidentifier() for part in params):
                missing.append(f"paramètres de {fname}")
                continue
            try:
                validate_expression(expression, params)
            except ExpressionError:
                missing.append(f"expression valide pour {fname}")
                continue
            functions[fname] = {"name": fname, "params": params, "expression": expression}

        if not functions:
            missing.append("fonction")

        tags = []
        seen_paths: set[str] = set()
        for match in _TAG.finditer(text):
            path = match.group(1).rstrip(".,;")
            if path not in seen_paths:
                seen_paths.add(path)
                tags.append({"path": path, "data_type": "Float4"})

        known = set(functions)
        cases: list[dict[str, object]] = []
        seen_cases: set[tuple[str, tuple[float, ...]]] = set()
        for match in _CASE.finditer(text):
            fname = match.group(1)
            if fname not in known:
                continue
            args = [float(part.strip()) for part in match.group(2).split(",")]
            expected = float(match.group(3))
            key = (fname, tuple(args))
            if key in seen_cases:
                continue
            seen_cases.add(key)
            cases.append(
                {"function": fname, "args": args, "expected": expected, "source": "demande"}
            )

        for function in functions.values():
            params = list(function["params"])  # type: ignore[arg-type]
            args = _sample_args(params)
            key = (str(function["name"]), tuple(args))
            if key in seen_cases:
                continue
            expected = evaluate(str(function["expression"]), dict(zip(params, args, strict=True)))
            cases.append(
                {
                    "function": function["name"],
                    "args": args,
                    "expected": round(expected, 6),
                    "source": "expression",
                }
            )

        package = slug(module_name) if module_name else ""
        spec = {
            "module_name": module_name,
            "module_id": f"com.azurz.{package}" if package else "",
            "project_name": f"azurz_{package}" if package else "",
            "package": package,
            "version": version,
            "functions": list(functions.values()),
            "tags": tags,
            "cases": cases,
        }
        actionable = not missing
        criteria = _criteria_text(spec) if actionable else ""
        summary = _summary(spec, missing)
        title = module_name or _fallback_title(text)
        return Analysis(
            title=title,
            summary=summary,
            capability="ignition.module.build",
            actionable=actionable,
            missing=missing,
            spec=spec if module_name or functions else spec,
            acceptance_criteria=criteria,
            tags=["azurz", "ignition-module"],
        )


class FallbackAnalyzer:
    """Dernier recours : la demande est conservée, sans capacité métier."""

    def score(self, text: str) -> float:
        return 0.01

    def analyze(self, text: str) -> Analysis:
        title = _fallback_title(text)
        return Analysis(
            title=title,
            summary="La demande ne correspond à aucune capacité enregistrée.",
            capability="",
            actionable=False,
            missing=["préciser le type de demande et le résultat attendu"],
            spec={},
            acceptance_criteria="",
            tags=["azurz"],
        )


def _fallback_title(text: str) -> str:
    compact = " ".join(text.split())
    if len(compact) <= 80:
        return compact or "Demande sans titre"
    return compact[:77] + "..."


def _summary(spec: dict[str, object], missing: list[str]) -> str:
    if missing:
        return "Demande incomplète : " + ", ".join(missing) + "."
    functions = spec.get("functions") or []
    names = ", ".join(str(item["name"]) for item in functions)  # type: ignore[index]
    return f"Module {spec.get('module_name')} {spec.get('version')} avec {names}."


def _criteria_text(spec: dict[str, object]) -> str:
    lines = []
    for case in spec.get("cases") or []:
        args = ", ".join(format_number(float(value)) for value in case["args"])  # type: ignore[index]
        expected = format_number(float(case["expected"]))  # type: ignore[index]
        lines.append(f"{case['function']}({args}) = {expected}")  # type: ignore[index]
    return "\n".join(lines)
