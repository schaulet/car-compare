#!/usr/bin/env bash
# Démonstration bout-en-bout du pont, comme le ferait le chat Dust.
set -euo pipefail

BASE="${BASE:-http://127.0.0.1:8080}"
MESSAGE="${MESSAGE:-Je veux un module Ignition nommé « OEE Ligne 1 » qui expose la fonction compute_oee(availability, performance, quality) = availability * performance * quality et un tag mémoire Plant/Ligne1/OEE.}"

python3 - "$BASE" "$MESSAGE" <<'PY'
import json
import sys
import time
import urllib.request

base, message = sys.argv[1], sys.argv[2]


def call(method, path, payload=None):
    data = None if payload is None else json.dumps(payload).encode()
    request = urllib.request.Request(
        base + path,
        data=data,
        method=method,
        headers={"Content-Type": "application/json"},
    )
    with urllib.request.urlopen(request, timeout=180) as response:
        return json.load(response)


for _ in range(60):
    try:
        health = call("GET", "/api/health")
        if health.get("status") == "ok":
            break
    except Exception:
        time.sleep(2)
else:
    sys.exit("le pont ne répond pas")

created = call("POST", "/api/demands", {"message": message, "requester": "dust"})
print("demande", created["id"], created["state"], created["title"])
enriched = call(
    "POST",
    f"/api/demands/{created['id']}/messages",
    {"message": "Ajoute le critère compute_oee(0.9, 0.8, 0.95) = 0.684"},
)
print("après enrichissement", enriched["state"])
call("PUT", "/api/settings", {"auto_process": True})
demand = enriched
for _ in range(40):
    rows = call("GET", "/api/demands")
    demand = next(item for item in rows if item["id"] == created["id"])
    if demand["state"] in {"done", "failed", "blocked"}:
        break
    time.sleep(2)
print("état", demand["state"], demand.get("reason") or "")
if demand["state"] != "done":
    sys.exit(1)
artifacts = call("GET", "/api/artifacts")
artifact = artifacts[-1]
print("artefact", artifact["name"], artifact["version"])
deployment = call(
    "POST",
    "/api/deployments",
    {
        "artifact_id": artifact["id"],
        "version": artifact["version"],
        "target_ids": ["ligne-a", "supervision"],
    },
)
print("déploiement", deployment["state"])
print(json.dumps(deployment.get("report"), ensure_ascii=False, indent=2))
PY
