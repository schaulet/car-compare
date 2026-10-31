"""Steps du pont Azurz."""

import inspect

from behave import given, then, use_step_matcher, when
from behave.runner import Context

from azurz.bootstrap import create_memory
from azurz.contrib import build_catalog
from azurz.domain.models import Target
from azurz.orchestration import select_agents
from azurz.platform import OEE_MESSAGE

use_step_matcher("re")


def _last(context: Context):
    demands = context.platform.workitems.list()
    assert demands, "aucune demande"
    return demands[-1]


@given("une plateforme Azurz raccordée à Azure DevOps")
def step_platform(context: Context):
    context.platform = create_memory(targets=[])
    context.demand = None
    context.deployment = None


@given('une plateforme avec les instances "(?P<a>[^"]+)" et "(?P<b>[^"]+)"')
def step_platform_targets(context: Context, a: str, b: str):
    context.platform = create_memory(
        targets=[
            Target(a, a, "ignition", "", ""),
            Target(b, b, "ignition", "", ""),
        ]
    )
    context.demand = None
    context.deployment = None


@given("le traitement automatique est désactivé")
def step_auto_off(context: Context):
    context.platform.set_auto(False)


@given('le chat a créé une demande avec "(?P<message>.*)"')
def step_created(context: Context, message: str):
    context.demand = context.platform.submit_message(message)


@given('l\'agent "(?P<agent>[^"]+)" n\'a pas la prérogative "(?P<right>[^"]+)"')
def step_revoke(context: Context, agent: str, right: str):
    context.platform.revoke_prerogative(agent, right)


@given("une demande module OEE a été traitée")
def step_oee_done(context: Context):
    context.demand = context.platform.submit_message(OEE_MESSAGE)
    context.demand = context.platform.trigger(context.demand.id)
    assert context.demand.state == "done"
    assert context.demand.artifact_ids


@given("le serveur HTTP Azurz")
def step_http(context: Context):
    from starlette.testclient import TestClient

    from azurz.web.app import create_http_app

    context.platform = create_memory(targets=[])
    context.http = TestClient(
        create_http_app(context.platform, mcp_token="dust-lab-token", worker=False)
    )
    context.http.__enter__()


@when('le chat envoie "(?P<message>.*)"')
def step_send(context: Context, message: str):
    context.demand = context.platform.submit_message(message)


@when('le chat enrichit cette demande avec "(?P<message>.*)"')
def step_enrich(context: Context, message: str):
    context.demand = context.platform.iterate(context.demand.id, message)


@when("l'opérateur déclenche le traitement de la demande")
def step_process(context: Context):
    context.demand = context.platform.trigger(context.demand.id)


@when("l'horloge de traitement avance")
def step_tick(context: Context):
    processed = context.platform.tick()
    if processed:
        context.demand = processed[-1]
    elif context.demand is not None:
        context.demand = context.platform.workitems.get(context.demand.id)


@when("l'opérateur active le traitement automatique")
def step_auto_on(context: Context):
    context.platform.set_auto(True)


@when("le chat HTTP envoie une demande OEE complète")
def step_http_send(context: Context):
    response = context.http.post(
        "/api/demands",
        json={"message": OEE_MESSAGE, "requester": "dust"},
    )
    assert response.status_code == 200, response.text
    context.demand = context.platform.workitems.get(response.json()["id"])


@when("l'API active le traitement automatique")
def step_http_auto(context: Context):
    response = context.http.put("/api/settings", json={"auto_process": True})
    assert response.status_code == 200, response.text
    context.demand = _last(context)


@when('l\'opérateur déploie la dernière version vers "(?P<target>[^"]+)"')
def step_deploy(context: Context, target: str):
    artifact = context.platform.artifacts.list()[-1]
    context.deployment = context.platform.deploy(artifact.id, artifact.version, [target])


@when(
    'j\'enregistre un agent "(?P<agent>[^"]+)" de capacité "(?P<capability>[^"]+)" '
    'et de prérogatives "(?P<rights>[^"]+)"'
)
def step_register(context: Context, agent: str, capability: str, rights: str):
    from azurz.agents.callable import FunctionAgent
    from azurz.domain.models import AgentDescriptor, AgentResult

    def _run(_demand, _context):
        return AgentResult(state="done")

    context.platform.agents.register(
        FunctionAgent(
            AgentDescriptor(
                id=agent,
                name=agent,
                capabilities=(capability,),
                prerogatives=frozenset(part.strip() for part in rights.split(",") if part.strip()),
            ),
            _run,
        )
    )
    context.registered_capability = capability
    context.registered_agent = agent


@then('une demande Azure DevOps existe avec le titre "(?P<title>.*)"')
def step_title(context: Context, title: str):
    assert context.demand.title == title
    assert context.demand.id > 0


@then("la demande est prête à être traitée")
def step_ready(context: Context):
    assert context.demand.state == "ready"


@then("la demande reste prête")
def step_still_ready(context: Context):
    assert context.demand.state == "ready"


@then('la demande porte la capacité "(?P<capability>[^"]+)"')
def step_capability(context: Context, capability: str):
    assert context.demand.capability == capability


@then('la spécification déclare le tag "(?P<path>[^"]+)"')
def step_tag(context: Context, path: str):
    tags = context.demand.spec.get("tags") or []
    assert any(item.get("path") == path for item in tags), tags


@then('les outils MCP exposés contiennent "(?P<name>[^"]+)"')
def step_tools(context: Context, name: str):
    assert name in build_catalog(context.platform).names()


@then("la demande n'est pas prête")
def step_not_ready(context: Context):
    assert context.demand.state == "needs_input"


@then("l'analyse signale des éléments manquants")
def step_missing(context: Context):
    assert context.demand.missing


@then("il n'existe qu'une seule demande")
def step_single(context: Context):
    assert len(context.platform.workitems.list()) == 1


@then('le texte "(?P<message>.*)" reste dans l\'historique')
def step_history(context: Context, message: str):
    assert message in context.demand.messages


@then("la demande est terminée")
def step_done(context: Context):
    assert context.demand.state == "done"


@then('la proposition décrit "(?P<needle>[^"]+)"')
def step_proposal(context: Context, needle: str):
    assert needle in context.demand.proposal


@then("un artefact versionné est disponible")
def step_artifact(context: Context):
    artifacts = context.platform.artifacts.list()
    assert artifacts
    assert artifacts[-1].version
    assert artifacts[-1].id in context.demand.artifact_ids


@then("le rapport de tests est réussi")
def step_report_ok(context: Context):
    assert context.demand.report["passed"] is True


@then('le rapport mentionne "(?P<needle>[^"]+)"')
def step_report_text(context: Context, needle: str):
    blob = str(context.demand.report)
    assert needle in blob


@then("la demande n'est pas terminée")
def step_not_done(context: Context):
    assert context.demand.state != "done"


@then('le motif mentionne "(?P<needle>[^"]+)"')
def step_reason(context: Context, needle: str):
    assert needle in context.demand.reason


@then("l'API liste une demande terminée avec un artefact")
def step_http_done(context: Context):
    response = context.http.get("/api/demands")
    assert response.status_code == 200, response.text
    rows = response.json()
    assert any(row["state"] == "done" and row["artifact_ids"] for row in rows)
    artifacts = context.http.get("/api/artifacts")
    assert artifacts.status_code == 200
    assert artifacts.json()


@then('l\'instance "(?P<target>[^"]+)" contient le projet du module')
def step_has_project(context: Context, target: str):
    artifact = context.platform.artifacts.list()[-1]
    project = context.platform.project_dir(target, artifact.project_name)
    assert (project / "project.json").is_file()
    assert (project / "ignition" / "script-python").is_dir()


@then('l\'instance "(?P<target>[^"]+)" ne contient pas le projet du module')
def step_without_project(context: Context, target: str):
    artifact = context.platform.artifacts.list()[-1]
    project = context.platform.project_dir(target, artifact.project_name)
    assert not project.exists()


@then("la demande de déploiement est terminée")
def step_deploy_done(context: Context):
    assert context.deployment.state == "done"


@then('la demande de déploiement a été traitée par l\'agent "(?P<agent>[^"]+)"')
def step_deploy_agent(context: Context, agent: str):
    assert context.deployment.agent_id == agent


@then("la demande de déploiement n'est pas terminée")
def step_deploy_not_done(context: Context):
    assert context.deployment.state != "done"


@then('le motif de déploiement mentionne "(?P<needle>[^"]+)"')
def step_deploy_reason(context: Context, needle: str):
    assert needle in context.deployment.reason


@then('les agents éligibles pour "(?P<capability>[^"]+)" sont "(?P<agent>[^"]+)"')
def step_eligible(context: Context, capability: str, agent: str):
    found = select_agents(context.platform.agents.all(), context.platform.policies, capability)
    assert [item.descriptor.id for item in found] == [agent]


@then('l\'hôte de traitement ne référence pas "(?P<needle>[^"]+)" en dur')
def step_host_generic(context: Context, needle: str):
    from azurz import orchestration

    source = inspect.getsource(orchestration).lower()
    assert needle.lower() not in source
