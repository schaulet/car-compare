"""Contributions d'outils MCP. Chaque capacité s'enregistre, l'adaptateur les monte."""

from __future__ import annotations

import json

from azurz.domain.registry import ToolCatalog, ToolContribution
from azurz.platform import OEE_MESSAGE, Platform


def _dump(payload: object) -> str:
    return json.dumps(payload, ensure_ascii=False, default=str)


def contribute_demand_tools(catalog: ToolCatalog, platform: Platform) -> None:
    def analyser_demande(message: str) -> str:
        """Analyse un message de chat sans créer de work item."""
        analysis = platform.analyzers.analyze(message)
        return _dump(
            {
                "titre": analysis.title,
                "resume": analysis.summary,
                "capacite": analysis.capability,
                "prete": analysis.actionable,
                "manques": analysis.missing,
                "criteres": analysis.acceptance_criteria,
            }
        )

    def creer_demande_azure_devops(message: str, demandeur: str = "dust") -> str:
        """Analyse le message et crée la demande dans Azure DevOps."""
        demand = platform.submit_message(message, requester=demandeur)
        return _dump(_demand_payload(demand))

    def enrichir_demande(demande_id: int, message: str) -> str:
        """Enrichit une demande existante avec un nouveau message du chat."""
        demand = platform.iterate(demande_id, message)
        return _dump(_demand_payload(demand))

    def consulter_demande(demande_id: int) -> str:
        """Retourne l'état, la proposition, le rapport et les artefacts d'une demande."""
        return _dump(_demand_payload(platform.workitems.get(demande_id)))

    def lister_demandes() -> str:
        """Liste les demandes suivies par le pont."""
        return _dump([_demand_payload(item) for item in platform.workitems.list()])

    catalog.register(
        ToolContribution(
            "analyser_demande",
            "Analyse un message du chat Dust et indique si la demande est assez précise "
            "pour devenir un ticket traitable.",
            analyser_demande,
        )
    )
    catalog.register(
        ToolContribution(
            "creer_demande_azure_devops",
            "Analyse le message du chat et crée la demande correspondante dans Azure DevOps.",
            creer_demande_azure_devops,
        )
    )
    catalog.register(
        ToolContribution(
            "enrichir_demande",
            "Ajoute un message du chat à une demande existante et met à jour son analyse.",
            enrichir_demande,
        )
    )
    catalog.register(
        ToolContribution(
            "consulter_demande",
            "Lit une demande : état, proposition d'implémentation, rapport de tests, artefacts.",
            consulter_demande,
        )
    )
    catalog.register(
        ToolContribution(
            "lister_demandes",
            "Liste les demandes en attente, en cours et terminées.",
            lister_demandes,
        )
    )


def contribute_delivery_tools(catalog: ToolCatalog, platform: Platform) -> None:
    def lister_artefacts() -> str:
        """Liste les artefacts publiés avec leur version."""
        return _dump([_artifact_payload(item) for item in platform.artifacts.list()])

    def lister_instances_ignition() -> str:
        """Liste les instances Ignition vers lesquelles un artefact peut être déployé."""
        return _dump(
            [
                {
                    "id": target.id,
                    "nom": target.name,
                    "url": target.public_url,
                }
                for target in platform.targets.values()
            ]
        )

    def deployer_artefact(artefact_id: str, version: str, instances: str) -> str:
        """Déploie une version vers les instances listées, séparées par des virgules."""
        target_ids = [part.strip() for part in instances.split(",") if part.strip()]
        demand = platform.deploy(artefact_id, version, target_ids)
        return _dump(_demand_payload(demand))

    catalog.register(
        ToolContribution(
            "lister_artefacts",
            "Liste les artefacts disponibles et leur version.",
            lister_artefacts,
        )
    )
    catalog.register(
        ToolContribution(
            "lister_instances_ignition",
            "Liste les instances Ignition du lab.",
            lister_instances_ignition,
        )
    )
    catalog.register(
        ToolContribution(
            "deployer_artefact",
            "Demande à l'agent de déploiement d'installer une version d'artefact "
            "sur les instances indiquées.",
            deployer_artefact,
        )
    )


def build_catalog(platform: Platform) -> ToolCatalog:
    catalog = ToolCatalog()
    contribute_demand_tools(catalog, platform)
    contribute_delivery_tools(catalog, platform)
    return catalog


def _demand_payload(demand: object) -> dict:
    return {
        "id": demand.id,
        "titre": demand.title,
        "etat": demand.state,
        "capacite": demand.capability,
        "manques": demand.missing,
        "proposition": demand.proposal,
        "rapport": demand.report,
        "artefacts": demand.artifact_ids,
        "motif": demand.reason,
        "agent": demand.agent_id,
        "messages": demand.messages,
        "exemple": OEE_MESSAGE,
    }


def _artifact_payload(artifact: object) -> dict:
    return {
        "id": artifact.id,
        "nom": artifact.name,
        "version": artifact.version,
        "demande_id": artifact.demand_id,
    }
