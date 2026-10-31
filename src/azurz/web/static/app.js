const state = {
  selected: null,
  targets: new Set(),
  busy: false,
  snapshot: "",
};

const demandsEl = document.querySelector("#demands");
const artifactsEl = document.querySelector("#artifacts");
const targetsEl = document.querySelector("#targets");
const deploymentsEl = document.querySelector("#deployments");
const agentsEl = document.querySelector("#agents");
const messageEl = document.querySelector("#message");
const modeEl = document.querySelector("#mode");
const chatResult = document.querySelector("#chat-result");
const deployStatus = document.querySelector("#deploy-status");
const autoEl = document.querySelector("#auto");

const labels = {
  needs_input: "à enrichir",
  ready: "prête",
  in_progress: "en cours",
  done: "terminée",
  failed: "échec",
  blocked: "bloquée",
};

async function api(path, options) {
  const response = await fetch(path, {
    headers: { "Content-Type": "application/json" },
    ...options,
  });
  const payload = await response.json().catch(() => ({}));
  if (!response.ok) {
    throw new Error(payload.detail || "Requête refusée");
  }
  return payload;
}

function escape(value) {
  return String(value ?? "")
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;");
}

function setMode() {
  modeEl.textContent = state.selected
    ? `Le message enrichira la demande #${state.selected}.`
    : "Une nouvelle demande sera créée.";
}

async function sendMessage() {
  const message = messageEl.value.trim();
  if (!message) {
    chatResult.textContent = "Écrivez d'abord la demande.";
    return;
  }
  state.busy = true;
  chatResult.textContent = "Analyse en cours…";
  try {
    const demand = state.selected
      ? await api(`/api/demands/${state.selected}/messages`, {
          method: "POST",
          body: JSON.stringify({ message }),
        })
      : await api("/api/demands", {
          method: "POST",
          body: JSON.stringify({ message, requester: "operator" }),
        });
    state.selected = demand.id;
    const missing = (demand.missing || []).join(", ");
    chatResult.textContent = missing
      ? `Demande #${demand.id} — ${labels[demand.state] || demand.state}. Il manque : ${missing}.`
      : `Demande #${demand.id} — ${labels[demand.state] || demand.state}.`;
    messageEl.value = "";
    await refresh();
  } catch (error) {
    chatResult.textContent = error.message;
  } finally {
    state.busy = false;
  }
}

async function loadExample() {
  const example = await api("/api/examples");
  messageEl.value = example.message;
}

async function toggleAuto() {
  autoEl.disabled = true;
  try {
    await api("/api/settings", {
      method: "PUT",
      body: JSON.stringify({ auto_process: autoEl.checked }),
    });
    await refresh();
  } catch (error) {
    chatResult.textContent = error.message;
  } finally {
    autoEl.disabled = false;
  }
}

async function processDemand(id) {
  state.busy = true;
  chatResult.textContent = `Traitement de la demande #${id}…`;
  try {
    await api(`/api/demands/${id}/process`, { method: "POST", body: "{}" });
    await refresh();
    chatResult.textContent = `Demande #${id} traitée.`;
  } catch (error) {
    chatResult.textContent = error.message;
  } finally {
    state.busy = false;
  }
}

async function deploy(artifactId, version) {
  const targetIds = [...state.targets];
  if (!targetIds.length) {
    deployStatus.textContent = "Cochez au moins une instance.";
    return;
  }
  deployStatus.textContent = "Déploiement en cours…";
  try {
    const demand = await api("/api/deployments", {
      method: "POST",
      body: JSON.stringify({ artifact_id: artifactId, version, target_ids: targetIds }),
    });
    deployStatus.textContent = `Déploiement #${demand.id} : ${labels[demand.state] || demand.state}.`;
    await refresh();
  } catch (error) {
    deployStatus.textContent = error.message;
  }
}

function renderDemands(rows) {
  if (!rows.length) {
    demandsEl.innerHTML = "<p class='muted'>Aucune demande pour l'instant.</p>";
    return;
  }
  demandsEl.innerHTML = rows
    .slice()
    .reverse()
    .map((demand) => {
      const report = (demand.report && demand.report.scenarios) || [];
      const proofs = report
        .map(
          (item) =>
            `<li>${escape(item.name)} — ${escape(item.status)}<div class="pre">${escape(item.evidence)}</div></li>`,
        )
        .join("");
      const canRun = demand.state === "ready";
      return `<article class="card ${state.selected === demand.id ? "selected" : ""}" data-id="${demand.id}">
        <div class="row">
          <strong>#${demand.id} ${escape(demand.title)}</strong>
          <span class="badge ${escape(demand.state)}">${escape(labels[demand.state] || demand.state)}</span>
        </div>
        <p class="muted">${escape(demand.capability || "capacité non déterminée")}</p>
        <p>${escape(demand.description)}</p>
        ${demand.missing && demand.missing.length ? `<p>À préciser : ${escape(demand.missing.join(", "))}</p>` : ""}
        ${demand.proposal ? `<details><summary>Proposition</summary><div class="pre">${escape(demand.proposal)}</div></details>` : ""}
        ${proofs ? `<details open><summary>Rapport de tests</summary><ul>${proofs}</ul></details>` : ""}
        ${demand.reason ? `<p>${escape(demand.reason)}</p>` : ""}
        <div class="row">
          <button type="button" data-select="${demand.id}" class="secondary">Enrichir celle-ci</button>
          ${canRun ? `<button type="button" data-process="${demand.id}">Traiter</button>` : ""}
        </div>
      </article>`;
    })
    .join("");
}

function renderTargets(rows) {
  targetsEl.innerHTML = rows
    .map(
      (target) => `<label class="row">
        <input type="checkbox" data-target="${escape(target.id)}" ${state.targets.has(target.id) ? "checked" : ""} />
        <span>${escape(target.name)}</span>
        <a class="ghost" href="${escape(target.public_url)}" target="_blank" rel="noreferrer">ouvrir</a>
      </label>`,
    )
    .join("");
}

function renderArtifacts(rows) {
  if (!rows.length) {
    artifactsEl.innerHTML = "<p class='muted'>Aucun artefact publié.</p>";
    return;
  }
  const body = rows
    .map(
      (artifact) => `<tr>
        <td>${escape(artifact.name)}</td>
        <td>${escape(artifact.version)}</td>
        <td><a href="/api/artifacts/${artifact.id}/rapport" target="_blank" rel="noreferrer">rapport</a></td>
        <td><a href="/api/artifacts/${artifact.id}/download">télécharger</a></td>
        <td><button type="button" data-deploy="${artifact.id}" data-version="${escape(artifact.version)}">Déployer la sélection</button></td>
      </tr>`,
    )
    .join("");
  artifactsEl.innerHTML = `<table><thead><tr><th>Artefact</th><th>Version</th><th></th><th></th><th></th></tr></thead><tbody>${body}</tbody></table>`;
}

function renderDeployments(rows) {
  if (!rows.length) {
    deploymentsEl.innerHTML = "<p class='muted'>Aucun déploiement.</p>";
    return;
  }
  deploymentsEl.innerHTML = rows
    .slice()
    .reverse()
    .map((item) => {
      const details = (item.details || [])
        .map((detail) => `${detail.target_id} : ${detail.status} (${detail.detail || ""})`)
        .join(" · ");
      return `<article class="artifact"><strong>${escape(item.version)}</strong> vers ${escape((item.target_ids || []).join(", "))}
        <div class="muted">${escape(details)}</div></article>`;
    })
    .join("");
}

function renderAgents(payload) {
  const required = Object.fromEntries((payload.policies || []).map((item) => [item.capability, item.required]));
  agentsEl.innerHTML = (payload.agents || [])
    .map((agent) => {
      const needs = agent.capabilities.map((item) => (required[item] || []).join(", ")).filter(Boolean);
      return `<article class="agent">
        <strong>${escape(agent.name)}</strong>
        <div>Capacités : ${escape(agent.capabilities.join(", "))}</div>
        <div>Prérogatives : ${escape(agent.prerogatives.join(", "))}</div>
        <div class="muted">Exigées pour ces capacités : ${escape(needs.join(" · ") || "aucune")}</div>
      </article>`;
    })
    .join("");
}

async function refresh() {
  const [settings, demands, targets, artifacts, deployments, agents] = await Promise.all([
    api("/api/settings"),
    api("/api/demands"),
    api("/api/targets"),
    api("/api/artifacts"),
    api("/api/deployments"),
    api("/api/agents"),
  ]);
  const snapshot = JSON.stringify({
    settings,
    demands,
    targets,
    artifacts,
    deployments,
    agents,
    selected: state.selected,
  });
  if (snapshot === state.snapshot) {
    return;
  }
  state.snapshot = snapshot;
  if (document.activeElement !== autoEl) {
    autoEl.checked = settings.auto_process;
  }
  renderDemands(demands);
  renderTargets(targets);
  renderArtifacts(artifacts);
  renderDeployments(deployments);
  renderAgents(agents);
  setMode();
}

document.querySelector("#send").addEventListener("click", sendMessage);
document.querySelector("#fresh").addEventListener("click", () => {
  state.selected = null;
  state.snapshot = "";
  setMode();
  refresh();
});
document.querySelector("#example").addEventListener("click", loadExample);
autoEl.addEventListener("change", toggleAuto);

document.body.addEventListener("click", (event) => {
  const select = event.target.closest("[data-select]");
  if (select) {
    state.selected = Number(select.dataset.select);
    setMode();
    refresh();
    return;
  }
  const process = event.target.closest("[data-process]");
  if (process) {
    processDemand(Number(process.dataset.process));
    return;
  }
  const deployButton = event.target.closest("[data-deploy]");
  if (deployButton) {
    deploy(deployButton.dataset.deploy, deployButton.dataset.version);
  }
});

document.body.addEventListener("change", (event) => {
  const input = event.target.closest("[data-target]");
  if (!input) {
    return;
  }
  if (input.checked) {
    state.targets.add(input.dataset.target);
  } else {
    state.targets.delete(input.dataset.target);
  }
});

refresh();
setInterval(() => {
  if (!state.busy) {
    refresh().catch(() => {});
  }
}, 2500);
