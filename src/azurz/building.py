"""Construction d'un module Ignition, de ses tests Cucumber et du rapport."""

from __future__ import annotations

import ast
import json
import subprocess
import sys
import textwrap
import xml.etree.ElementTree as ET
from datetime import UTC, datetime
from html import escape
from pathlib import Path
from typing import Any

from azurz.domain.expressions import validate_expression
from azurz.domain.textutil import format_number

_STEPS = '''\
from behave import given, then, use_step_matcher, when
import importlib.util
import json
from pathlib import Path
import xml.etree.ElementTree as ET

use_step_matcher("parse")


def _root():
    return Path(__file__).resolve().parents[2]


def _functions():
    root = _root()
    found = {}
    for code in (root / "scripts").glob("*/code.py"):
        spec = importlib.util.spec_from_file_location(code.parent.name, code)
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
        for name in dir(module):
            if name.startswith("_"):
                continue
            value = getattr(module, name)
            if callable(value):
                found[name] = value
    return found


@given("le module est chargé")
def step_loaded(context):
    context.fns = _functions()
    context.root = _root()


@when('j\\'appelle "{name}" avec "{args}"')
def step_call(context, name, args):
    values = [float(part) for part in args.split() if part]
    context.result = context.fns[name](*values)
    context.called = name


@then('le résultat vaut "{expected}"')
def step_result(context, expected):
    assert abs(float(context.result) - float(expected)) < 1e-4, (
        f"{context.called} a renvoyé {context.result}, attendu {expected}"
    )


@then('le descripteur déclare l\\'identifiant "{module_id}"')
def step_id(context, module_id):
    tree = ET.parse(context.root / "module.xml")
    assert tree.findtext(".//id") == module_id


@then('le descripteur déclare la version "{version}"')
def step_version(context, version):
    tree = ET.parse(context.root / "module.xml")
    assert tree.findtext(".//version") == version


@then('le module déclare le tag "{path}"')
def step_tag(context, path):
    tags = context.fns["azurz_tag_definitions"]()
    assert any(item["path"] == path for item in tags), tags
'''


def prepare_build(spec: dict[str, Any], dest: Path) -> None:
    _validate_spec(spec)
    dest.mkdir(parents=True, exist_ok=True)
    (dest / "spec.json").write_text(
        json.dumps(spec, ensure_ascii=False, indent=2),
        encoding="utf-8",
    )
    (dest / "module.xml").write_text(_module_xml(spec), encoding="utf-8")
    (dest / "proposition.md").write_text(_proposal(spec), encoding="utf-8")
    package = str(spec["package"])
    code = _python_module(spec)
    script_dir = dest / "scripts" / package
    script_dir.mkdir(parents=True, exist_ok=True)
    (script_dir / "code.py").write_text(code, encoding="utf-8")
    project = dest / "gateway_project"
    resource_dir = project / "ignition" / "script-python" / package
    resource_dir.mkdir(parents=True, exist_ok=True)
    (project / "project.json").write_text(_project_json(spec), encoding="utf-8")
    (resource_dir / "code.py").write_text(code, encoding="utf-8")
    (resource_dir / "resource.json").write_text(_resource_json(), encoding="utf-8")
    feature_dir = dest / "features"
    (feature_dir / "steps").mkdir(parents=True, exist_ok=True)
    (feature_dir / "module.feature").write_text(_feature(spec), encoding="utf-8")
    (feature_dir / "steps" / "steps.py").write_text(_STEPS, encoding="utf-8")


def run_tests(dest: Path) -> dict[str, Any]:
    root = dest.resolve()
    report_path = root / "behave.json"
    completed = subprocess.run(
        [
            sys.executable,
            "-m",
            "behave",
            "--no-capture",
            "--format",
            "json.pretty",
            "--outfile",
            str(report_path),
            "features",
        ],
        cwd=root,
        capture_output=True,
        text=True,
        timeout=90,
        check=False,
    )
    parsed = _parse_behave(report_path) if report_path.exists() else {"scenarios": []}
    passed = completed.returncode == 0 and bool(parsed["scenarios"])
    if not parsed["scenarios"]:
        passed = False
    evidence = _evidence(dest, parsed)
    return {
        "passed": passed,
        "returncode": completed.returncode,
        "scenarios": evidence,
        "stdout": completed.stdout[-8000:],
        "stderr": completed.stderr[-4000:],
    }


def write_reports(dest: Path, spec: dict[str, Any], report: dict[str, Any]) -> None:
    (dest / "rapport.json").write_text(
        json.dumps(report, ensure_ascii=False, indent=2),
        encoding="utf-8",
    )
    (dest / "rapport.html").write_text(_html(spec, report), encoding="utf-8")


def package_modl(dest: Path, spec: dict[str, Any]) -> Path:
    import zipfile

    filename = f"{spec['package']}-{spec['version']}.modl"
    target = dest / filename
    with zipfile.ZipFile(target, "w", compression=zipfile.ZIP_DEFLATED) as archive:
        for relative in ("module.xml",):
            archive.write(dest / relative, relative)
        package = dest / "scripts" / str(spec["package"])
        for file in package.rglob("*"):
            if file.is_file():
                archive.write(file, str(file.relative_to(dest)))
    return target


def _validate_spec(spec: dict[str, Any]) -> None:
    for function in spec["functions"]:
        params = list(function["params"])
        expression = str(function["expression"])
        validate_expression(expression, params)
        tree = ast.parse(expression, mode="eval")
        dumped = ast.unparse(tree)
        function["expression"] = dumped


def _module_xml(spec: dict[str, Any]) -> str:
    module = ET.Element("module")
    ET.SubElement(module, "id").text = str(spec["module_id"])
    ET.SubElement(module, "name").text = str(spec["module_name"])
    ET.SubElement(module, "description").text = _proposal(spec).split("\n", 1)[0]
    ET.SubElement(module, "version").text = str(spec["version"])
    ET.SubElement(module, "requiredIgnitionVersion").text = "8.1.0"
    ET.SubElement(module, "requiredFrameworkVersion").text = "8"
    root = ET.Element("modules")
    root.append(module)
    body = ET.tostring(root, encoding="unicode")
    return '<?xml version="1.0" encoding="UTF-8"?>\n' + body + "\n"


def _proposal(spec: dict[str, Any]) -> str:
    lines = [
        f"# Proposition — {spec['module_name']}",
        "",
        f"- Identifiant : `{spec['module_id']}`",
        f"- Version : `{spec['version']}`",
        f"- Projet gateway : `{spec['project_name']}`",
        "",
        "## Fonctions",
    ]
    for function in spec["functions"]:
        params = ", ".join(function["params"])
        lines.append(f"- `{function['name']}({params}) = {function['expression']}`")
    lines.append("")
    lines.append("## Tags mémoire")
    if spec["tags"]:
        for tag in spec["tags"]:
            lines.append(f"- `{tag['path']}` ({tag['data_type']})")
    else:
        lines.append("- Aucun tag demandé.")
    lines.extend(
        [
            "",
            "## Implémentation",
            "Le livrable est une bibliothèque de scripts Ignition 8.1, empaquetée avec un "
            "`module.xml`. Les tests Cucumber exécutent les fonctions et vérifient le descripteur. "
            "Le déploiement copie le projet dans l'instance choisie.",
            "",
        ]
    )
    return "\n".join(lines)


def _python_module(spec: dict[str, Any]) -> str:
    chunks = [
        f'"""Module {spec["module_name"]} version {spec["version"]}, généré par Azurz."""',
        "",
    ]
    for function in spec["functions"]:
        params = ", ".join(function["params"])
        chunks.append(f"def {function['name']}({params}):")
        chunks.append(f'    """{function["expression"]}"""')
        chunks.append(f"    return {function['expression']}")
        chunks.append("")
    tag_literal = json.dumps(spec["tags"], ensure_ascii=False)
    chunks.append("def azurz_tag_definitions():")
    chunks.append(f"    return {tag_literal}")
    chunks.append("")
    return "\n".join(chunks)


def _project_json(spec: dict[str, Any]) -> str:
    payload = {
        "title": spec["module_name"],
        "description": f"Projet généré par Azurz {spec['version']}",
        "parent": "",
        "enabled": True,
        "inheritable": False,
        "editable": True,
    }
    return json.dumps(payload, ensure_ascii=False, indent=2) + "\n"


def _resource_json() -> str:
    payload = {
        "scope": "A",
        "version": 1,
        "restricted": False,
        "overridable": True,
        "files": ["code.py"],
        "attributes": {
            "lastModification": {
                "actor": "azurz",
                "timestamp": datetime.now(UTC).strftime("%Y-%m-%dT%H:%M:%SZ"),
            }
        },
    }
    return json.dumps(payload, ensure_ascii=False, indent=2) + "\n"


def _feature(spec: dict[str, Any]) -> str:
    lines = [
        f"Feature: Module {spec['module_name']}",
        f"  Le module {spec['module_id']} version {spec['version']} est livré avec ses preuves.",
        "",
    ]
    for case in spec["cases"]:
        args = " ".join(format_number(float(value)) for value in case["args"])
        expected = format_number(float(case["expected"]))
        lines.extend(
            [
                f"  Scenario: {case['function']} vaut {expected}",
                "    Given le module est chargé",
                f'    When j\'appelle "{case["function"]}" avec "{args}"',
                f'    Then le résultat vaut "{expected}"',
                "",
            ]
        )
    lines.extend(
        [
            "  Scenario: le descripteur identifie le module",
            "    Given le module est chargé",
            f'    Then le descripteur déclare l\'identifiant "{spec["module_id"]}"',
            f'    And le descripteur déclare la version "{spec["version"]}"',
            "",
        ]
    )
    for tag in spec["tags"]:
        lines.extend(
            [
                f"  Scenario: le tag {tag['path']} est déclaré",
                "    Given le module est chargé",
                f'    Then le module déclare le tag "{tag["path"]}"',
                "",
            ]
        )
    return "\n".join(lines)


def _parse_behave(path: Path) -> dict[str, Any]:
    raw = json.loads(path.read_text(encoding="utf-8"))
    scenarios = []
    for feature in raw:
        for element in feature.get("elements") or []:
            if element.get("type") != "scenario":
                continue
            steps = element.get("steps") or []
            statuses = [((step.get("result") or {}).get("status") or "failed") for step in steps]
            if element.get("status"):
                status = element["status"]
            elif statuses and all(item == "passed" for item in statuses):
                status = "passed"
            else:
                status = "failed"
            scenarios.append({"name": element.get("name") or "", "status": status, "steps": steps})
    return {"scenarios": scenarios}


def _evidence(dest: Path, parsed: dict[str, Any]) -> list[dict[str, Any]]:
    spec = json.loads((dest / "spec.json").read_text(encoding="utf-8"))
    by_expected = {}
    for case in spec["cases"]:
        expected = format_number(float(case["expected"]))
        args = ", ".join(format_number(float(value)) for value in case["args"])
        by_expected[f"{case['function']} vaut {expected}"] = (
            f"{case['function']}({args}) = {expected}"
        )
    rows = []
    for scenario in parsed["scenarios"]:
        proof = by_expected.get(scenario["name"], scenario["name"])
        failed = next(
            (
                (step.get("result") or {}).get("error_message")
                for step in scenario["steps"]
                if (step.get("result") or {}).get("status") not in {None, "passed"}
            ),
            None,
        )
        if isinstance(failed, list):
            failed = "\n".join(failed)
        rows.append(
            {
                "name": scenario["name"],
                "status": scenario["status"],
                "evidence": proof if not failed else f"{proof}\n{failed}",
            }
        )
    return rows


def _html(spec: dict[str, Any], report: dict[str, Any]) -> str:
    rows = []
    for scenario in report["scenarios"]:
        rows.append(
            "<tr>"
            f"<td>{escape(str(scenario['name']))}</td>"
            f"<td>{escape(str(scenario['status']))}</td>"
            f"<td><pre>{escape(str(scenario['evidence']))}</pre></td>"
            "</tr>"
        )
    body = "\n".join(rows) or "<tr><td colspan='3'>Aucun scénario</td></tr>"
    status = "réussi" if report["passed"] else "en échec"
    return textwrap.dedent(
        f"""\
        <!DOCTYPE html>
        <html lang="fr">
        <head>
          <meta charset="utf-8" />
          <title>Rapport {escape(str(spec["module_name"]))} {escape(str(spec["version"]))}</title>
          <style>
            body {{ font-family: Georgia, serif; margin: 2rem; color: #1c1915; }}
            table {{ border-collapse: collapse; width: 100%; }}
            td, th {{ border: 1px solid #d9d1c3; padding: 0.5rem; vertical-align: top; }}
            pre {{ white-space: pre-wrap; margin: 0; }}
          </style>
        </head>
        <body>
          <h1>{escape(str(spec["module_name"]))} {escape(str(spec["version"]))}</h1>
          <p>Statut des tests : {status}.</p>
          <table>
            <thead><tr><th>Scénario</th><th>Statut</th><th>Preuve</th></tr></thead>
            <tbody>
            {body}
            </tbody>
          </table>
        </body>
        </html>
        """
    )
