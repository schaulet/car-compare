use std::collections::BTreeMap;

use axum::http::{header, Method, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use base64::Engine;
use serde_json::{json, Map, Value};

use crate::http::{fail, parse_json, pct_decode, query_param};
use crate::store::{object_id, value_key, Bucket};
use crate::AppState;

pub fn dispatch(
    state: &AppState,
    method: &Method,
    org: &str,
    project: &str,
    rest: &[String],
    query: &str,
    body: &[u8],
) -> Option<Response> {
    let area = rest.first()?.to_ascii_lowercase();
    let resource = rest.get(1)?.to_ascii_lowercase();
    let response = match (area.as_str(), resource.as_str()) {
        ("git", "repositories") => git_repositories(state, method, org, project, rest, query, body),
        ("git", "refs") => git_alias(state, method, org, project, "refs", query, body),
        ("git", "commits") => git_alias(state, method, org, project, "commits", query, body),
        ("git", "pushes") => git_alias(state, method, org, project, "pushes", query, body),
        ("git", "pullrequests") => pull_requests(state, method, org, project, None, rest, body),
        ("build", "definitions") => definitions(state, method, org, project, body),
        ("build", "builds") => builds(state, method, org, project, rest, query, body),
        ("distributedtask", "queues") | ("distributedtask", "pools") => {
            task_agents(state, method, org, Some(project), rest, query, body)
        }
        ("distributedtask", "securefiles") => secure_files(state, method, org, project, rest, query, body),
        ("pipelines", "pipelines") => pipelines(state, method, org, project, rest, body),
        ("pipelines", "runs") => pipeline_runs(state, method, org, project, rest),
        ("pipelines", "checks") => check_configurations(state, method, org, project, rest, query, body),
        ("pipelines", "approvals") => pipeline_approvals(state, method, org, project, rest, query, body),
        ("testplan", "plans") | ("test", "plans") => test_plans(state, method, org, project, rest, body),
        ("test", "runs") => test_runs(state, method, org, project, rest, body),
        ("wiki", "wikis") => wikis(state, method, org, project, rest, query, body),
        ("packaging", "feeds") => feeds(state, method, org, project, rest, body),
        ("release", "definitions") => release_definitions(state, method, org, project, rest, body),
        ("release", "releases") => release_releases(state, method, org, project, rest, body),
        ("release", "approvals") => release_approvals(state, method, org, project, rest, body),
        ("policy", "types") => policy_types(method, org, project, rest),
        ("policy", "configurations") => policy_configurations(state, method, org, project, rest, body),
        ("policy", "evaluations") => policy_evaluations(state, method, org, project, query, body),
        _ => return None,
    };
    Some(response)
}

pub fn org_dispatch(state: &AppState, method: &Method, org: &str, rest: &[String], query: &str, body: &[u8]) -> Option<Response> {
    let area = rest.first()?.to_ascii_lowercase();
    if area == "securitynamespaces" && method == Method::GET {
        return Some(Json(security_namespaces(org)).into_response());
    }
    let resource = rest.get(1)?.to_ascii_lowercase();
    let response = match (area.as_str(), resource.as_str()) {
        ("hooks", "subscriptions") => subscriptions(state, method, org, rest, body),
        ("hooks", "inbox") => hooks_inbox(state, method, org),
        ("distributedtask", "pools") | ("distributedtask", "queues") => {
            task_agents(state, method, org, None, rest, query, body)
        }
        ("graph", "users") => graph(state, method, org, "graph/users", "azurz"),
        ("graph", "groups") => graph(state, method, org, "graph/groups", format!("{org} Team")),
        _ => return None,
    };
    Some(response)
}

pub fn wit_catalog(method: &Method, _org: &str, _project: &str, rest: &[String]) -> Option<Response> {
    if method != Method::GET {
        return None;
    }
    let resource = rest.get(1)?.to_ascii_lowercase();
    if resource == "workitemtypes" && rest.len() == 2 {
        return Some(Json(work_item_types()).into_response());
    }
    if resource == "fields" && rest.len() == 2 {
        return Some(Json(wit_fields()).into_response());
    }
    None
}

fn git_repositories(
    state: &AppState,
    method: &Method,
    org: &str,
    project: &str,
    rest: &[String],
    query: &str,
    body: &[u8],
) -> Response {
    if rest.len() == 2 && method == Method::GET {
        let value = list(state, org, project, "git/repositories")
            .into_iter()
            .map(|item| dress_repo(org, project, item))
            .collect::<Vec<_>>();
        return Json(page(value)).into_response();
    }
    if rest.len() == 2 && method == Method::POST {
        let payload = parse_json(body).unwrap_or(json!({}));
        let name = text_of(&payload, "name").unwrap_or_else(|| format!("{project}-repo"));
        let created = state.write(|db| {
            db.ensure_project(org, project);
            let stored = db.project_mut(org, project)?;
            let item = {
                let bucket = stored.buckets.entry("git/repositories".into()).or_insert_with(Bucket::empty);
                let id = format!("repo-{}", bucket.next);
                let item = dress_repo(
                    org,
                    project,
                    json!({"id": id, "name": name, "defaultBranch": "refs/heads/main", "size": 0}),
                );
                bucket.put(item.clone());
                item
            };
            stored.seed_missing(project);
            Some(item)
        });
        return match created {
            Some(item) => Json(item).into_response(),
            None => fail(StatusCode::NOT_FOUND, "projet inconnu"),
        };
    }
    let repo_id = rest.get(2).map(String::as_str).unwrap_or("");
    if rest.len() == 3 && method == Method::GET {
        return match find_repo(state, org, project, repo_id) {
            Some(item) => Json(dress_repo(org, project, item)).into_response(),
            None => fail(StatusCode::NOT_FOUND, "dépôt introuvable"),
        };
    }
    let sub = rest.get(3).map(|value| value.to_ascii_lowercase()).unwrap_or_default();
    match sub.as_str() {
        "refs" => refs(state, method, org, project, repo_id, query, body),
        "annotatedtags" => annotated_tags(state, method, org, project, repo_id, tail_after(rest, "annotatedtags"), body),
        "commits" => commits(state, method, org, project, repo_id, tail_after(rest, "commits"), query, body),
        "pushes" => pushes(state, method, org, project, repo_id, tail_after(rest, "pushes"), body),
        "cherrypicks" => git_replay(state, method, org, project, repo_id, tail_after(rest, "cherrypicks"), body, false),
        "reverts" => git_replay(state, method, org, project, repo_id, tail_after(rest, "reverts"), body, true),
        "pullrequests" => pull_requests(state, method, org, project, Some(repo_id), rest, body),
        "labels" => repo_labels(state, method, org, project, repo_id, tail_after(rest, "labels")),
        "items" => items(state, org, project, repo_id, query),
        "diffs" => diffs(state, org, project, repo_id, query),
        _ => fail(StatusCode::NOT_FOUND, "ressource git introuvable"),
    }
}

fn git_alias(
    state: &AppState,
    method: &Method,
    org: &str,
    project: &str,
    kind: &str,
    query: &str,
    body: &[u8],
) -> Response {
    let Some(repo_id) = default_repo_id(state, org, project) else {
        return fail(StatusCode::NOT_FOUND, "dépôt introuvable");
    };
    match kind {
        "refs" => refs(state, method, org, project, &repo_id, query, body),
        "commits" => commits(state, method, org, project, &repo_id, &[], "", body),
        "pushes" => pushes(state, method, org, project, &repo_id, &[], body),
        _ => fail(StatusCode::NOT_FOUND, "ressource git introuvable"),
    }
}

fn refs(
    state: &AppState,
    method: &Method,
    org: &str,
    project: &str,
    repo_id: &str,
    query: &str,
    body: &[u8],
) -> Response {
    let key = format!("git/repositories/{repo_id}/refs");
    if method == Method::GET {
        let mut value = list(state, org, project, &key);
        if let Some(filter) = query_param(query, "filter") {
            let decoded = pct_decode(filter);
            let prefix = if decoded.starts_with("refs/") {
                decoded
            } else {
                format!("refs/{decoded}")
            };
            value.retain(|item| text_of(item, "name").unwrap_or_default().starts_with(&prefix));
        }
        return Json(page(value)).into_response();
    }
    if method != Method::POST {
        return fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée");
    }
    let payload = parse_json(body).unwrap_or(json!([]));
    let updates = payload.as_array().cloned().unwrap_or_else(|| vec![payload]);
    let created = state.write(|db| {
        db.ensure_project(org, project);
        let stored = db.project_mut(org, project)?;
        let mut rows = Vec::new();
        for update in updates {
            let name = text_of(&update, "name").unwrap_or_else(|| "refs/heads/main".into());
            let old_object = update
                .get("oldObjectId")
                .cloned()
                .unwrap_or(json!("0000000000000000000000000000000000000000"));
            if text_of(&update, "newObjectId").as_deref().is_some_and(is_zero_object_id) {
                let removed = stored
                    .buckets
                    .get_mut(&key)
                    .is_some_and(|bucket| bucket.remove(&name));
                if let Some(short) = name.strip_prefix("refs/tags/") {
                    let tags_key = format!("git/repositories/{repo_id}/annotatedtags");
                    if let Some(tags) = stored.buckets.get_mut(&tags_key) {
                        let ids: Vec<String> = tags
                            .items
                            .iter()
                            .filter_map(|(id, item)| {
                                (item.get("name").and_then(|value| value.as_str()) == Some(short)).then(|| id.clone())
                            })
                            .collect();
                        for id in ids {
                            tags.remove(&id);
                        }
                    }
                }
                rows.push(json!({
                    "name": name,
                    "oldObjectId": old_object,
                    "newObjectId": "0000000000000000000000000000000000000000",
                    "success": removed,
                    "updateStatus": if removed { "succeeded" } else { "invalidRefName" }
                }));
                continue;
            }
            let object = text_of(&update, "newObjectId")
                .filter(|value| !value.is_empty() && !is_zero_object_id(value))
                .unwrap_or_else(|| object_id(&format!("{repo_id}:{name}")));
            let item = json!({
                "id": name,
                "name": name,
                "objectId": object,
                "oldObjectId": old_object,
                "success": true
            });
            stored.buckets.entry(key.clone()).or_insert_with(Bucket::empty).put(item.clone());
            rows.push(item);
        }
        Some(rows)
    });
    match created {
        Some(value) => {
            let names = value.iter().filter_map(|item| text_of(item, "name")).collect::<Vec<_>>();
            advance_pull_iterations(state, org, project, repo_id, &names);
            Json(page(value)).into_response()
        }
        None => fail(StatusCode::NOT_FOUND, "dépôt introuvable"),
    }
}

fn annotated_tags(
    state: &AppState,
    method: &Method,
    org: &str,
    project: &str,
    repo_id: &str,
    tail: &[String],
    body: &[u8],
) -> Response {
    if find_repo(state, org, project, repo_id).is_none() {
        return fail(StatusCode::NOT_FOUND, "dépôt introuvable");
    }
    let key = format!("git/repositories/{repo_id}/annotatedtags");
    if let Some(object_id) = tail.first() {
        if method != Method::GET {
            return fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée");
        }
        return match get_item(state, org, project, &key, object_id) {
            Some(item) => Json(item).into_response(),
            None => fail(StatusCode::NOT_FOUND, "étiquette introuvable"),
        };
    }
    if method != Method::POST {
        return fail(StatusCode::NOT_FOUND, "étiquette introuvable");
    }
    let payload = parse_json(body).unwrap_or(json!({}));
    let Some(name) = text_of(&payload, "name").as_deref().and_then(tag_short_name) else {
        return fail(StatusCode::BAD_REQUEST, "nom d'étiquette absent");
    };
    let commit = payload
        .get("taggedObject")
        .and_then(|value| text_of(value, "objectId"))
        .unwrap_or_default();
    if get_item(state, org, project, &format!("git/repositories/{repo_id}/commits"), &commit).is_none() {
        return fail(StatusCode::BAD_REQUEST, "commit introuvable");
    }
    let ref_name = format!("refs/tags/{name}");
    if list(state, org, project, &format!("git/repositories/{repo_id}/refs"))
        .iter()
        .any(|item| text_of(item, "name").as_deref() == Some(ref_name.as_str()))
    {
        return fail(StatusCode::BAD_REQUEST, "étiquette existante");
    }
    let tag_id = object_id(&format!("{repo_id}:tag:{name}:{commit}"));
    let message = text_of(&payload, "message").unwrap_or_default();
    let item = json!({
        "id": tag_id,
        "name": name,
        "objectId": tag_id,
        "message": message,
        "taggedObject": {"objectId": commit, "objectType": "commit"},
        "taggedBy": {"name": "azurz", "email": "azurz@azurz.lab", "date": "2026-10-29T00:00:00Z"},
        "url": format!("/{org}/{project}/_apis/git/repositories/{repo_id}/annotatedtags/{tag_id}")
    });
    put_item(state, org, project, &key, item.clone());
    put_item(
        state,
        org,
        project,
        &format!("git/repositories/{repo_id}/refs"),
        json!({
            "id": ref_name,
            "name": ref_name,
            "objectId": tag_id,
            "peeledObjectId": commit,
            "success": true
        }),
    );
    Json(item).into_response()
}

fn is_zero_object_id(value: &str) -> bool {
    value.len() == 40 && value.chars().all(|ch| ch == '0')
}

fn tag_short_name(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    let name = trimmed.strip_prefix("refs/tags/").unwrap_or(trimmed);
    if name.is_empty()
        || name.chars().any(|ch| ch.is_whitespace() || matches!(ch, '~' | '^' | ':' | '\\' | '?' | '*' | '['))
        || name.contains("..")
        || name.starts_with('/')
        || name.ends_with('/')
        || name.ends_with('.')
        || name.contains("//")
        || name.contains("@{")
    {
        None
    } else {
        Some(name.to_string())
    }
}

fn commits(state: &AppState, method: &Method, org: &str, project: &str, repo_id: &str, tail: &[String], query: &str, body: &[u8]) -> Response {
    let key = format!("git/repositories/{repo_id}/commits");
    if let Some(commit_id) = tail.first() {
        if tail.get(1).is_some_and(|part| part.eq_ignore_ascii_case("statuses")) {
            return commit_statuses(state, method, org, project, repo_id, commit_id, query, body);
        }
        if tail.get(1).is_some_and(|part| part.eq_ignore_ascii_case("changes")) && method == Method::GET {
            let changes = list(state, org, project, &format!("{key}/{commit_id}/changes"));
            return Json(json!({
                "changeCounts": change_counts(&changes),
                "changes": changes
            }))
            .into_response();
        }
        if method == Method::GET {
            return match get_item(state, org, project, &key, commit_id) {
                Some(item) => Json(item).into_response(),
                None => fail(StatusCode::NOT_FOUND, "commit introuvable"),
            };
        }
        return fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée");
    }
    if method == Method::GET {
        return Json(page(list(state, org, project, &key))).into_response();
    }
    if method != Method::POST {
        return fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée");
    }
    let payload = parse_json(body).unwrap_or(json!({}));
    let comment = text_of(&payload, "comment")
        .or_else(|| text_of(&payload, "name"))
        .unwrap_or_else(|| "Mise à jour".into());
    let commit_id = object_id(&format!("{repo_id}:{comment}:{}", list(state, org, project, &key).len()));
    let item = json!({
        "id": commit_id,
        "commitId": commit_id,
        "comment": comment,
        "author": {"name": "azurz", "date": "2026-10-29T00:00:00Z"},
        "committer": {"name": "azurz", "date": "2026-10-29T00:00:00Z"},
        "url": format!("/{org}/{project}/_apis/git/repositories/{repo_id}/commits/{commit_id}")
    });
    put_item(state, org, project, &key, item.clone());
    Json(item).into_response()
}

fn commit_statuses(
    state: &AppState,
    method: &Method,
    org: &str,
    project: &str,
    repo_id: &str,
    commit_id: &str,
    query: &str,
    body: &[u8],
) -> Response {
    let commits_key = format!("git/repositories/{repo_id}/commits");
    if get_item(state, org, project, &commits_key, commit_id).is_none() {
        return fail(StatusCode::NOT_FOUND, "commit introuvable");
    }
    let key = format!("{commits_key}/{commit_id}/statuses");
    if method == Method::GET {
        let mut items = list(state, org, project, &key);
        items.sort_by_key(|item| std::cmp::Reverse(item.get("id").and_then(|value| value.as_i64()).unwrap_or(0)));
        let latest_only = query_param(query, "latestOnly")
            .is_some_and(|value| value.eq_ignore_ascii_case("true") || value == "1");
        if latest_only {
            let mut seen = std::collections::BTreeSet::new();
            items.retain(|item| {
                let context = item.get("context");
                let name = context.and_then(|value| value.get("name")).and_then(|value| value.as_str()).unwrap_or("");
                let genre = context.and_then(|value| value.get("genre")).and_then(|value| value.as_str()).unwrap_or("");
                seen.insert(format!("{genre}\n{name}"))
            });
        }
        return Json(page(items)).into_response();
    }
    if method != Method::POST {
        return fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée");
    }
    let payload = parse_json(body).unwrap_or(json!({}));
    let Some(state_name) = text_of(&payload, "state").as_deref().and_then(git_status_state) else {
        return fail(StatusCode::BAD_REQUEST, "état inconnu");
    };
    let Some(context) = payload.get("context").and_then(|value| value.as_object()) else {
        return fail(StatusCode::BAD_REQUEST, "contexte absent");
    };
    let Some(name) = context.get("name").and_then(|value| value.as_str()).filter(|value| !value.is_empty()) else {
        return fail(StatusCode::BAD_REQUEST, "contexte absent");
    };
    let genre = context.get("genre").and_then(|value| value.as_str()).unwrap_or("");
    let next = list(state, org, project, &key).len() as i64 + 1;
    let item = json!({
        "id": next,
        "state": state_name,
        "description": text_of(&payload, "description").unwrap_or_default(),
        "context": {"name": name, "genre": genre},
        "targetUrl": payload.get("targetUrl").cloned().unwrap_or(Value::Null),
        "creationDate": "2026-10-29T00:00:00Z",
        "updatedDate": "2026-10-29T00:00:00Z",
        "createdBy": {"displayName": "azurz", "uniqueName": "azurz"}
    });
    put_item(state, org, project, &key, item.clone());
    Json(item).into_response()
}

fn git_status_state(value: &str) -> Option<&'static str> {
    match value.to_ascii_lowercase().as_str() {
        "notset" => Some("notSet"),
        "pending" => Some("pending"),
        "succeeded" => Some("succeeded"),
        "failed" => Some("failed"),
        "error" => Some("error"),
        "partiallysucceeded" => Some("partiallySucceeded"),
        "notapplicable" => Some("notApplicable"),
        _ => None,
    }
}

fn pushes(state: &AppState, method: &Method, org: &str, project: &str, repo_id: &str, tail: &[String], body: &[u8]) -> Response {
    let key = format!("git/repositories/{repo_id}/pushes");
    if let Some(push_id) = tail.first() {
        if method == Method::GET {
            return match get_item(state, org, project, &key, push_id) {
                Some(item) => Json(item).into_response(),
                None => fail(StatusCode::NOT_FOUND, "poussée introuvable"),
            };
        }
        return fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée");
    }
    if method == Method::GET {
        return Json(page(list(state, org, project, &key))).into_response();
    }
    if method != Method::POST {
        return fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée");
    }
    let payload = parse_json(body).unwrap_or(json!({}));
    if payload.get("commits").is_none() && payload.get("refUpdates").is_none() {
        let ref_name = text_of(&payload, "refName")
            .or_else(|| text_of(&payload, "name"))
            .unwrap_or_else(|| "refs/heads/main".into());
        let next = list(state, org, project, &key).len() as i64 + 1;
        let item = json!({
            "id": next,
            "pushId": next,
            "date": "2026-10-29T00:00:00Z",
            "refName": ref_name,
            "repository": {"id": repo_id, "name": repo_name(state, org, project, repo_id)},
            "pushedBy": {"displayName": "azurz"}
        });
        put_item(state, org, project, &key, item.clone());
        deliver_work_item(state, org, project, "git.push", &item);
        return Json(item).into_response();
    }
    match apply_push(state, org, project, repo_id, &payload) {
        PushOutcome::Created(item) => {
            let names = item
                .get("refUpdates")
                .and_then(|value| value.as_array())
                .map(|rows| rows.iter().filter_map(|row| text_of(row, "name")).collect::<Vec<_>>())
                .unwrap_or_default();
            advance_pull_iterations(state, org, project, repo_id, &names);
            deliver_work_item(state, org, project, "git.push", &item);
            Json(item).into_response()
        }
        PushOutcome::Conflict => fail(StatusCode::CONFLICT, "la référence a changé"),
        PushOutcome::Missing => fail(StatusCode::NOT_FOUND, "dépôt introuvable"),
    }
}

fn git_replay(
    state: &AppState,
    method: &Method,
    org: &str,
    project: &str,
    repo_id: &str,
    tail: &[String],
    body: &[u8],
    revert: bool,
) -> Response {
    if find_repo(state, org, project, repo_id).is_none() {
        return fail(StatusCode::NOT_FOUND, "dépôt introuvable");
    }
    let kind = if revert { "revert" } else { "cherry" };
    let resource = if revert { "reverts" } else { "cherryPicks" };
    let key = format!("git/repositories/{repo_id}/{}", if revert { "reverts" } else { "cherrypicks" });
    if let Some(operation_id) = tail.first() {
        if method != Method::GET {
            return fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée");
        }
        return match get_item(state, org, project, &key, operation_id) {
            Some(item) => Json(item).into_response(),
            None => fail(
                StatusCode::NOT_FOUND,
                if revert { "revert introuvable" } else { "cherry-pick introuvable" },
            ),
        };
    }
    if method == Method::GET {
        return Json(page(list(state, org, project, &key))).into_response();
    }
    if method != Method::POST {
        return fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée");
    }
    let payload = parse_json(body).unwrap_or(json!({}));
    let Some(generated) = text_of(&payload, "generatedRefName").filter(|name| name.starts_with("refs/")) else {
        return fail(StatusCode::BAD_REQUEST, "référence absente");
    };
    let Some(onto) = text_of(&payload, "ontoRefName").filter(|name| name.starts_with("refs/")) else {
        return fail(StatusCode::BAD_REQUEST, "branche cible absente");
    };
    let commit_ids = payload
        .get("source")
        .and_then(|source| source.get("commitList"))
        .and_then(|value| value.as_array())
        .map(|rows| {
            rows.iter()
                .filter_map(|row| text_of(row, "commitId").or_else(|| text_of(row, "id")))
                .filter(|id| !id.is_empty())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    if commit_ids.is_empty() {
        return fail(StatusCode::BAD_REQUEST, "commits absents");
    }
    let commits_key = format!("git/repositories/{repo_id}/commits");
    for commit_id in &commit_ids {
        if get_item(state, org, project, &commits_key, commit_id).is_none() {
            return fail(StatusCode::BAD_REQUEST, "commit introuvable");
        }
    }
    if get_item(state, org, project, &format!("git/repositories/{repo_id}/refs"), &generated).is_some() {
        return fail(StatusCode::BAD_REQUEST, "référence existante");
    }
    if get_item(state, org, project, &format!("git/repositories/{repo_id}/refs"), &onto)
        .and_then(|item| text_of(&item, "objectId"))
        .is_none()
    {
        return fail(StatusCode::BAD_REQUEST, "branche cible introuvable");
    }
    let repository_name = repo_name(state, org, project, repo_id);
    let outcome = state.write(|db| {
        let stored = db.project_mut(org, project)?;
        let refs_key = format!("git/repositories/{repo_id}/refs");
        if stored.get_bucket(&refs_key, &generated).is_some() {
            return Some(Err("référence existante"));
        }
        let Some(onto_now) = stored.get_bucket(&refs_key, &onto).and_then(|item| text_of(&item, "objectId")) else {
            return Some(Err("branche cible introuvable"));
        };
        let mut parent = onto_now.clone();
        let mut blobs = stored.list_bucket(&format!("{commits_key}/{onto_now}/blobs"));
        let mut created = Vec::new();
        for (index, commit_id) in commit_ids.iter().enumerate() {
            let Some(source) = stored.get_bucket(&commits_key, commit_id) else {
                return Some(Err("commit introuvable"));
            };
            let original = text_of(&source, "comment").unwrap_or_else(|| "Mise à jour".into());
            let comment = if revert {
                format!("Revert \"{original}\"")
            } else {
                original
            };
            let source_changes = stored.list_bucket(&format!("{commits_key}/{commit_id}/changes"));
            let working = if revert {
                source_changes.iter().map(invert_stored_change).collect::<Vec<_>>()
            } else {
                source_changes
            };
            let (next_blobs, recorded) = apply_onto_blobs(&blobs, &working);
            let new_id = object_id(&format!("{kind}:{repo_id}:{index}:{comment}:{parent}"));
            let item = json!({
                "id": new_id,
                "commitId": new_id,
                "comment": comment,
                "author": {"name": "azurz", "email": "azurz@lab", "date": "2026-10-29T00:00:00Z"},
                "committer": {"name": "azurz", "email": "azurz@lab", "date": "2026-10-29T00:00:00Z"},
                "parents": [parent],
                "changeCounts": change_counts(&recorded),
                "url": format!("/{org}/{project}/_apis/git/repositories/{repo_id}/commits/{new_id}")
            });
            stored.put_bucket(&commits_key, item.clone());
            for change in &recorded {
                stored.put_bucket(&format!("{commits_key}/{new_id}/changes"), change.clone());
            }
            for blob in &next_blobs {
                stored.put_bucket(&format!("{commits_key}/{new_id}/blobs"), blob.clone());
            }
            blobs = next_blobs;
            parent = new_id;
            created.push(item);
        }
        let tip = parent;
        stored.put_bucket(
            &refs_key,
            json!({"id": generated, "name": generated, "objectId": tip, "newObjectId": tip, "success": true}),
        );
        let pushes_key = format!("git/repositories/{repo_id}/pushes");
        let push_id = stored.list_bucket(&pushes_key).len() as i64 + 1;
        let push = json!({
            "id": push_id,
            "pushId": push_id,
            "date": "2026-10-29T00:00:00Z",
            "commits": created,
            "refUpdates": [{"name": generated, "oldObjectId": "0000000000000000000000000000000000000000", "newObjectId": tip}],
            "repository": {"id": repo_id, "name": repository_name},
            "pushedBy": {"displayName": "azurz", "uniqueName": "azurz"},
            "url": format!("/{org}/{project}/_apis/git/repositories/{repo_id}/pushes/{push_id}")
        });
        stored.put_bucket(&pushes_key, push.clone());
        let op_id = stored.list_bucket(&key).len() as i64 + 1;
        let id_name = if revert { "revertId" } else { "cherryPickId" };
        let operation = json!({
            "id": op_id,
            id_name: op_id,
            "parameters": {
                "generatedRefName": generated,
                "ontoRefName": onto,
                "source": {"commitList": commit_ids.iter().map(|id| json!({"commitId": id})).collect::<Vec<_>>()}
            },
            "status": "completed",
            "detailedStatus": {"status": "succeeded", "currentCommitId": tip, "progress": 1.0},
            "url": format!("/{org}/{project}/_apis/git/repositories/{repo_id}/{resource}/{op_id}")
        });
        stored.put_bucket(&key, operation.clone());
        Some(Ok((operation, push, generated.clone())))
    });
    match outcome {
        Some(Ok((operation, push, generated))) => {
            advance_pull_iterations(state, org, project, repo_id, &[generated]);
            deliver_work_item(state, org, project, "git.push", &push);
            Json(operation).into_response()
        }
        Some(Err(message)) => fail(StatusCode::BAD_REQUEST, message),
        None => fail(StatusCode::NOT_FOUND, "dépôt introuvable"),
    }
}

fn invert_stored_change(change: &Value) -> Value {
    let kind = text_of(change, "changeType").unwrap_or_else(|| "add".into()).to_ascii_lowercase();
    let path = change
        .get("item")
        .and_then(|item| text_of(item, "path"))
        .or_else(|| text_of(change, "id"))
        .unwrap_or_else(|| "/fichier.txt".into());
    let original = change.get("originalContent").and_then(|value| text_of(value, "content"));
    let fresh = change.get("newContent").and_then(|value| text_of(value, "content"));
    match kind.as_str() {
        "delete" => json!({
            "changeType": "add",
            "item": {"path": path, "gitObjectType": "blob"},
            "newContent": {"content": original.unwrap_or_default(), "contentType": "rawtext"}
        }),
        "edit" => json!({
            "changeType": "edit",
            "item": {"path": path, "gitObjectType": "blob"},
            "newContent": {"content": original.clone().unwrap_or_default(), "contentType": "rawtext"},
            "originalContent": fresh.as_ref().map(|text| json!({"content": text, "contentType": "rawtext"})).unwrap_or(Value::Null)
        }),
        _ => json!({
            "changeType": "delete",
            "item": {"path": path, "gitObjectType": "blob"},
            "originalContent": fresh.as_ref().map(|text| json!({"content": text, "contentType": "rawtext"})).unwrap_or(Value::Null),
            "newContent": Value::Null
        }),
    }
}

fn apply_onto_blobs(base: &[Value], changes: &[Value]) -> (Vec<Value>, Vec<Value>) {
    let mut files: BTreeMap<String, Value> = BTreeMap::new();
    for blob in base {
        let path = text_of(blob, "path").or_else(|| text_of(blob, "id")).unwrap_or_default();
        if !path.is_empty() {
            files.insert(path, blob.clone());
        }
    }
    if !files.contains_key("/") {
        files.insert(
            "/".into(),
            json!({"id": "/", "path": "/", "isFolder": true, "gitObjectType": "tree"}),
        );
    }
    let mut recorded = Vec::new();
    for change in changes {
        let path = change
            .get("item")
            .and_then(|item| text_of(item, "path"))
            .or_else(|| text_of(change, "id"))
            .unwrap_or_default();
        if path.is_empty() {
            continue;
        }
        let path = if path.starts_with('/') { path } else { format!("/{path}") };
        let mut kind = text_of(change, "changeType").unwrap_or_else(|| "add".into()).to_ascii_lowercase();
        let exists = files.get(&path).is_some_and(|item| item.get("isFolder").and_then(|value| value.as_bool()) != Some(true));
        if kind == "add" && exists {
            kind = "edit".into();
        } else if kind == "edit" && !exists {
            kind = "add".into();
        }
        let original = files.get(&path).and_then(|item| text_of(item, "content"));
        let content = change
            .get("newContent")
            .and_then(|value| text_of(value, "content"))
            .unwrap_or_default();
        if kind == "delete" {
            files.remove(&path);
        } else {
            let parts: Vec<&str> = path.trim_matches('/').split('/').filter(|part| !part.is_empty()).collect();
            if parts.len() > 1 {
                let mut prefix = String::new();
                for part in &parts[..parts.len() - 1] {
                    prefix.push('/');
                    prefix.push_str(part);
                    files.entry(prefix.clone()).or_insert_with(|| {
                        json!({"id": prefix, "path": prefix, "isFolder": true, "gitObjectType": "tree"})
                    });
                }
            }
            files.insert(
                path.clone(),
                json!({
                    "id": path,
                    "path": path,
                    "isFolder": false,
                    "gitObjectType": "blob",
                    "content": content
                }),
            );
        }
        recorded.push(json!({
            "id": path,
            "changeType": kind,
            "item": {"path": path, "gitObjectType": "blob"},
            "originalContent": original.as_ref().map(|text| json!({"content": text, "contentType": "rawtext"})).unwrap_or(Value::Null),
            "newContent": if kind == "delete" { Value::Null } else { json!({"content": content, "contentType": "rawtext"}) }
        }));
    }
    (files.into_values().collect(), recorded)
}

enum PushOutcome {
    Created(Value),
    Conflict,
    Missing,
}

fn apply_push(state: &AppState, org: &str, project: &str, repo_id: &str, payload: &Value) -> PushOutcome {
    let repository_name = repo_name(state, org, project, repo_id);
    state.write(|db| {
        db.ensure_project(org, project);
        let Some(stored) = db.project_mut(org, project) else {
            return PushOutcome::Missing;
        };
        let refs_key = format!("git/repositories/{repo_id}/refs");
        let commits_key = format!("git/repositories/{repo_id}/commits");
        let items_key = format!("git/repositories/{repo_id}/items");
        let pushes_key = format!("git/repositories/{repo_id}/pushes");
        let updates = payload
            .get("refUpdates")
            .and_then(|value| value.as_array())
            .cloned()
            .unwrap_or_else(|| vec![json!({"name": "refs/heads/main"})]);
        for update in &updates {
            let name = text_of(update, "name").unwrap_or_else(|| "refs/heads/main".into());
            let current = stored
                .get_bucket(&refs_key, &name)
                .and_then(|item| text_of(&item, "objectId"));
            if let Some(old) = text_of(update, "oldObjectId") {
                if ref_conflict(current.as_deref(), &old) {
                    return PushOutcome::Conflict;
                }
            }
        }
        let mut parent = updates.first().and_then(|update| {
            let name = text_of(update, "name").unwrap_or_else(|| "refs/heads/main".into());
            stored.get_bucket(&refs_key, &name).and_then(|item| text_of(&item, "objectId"))
        });
        let mut created_commits = Vec::new();
        let commit_rows = payload.get("commits").and_then(|value| value.as_array()).cloned().unwrap_or_default();
        for commit in commit_rows {
            let comment = text_of(&commit, "comment").unwrap_or_else(|| "Mise à jour".into());
            let commit_id = object_id(&format!(
                "{repo_id}:{comment}:{}",
                stored.list_bucket(&commits_key).len()
            ));
            let changes = commit.get("changes").and_then(|value| value.as_array()).cloned().unwrap_or_default();
            let mut stored_changes = Vec::new();
            for change in changes {
                let path = change
                    .get("item")
                    .and_then(|item| text_of(item, "path"))
                    .unwrap_or_else(|| "/fichier.txt".into());
                let path = if path.starts_with('/') { path } else { format!("/{path}") };
                let mut change_type = text_of(&change, "changeType").unwrap_or_else(|| "add".into()).to_ascii_lowercase();
                let original = stored.get_bucket(&items_key, &path).and_then(|item| text_of(&item, "content"));
                let exists = original.is_some() || stored.get_bucket(&items_key, &path).is_some();
                if change_type == "add" && exists {
                    change_type = "edit".into();
                } else if change_type == "edit" && !exists {
                    change_type = "add".into();
                }
                let content = change
                    .get("newContent")
                    .and_then(|value| text_of(value, "content").or_else(|| value.as_str().map(str::to_string)))
                    .unwrap_or_default();
                if change_type.eq_ignore_ascii_case("delete") {
                    if let Some(bucket) = stored.buckets.get_mut(&items_key) {
                        bucket.remove(&path);
                    }
                } else {
                    ensure_tree(stored, &items_key, &path);
                    stored.put_bucket(
                        &items_key,
                        json!({
                            "id": path,
                            "path": path,
                            "isFolder": false,
                            "gitObjectType": "blob",
                            "content": content
                        }),
                    );
                }
                stored_changes.push(json!({
                    "id": path,
                    "changeType": change_type,
                    "item": {"path": path, "gitObjectType": "blob"},
                    "originalContent": original.as_ref().map(|text| json!({"content": text, "contentType": "rawtext"})).unwrap_or(Value::Null),
                    "newContent": if change_type.eq_ignore_ascii_case("delete") { Value::Null } else { json!({"content": content, "contentType": "rawtext"}) }
                }));
            }
            let parents = parent.clone().map(|id| vec![json!(id)]).unwrap_or_default();
            let item = json!({
                "id": commit_id,
                "commitId": commit_id,
                "comment": comment,
                "author": {"name": "azurz", "email": "azurz@lab", "date": "2026-10-29T00:00:00Z"},
                "committer": {"name": "azurz", "email": "azurz@lab", "date": "2026-10-29T00:00:00Z"},
                "parents": parents,
                "changeCounts": change_counts(&stored_changes),
                "url": format!("/{org}/{project}/_apis/git/repositories/{repo_id}/commits/{commit_id}")
            });
            stored.put_bucket(&commits_key, item.clone());
            for change in stored_changes {
                stored.put_bucket(&format!("{commits_key}/{commit_id}/changes"), change);
            }
            let snapshot = stored.list_bucket(&items_key);
            let blob_key = format!("{commits_key}/{commit_id}/blobs");
            for blob in snapshot {
                stored.put_bucket(&blob_key, blob);
            }
            parent = Some(commit_id.clone());
            created_commits.push(item);
        }
        let mut applied = Vec::new();
        for update in updates {
            let name = text_of(&update, "name").unwrap_or_else(|| "refs/heads/main".into());
            let old = text_of(&update, "oldObjectId")
                .unwrap_or_else(|| "0000000000000000000000000000000000000000".into());
            let explicit = text_of(&update, "newObjectId")
                .filter(|value| !value.is_empty() && !value.chars().all(|ch| ch == '0'));
            let object = parent.clone().or(explicit).unwrap_or_else(|| object_id(&format!("{repo_id}:{name}")));
            let item = json!({
                "id": name,
                "name": name,
                "objectId": object,
                "oldObjectId": old,
                "newObjectId": object,
                "success": true
            });
            stored.put_bucket(&refs_key, item.clone());
            applied.push(item);
        }
        let next = stored.list_bucket(&pushes_key).len() as i64 + 1;
        let item = json!({
            "id": next,
            "pushId": next,
            "date": "2026-10-29T00:00:00Z",
            "commits": created_commits,
            "refUpdates": applied,
            "repository": {"id": repo_id, "name": repository_name},
            "pushedBy": {"displayName": "azurz", "uniqueName": "azurz"},
            "url": format!("/{org}/{project}/_apis/git/repositories/{repo_id}/pushes/{next}")
        });
        stored.put_bucket(&pushes_key, item.clone());
        PushOutcome::Created(item)
    })
}

fn ref_conflict(current: Option<&str>, old: &str) -> bool {
    let zeros = old.is_empty() || old.chars().all(|ch| ch == '0');
    if zeros {
        current.is_some()
    } else {
        current != Some(old)
    }
}

fn ensure_tree(stored: &mut crate::store::ProjectData, key: &str, path: &str) {
    let parts: Vec<&str> = path.trim_matches('/').split('/').filter(|part| !part.is_empty()).collect();
    if parts.len() <= 1 {
        return;
    }
    let mut prefix = String::new();
    for part in &parts[..parts.len() - 1] {
        prefix.push('/');
        prefix.push_str(part);
        if stored.get_bucket(key, &prefix).is_none() {
            stored.put_bucket(
                key,
                json!({"id": prefix, "path": prefix, "isFolder": true, "gitObjectType": "tree"}),
            );
        }
    }
}

fn change_counts(changes: &[Value]) -> Value {
    let mut add = 0;
    let mut edit = 0;
    let mut delete = 0;
    for change in changes {
        match text_of(change, "changeType").unwrap_or_default().to_ascii_lowercase().as_str() {
            "delete" => delete += 1,
            "edit" => edit += 1,
            _ => add += 1,
        }
    }
    json!({"Add": add, "Edit": edit, "Delete": delete})
}

fn tail_after<'a>(rest: &'a [String], name: &str) -> &'a [String] {
    rest.iter()
        .position(|part| part.eq_ignore_ascii_case(name))
        .map(|index| &rest[index + 1..])
        .unwrap_or(&[])
}

fn diffs(state: &AppState, org: &str, project: &str, repo_id: &str, query: &str) -> Response {
    let base_version = query_param(query, "baseVersion").map(crate::http::pct_decode).unwrap_or_default();
    let target_version = query_param(query, "targetVersion").map(crate::http::pct_decode).unwrap_or_default();
    if base_version.is_empty() || target_version.is_empty() {
        return fail(StatusCode::BAD_REQUEST, "versions absentes");
    }
    let base_type = query_param(query, "baseVersionType").map(crate::http::pct_decode).unwrap_or_default();
    let target_type = query_param(query, "targetVersionType").map(crate::http::pct_decode).unwrap_or_default();
    let Some(base) = resolve_version(state, org, project, repo_id, &base_version, &base_type) else {
        return fail(StatusCode::NOT_FOUND, "commit de base introuvable");
    };
    let Some(target) = resolve_version(state, org, project, repo_id, &target_version, &target_type) else {
        return fail(StatusCode::NOT_FOUND, "commit cible introuvable");
    };
    let chain = commits_after(state, org, project, repo_id, &base, &target);
    let mut nets: Vec<(String, String, Value, Value)> = Vec::new();
    for commit_id in &chain {
        for change in list(state, org, project, &format!("git/repositories/{repo_id}/commits/{commit_id}/changes")) {
            let path = change.get("item").and_then(|item| text_of(item, "path")).unwrap_or_else(|| value_key(&change["id"]));
            if path.is_empty() {
                continue;
            }
            let kind = text_of(&change, "changeType").unwrap_or_else(|| "add".into()).to_ascii_lowercase();
            let original = change.get("originalContent").cloned().unwrap_or(Value::Null);
            let fresh = change.get("newContent").cloned().unwrap_or(Value::Null);
            if let Some(index) = nets.iter().position(|(stored, _, _, _)| stored == &path) {
                let previous = nets[index].1.clone();
                if previous == "add" && kind == "delete" {
                    nets.remove(index);
                    continue;
                }
                let next_kind = if previous == "add" {
                    "add".to_string()
                } else if previous == "delete" && kind == "add" {
                    "edit".to_string()
                } else if kind == "delete" {
                    "delete".to_string()
                } else {
                    previous.clone()
                };
                let kept_original = if previous == "add" || !nets[index].2.is_null() {
                    nets[index].2.clone()
                } else {
                    original
                };
                nets[index] = (path, next_kind, kept_original, fresh);
            } else {
                nets.push((path, kind, original, fresh));
            }
        }
    }
    let changes: Vec<Value> = nets
        .into_iter()
        .map(|(path, kind, original, fresh)| {
            json!({
                "changeType": kind,
                "item": {"path": path, "gitObjectType": "blob"},
                "originalContent": original,
                "newContent": fresh
            })
        })
        .collect();
    Json(json!({
        "allChangesIncluded": true,
        "changeCounts": change_counts(&changes),
        "changes": changes,
        "commonCommit": base,
        "baseCommit": base,
        "targetCommit": target,
        "aheadCount": chain.len(),
        "behindCount": 0
    }))
    .into_response()
}

fn resolve_version(state: &AppState, org: &str, project: &str, repo_id: &str, version: &str, kind: &str) -> Option<String> {
    let branch = kind.eq_ignore_ascii_case("branch") || kind.eq_ignore_ascii_case("tag") || version.starts_with("refs/");
    if branch {
        let name = if version.starts_with("refs/") {
            version.to_string()
        } else {
            format!("refs/heads/{version}")
        };
        return get_item(state, org, project, &format!("git/repositories/{repo_id}/refs"), &name).and_then(|item| text_of(&item, "objectId"));
    }
    if get_item(state, org, project, &format!("git/repositories/{repo_id}/commits"), version).is_some() {
        Some(version.to_string())
    } else {
        None
    }
}

fn commits_after(state: &AppState, org: &str, project: &str, repo_id: &str, base: &str, target: &str) -> Vec<String> {
    if base == target {
        return Vec::new();
    }
    let mut cursor = target.to_string();
    let mut stack = Vec::new();
    for _ in 0..64 {
        if cursor == base {
            break;
        }
        stack.push(cursor.clone());
        let Some(commit) = get_item(state, org, project, &format!("git/repositories/{repo_id}/commits"), &cursor) else {
            break;
        };
        let Some(parent) = commit
            .get("parents")
            .and_then(|value| value.as_array())
            .and_then(|parents| parents.first())
            .map(value_key)
            .filter(|parent| !parent.is_empty())
        else {
            break;
        };
        cursor = parent;
    }
    stack.reverse();
    stack
}

fn items(state: &AppState, org: &str, project: &str, repo_id: &str, query: &str) -> Response {
    let version = query_param(query, "versionDescriptor.version").map(crate::http::pct_decode);
    let version_type = query_param(query, "versionDescriptor.versionType").map(crate::http::pct_decode).unwrap_or_default();
    let key = if let Some(version) = version.as_deref() {
        let Some(commit) = resolve_version(state, org, project, repo_id, version, &version_type) else {
            return fail(StatusCode::NOT_FOUND, "commit introuvable");
        };
        format!("git/repositories/{repo_id}/commits/{commit}/blobs")
    } else {
        format!("git/repositories/{repo_id}/items")
    };
    let rows = list(state, org, project, &key);
    let path = query_param(query, "path").map(|value| crate::http::pct_decode(value));
    let recursion = query_param(query, "recursionLevel").is_some();
    if recursion || path.is_none() || path.as_deref() == Some("/") {
        let scope = path.unwrap_or_else(|| "/".into());
        let value = if scope == "/" {
            rows
        } else {
            rows.into_iter()
                .filter(|item| text_of(item, "path").unwrap_or_default().starts_with(&scope))
                .collect()
        };
        return Json(page(value)).into_response();
    }
    let wanted = path.unwrap_or_default();
    match rows.into_iter().find(|item| text_of(item, "path").as_deref() == Some(wanted.as_str())) {
        Some(item) => Json(item).into_response(),
        None => fail(StatusCode::NOT_FOUND, "fichier introuvable"),
    }
}

fn pull_requests(
    state: &AppState,
    method: &Method,
    org: &str,
    project: &str,
    repo_id: Option<&str>,
    rest: &[String],
    body: &[u8],
) -> Response {
    let key = "git/pullrequests";
    if let Some((kind, pull_id, tail)) = pull_child(rest) {
        return pull_child_resource(state, method, org, project, &kind, &pull_id, tail, body);
    }
    let listing = method == Method::GET && rest.last().is_some_and(|part| part.eq_ignore_ascii_case("pullrequests"));
    if listing {
        let repo_filter = repo_id.map(str::to_string);
        let value = list(state, org, project, key)
            .into_iter()
            .filter(|item| match &repo_filter {
                Some(id) => text_of(&item["repository"], "id").as_deref() == Some(id.as_str()),
                None => true,
            })
            .collect();
        return Json(page(value)).into_response();
    }
    if method == Method::GET {
        let id = rest.last().map(String::as_str).unwrap_or("");
        return match get_item(state, org, project, key, id) {
            Some(item) => Json(item).into_response(),
            None => fail(StatusCode::NOT_FOUND, "demande de tirage introuvable"),
        };
    }
    if method != Method::POST {
        if method == Method::PATCH || method == Method::PUT {
            let id = rest.last().cloned().unwrap_or_default();
            let payload = parse_json(body).unwrap_or(json!({}));
            if let Some(strategy) = payload
                .get("completionOptions")
                .and_then(|options| text_of(options, "mergeStrategy"))
            {
                if !known_merge_strategy(&strategy) {
                    return fail(StatusCode::BAD_REQUEST, "stratégie de fusion inconnue");
                }
            }
            let previous = get_item(state, org, project, key, &id).and_then(|item| text_of(&item, "status"));
            return match patch_item(state, org, project, key, &id, &payload) {
                Some(item) => {
                    let completed = text_of(&item, "status").as_deref() == Some("completed");
                    let item = if completed && previous.as_deref() != Some("completed") {
                        let prepared = if squash_requested(&item) {
                            Some(Prepared::Squash(prepare_squash(state, org, project, &item)))
                        } else if rebase_requested(&item) {
                            Some(Prepared::Rebase(prepare_rebase(state, org, project, &item)))
                        } else {
                            None
                        };
                        finish_pull(state, org, project, &id, prepared).unwrap_or(item)
                    } else if completed {
                        finish_pull(state, org, project, &id, None).unwrap_or(item)
                    } else {
                        item
                    };
                    if completed && previous.as_deref() != Some("completed") {
                        deliver_work_item(state, org, project, "git.pullrequest.merged", &item);
                        if let Some(repo) = text_of(&item["repository"], "id") {
                            if let Some(target) = text_of(&item, "targetRefName") {
                                advance_pull_iterations(state, org, project, &repo, &[target]);
                            }
                        }
                    } else if !completed {
                        deliver_work_item(state, org, project, "git.pullrequest.updated", &item);
                    }
                    Json(item).into_response()
                }
                None => fail(StatusCode::NOT_FOUND, "demande de tirage introuvable"),
            };
        }
        return fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée");
    }
    let payload = parse_json(body).unwrap_or(json!({}));
    let title = text_of(&payload, "title")
        .or_else(|| text_of(&payload, "name"))
        .unwrap_or_else(|| "Demande de tirage".into());
    let source = text_of(&payload, "sourceRefName").unwrap_or_else(|| "refs/heads/feature".into());
    let target = text_of(&payload, "targetRefName").unwrap_or_else(|| "refs/heads/main".into());
    let description = text_of(&payload, "description").unwrap_or_default();
    let repository_id = repo_id
        .map(str::to_string)
        .or_else(|| text_of(&payload["repository"], "id"))
        .or_else(|| default_repo_id(state, org, project))
        .unwrap_or_default();
    let repository_name = repo_name(state, org, project, &repository_id);
    let next = list(state, org, project, key).len() as i64 + 1;
    let item = json!({
        "id": next,
        "pullRequestId": next,
        "codeReviewId": next,
        "status": payload.get("status").cloned().unwrap_or(json!("active")),
        "title": title,
        "description": description,
        "sourceRefName": source,
        "targetRefName": target,
        "mergeStatus": "succeeded",
        "isDraft": payload.get("isDraft").cloned().unwrap_or(json!(false)),
        "repository": {"id": repository_id, "name": repository_name},
        "createdBy": {"displayName": "azurz", "uniqueName": "azurz"},
        "creationDate": "2026-10-29T00:00:00Z",
        "url": format!("/{org}/{project}/_apis/git/repositories/{repository_id}/pullrequests/{next}")
    });
    put_item(state, org, project, key, item.clone());
    remember_iteration(state, org, project, &item);
    deliver_work_item(state, org, project, "git.pullrequest.created", &item);
    Json(item).into_response()
}

struct PullTips {
    source_id: Option<String>,
    target_id: Option<String>,
    common_id: Option<String>,
    ahead: Vec<Value>,
}

fn pull_tips(state: &AppState, org: &str, project: &str, pull: &Value) -> PullTips {
    let repo_id = text_of(&pull["repository"], "id").unwrap_or_default();
    let source_name = text_of(pull, "sourceRefName").unwrap_or_else(|| "refs/heads/feature".into());
    let target_name = text_of(pull, "targetRefName").unwrap_or_else(|| "refs/heads/main".into());
    let refs = list(state, org, project, &format!("git/repositories/{repo_id}/refs"));
    let object_of = |name: &str| {
        refs.iter()
            .find(|item| text_of(item, "name").as_deref() == Some(name))
            .and_then(|item| text_of(item, "objectId"))
    };
    let frozen_source = text_of(&pull["lastMergeSourceCommit"], "commitId").filter(|id| !id.is_empty());
    let frozen_target = text_of(&pull["lastMergeTargetCommit"], "commitId").filter(|id| !id.is_empty());
    let (source_id, target_id) = if text_of(pull, "status").as_deref() == Some("completed")
        && frozen_source.is_some()
        && frozen_target.is_some()
    {
        (frozen_source, frozen_target)
    } else {
        (
            object_of(&source_name).or(frozen_source),
            object_of(&target_name).or(frozen_target),
        )
    };
    tips_from_commits(state, org, project, &repo_id, source_id, target_id)
}

fn tips_from_commits(
    state: &AppState,
    org: &str,
    project: &str,
    repo_id: &str,
    source_id: Option<String>,
    target_id: Option<String>,
) -> PullTips {
    let commits = list(state, org, project, &format!("git/repositories/{repo_id}/commits"));
    let mut by_id = std::collections::BTreeMap::new();
    for commit in commits {
        let id = text_of(&commit, "commitId").or_else(|| text_of(&commit, "id")).unwrap_or_default();
        if !id.is_empty() {
            by_id.insert(id, commit);
        }
    }
    let blocked = commits_reachable(target_id.as_deref(), &by_id);
    let ahead = commits_ahead(source_id.as_deref(), &blocked, &by_id);
    let common_id = if ahead.is_empty() {
        source_id.clone()
    } else {
        ahead.last().and_then(|commit| {
            commit
                .get("parents")
                .and_then(|value| value.as_array())
                .and_then(|parents| parents.first())
                .and_then(|parent| parent.as_str())
                .map(str::to_string)
        })
    };
    PullTips {
        source_id,
        target_id,
        common_id,
        ahead,
    }
}

fn iteration_bucket(pull_id: &str) -> String {
    format!("git/pullrequests/{pull_id}/iterations")
}

fn remember_iteration(state: &AppState, org: &str, project: &str, pull: &Value) {
    let Some(pull_id) = pull.get("pullRequestId").map(value_key).filter(|id| !id.is_empty()) else {
        return;
    };
    let tips = pull_tips(state, org, project, pull);
    let key = iteration_bucket(&pull_id);
    let existing = list(state, org, project, &key);
    let source = tips.source_id.clone().unwrap_or_default();
    let target = tips.target_id.clone().unwrap_or_default();
    if let Some(last) = existing.iter().max_by_key(|item| item.get("id").and_then(|value| value.as_i64()).unwrap_or(0)) {
        let same_source = text_of(&last["sourceRefCommit"], "commitId").as_deref() == Some(source.as_str());
        let same_target = text_of(&last["targetRefCommit"], "commitId").as_deref() == Some(target.as_str());
        if same_source && same_target {
            return;
        }
    }
    let id = existing.len() as i64 + 1;
    put_item(state, org, project, &key, iteration_document(pull, &tips, id));
}

fn advance_pull_iterations(state: &AppState, org: &str, project: &str, repo_id: &str, ref_names: &[String]) {
    if ref_names.is_empty() {
        return;
    }
    for pull in list(state, org, project, "git/pullrequests") {
        if text_of(&pull["repository"], "id").as_deref() != Some(repo_id) {
            continue;
        }
        let status = text_of(&pull, "status").unwrap_or_default();
        if status == "completed" || status == "abandoned" {
            continue;
        }
        let source = text_of(&pull, "sourceRefName").unwrap_or_default();
        let target = text_of(&pull, "targetRefName").unwrap_or_default();
        if !ref_names.iter().any(|name| name == &source || name == &target) {
            continue;
        }
        remember_iteration(state, org, project, &pull);
    }
}

fn pull_commits(state: &AppState, method: &Method, org: &str, project: &str, pull_id: &str) -> Response {
    if method != Method::GET {
        return fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée");
    }
    let Some(pull) = get_item(state, org, project, "git/pullrequests", pull_id) else {
        return fail(StatusCode::NOT_FOUND, "demande de tirage introuvable");
    };
    Json(page(pull_tips(state, org, project, &pull).ahead)).into_response()
}

fn iteration_document(pull: &Value, tips: &PullTips, id: i64) -> Value {
    json!({
        "id": id,
        "description": format!("Iteration {id}"),
        "sourceRefName": text_of(pull, "sourceRefName").unwrap_or_else(|| "refs/heads/feature".into()),
        "targetRefName": text_of(pull, "targetRefName").unwrap_or_else(|| "refs/heads/main".into()),
        "sourceRefCommit": {"commitId": tips.source_id.clone().unwrap_or_default()},
        "targetRefCommit": {"commitId": tips.target_id.clone().unwrap_or_default()},
        "commonRefCommit": {"commitId": tips.common_id.clone().unwrap_or_default()}
    })
}

fn stored_iterations(state: &AppState, org: &str, project: &str, pull: &Value, pull_id: &str) -> Vec<Value> {
    let mut rows = list(state, org, project, &iteration_bucket(pull_id));
    if rows.is_empty() {
        rows.push(iteration_document(pull, &pull_tips(state, org, project, pull), 1));
    }
    rows.sort_by_key(|item| item.get("id").and_then(|value| value.as_i64()).unwrap_or(0));
    rows
}

fn pull_iterations(state: &AppState, method: &Method, org: &str, project: &str, pull_id: &str, tail: &[String]) -> Response {
    if method != Method::GET {
        return fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée");
    }
    let Some(pull) = get_item(state, org, project, "git/pullrequests", pull_id) else {
        return fail(StatusCode::NOT_FOUND, "demande de tirage introuvable");
    };
    let rows = stored_iterations(state, org, project, &pull, pull_id);
    if tail.is_empty() {
        return Json(page(rows)).into_response();
    }
    let Ok(wanted) = tail[0].parse::<i64>() else {
        return fail(StatusCode::NOT_FOUND, "itération introuvable");
    };
    let Some(iteration) = rows.into_iter().find(|item| item.get("id").and_then(|value| value.as_i64()) == Some(wanted)) else {
        return fail(StatusCode::NOT_FOUND, "itération introuvable");
    };
    if tail.len() == 1 {
        return Json(iteration).into_response();
    }
    if tail.len() == 2 && tail[1].eq_ignore_ascii_case("changes") {
        let repo_id = text_of(&pull["repository"], "id").unwrap_or_default();
        let tips = tips_from_commits(
            state,
            org,
            project,
            &repo_id,
            text_of(&iteration["sourceRefCommit"], "commitId"),
            text_of(&iteration["targetRefCommit"], "commitId"),
        );
        return Json(json!({"changeEntries": net_change_entries(state, org, project, &repo_id, &tips.ahead)})).into_response();
    }
    fail(StatusCode::NOT_FOUND, "itération introuvable")
}

fn net_change_entries(state: &AppState, org: &str, project: &str, repo_id: &str, ahead_newest_first: &[Value]) -> Vec<Value> {
    let mut nets: Vec<(String, String)> = Vec::new();
    for commit in ahead_newest_first.iter().rev() {
        let commit_id = text_of(commit, "commitId").or_else(|| text_of(commit, "id")).unwrap_or_default();
        if commit_id.is_empty() {
            continue;
        }
        for change in list(state, org, project, &format!("git/repositories/{repo_id}/commits/{commit_id}/changes")) {
            let path = change
                .get("item")
                .and_then(|item| text_of(item, "path"))
                .unwrap_or_else(|| value_key(&change["id"]));
            if path.is_empty() {
                continue;
            }
            let kind = text_of(&change, "changeType").unwrap_or_else(|| "add".into()).to_ascii_lowercase();
            if let Some(index) = nets.iter().position(|(stored, _)| stored == &path) {
                let previous = nets[index].1.clone();
                if previous == "add" && kind == "delete" {
                    nets.remove(index);
                    continue;
                }
                nets[index].1 = if previous == "add" {
                    "add".to_string()
                } else if previous == "delete" && kind == "add" {
                    "edit".to_string()
                } else if kind == "delete" {
                    "delete".to_string()
                } else {
                    previous
                };
            } else {
                nets.push((path, kind));
            }
        }
    }
    nets.into_iter()
        .enumerate()
        .map(|(index, (path, kind))| {
            let id = index as i64 + 1;
            json!({
                "changeTrackingId": id,
                "changeId": id,
                "item": {"path": path},
                "changeType": kind
            })
        })
        .collect()
}

fn commits_reachable(tip: Option<&str>, commits: &std::collections::BTreeMap<String, Value>) -> std::collections::BTreeSet<String> {
    let mut seen = std::collections::BTreeSet::new();
    let mut stack = Vec::new();
    if let Some(id) = tip {
        stack.push(id.to_string());
    }
    while let Some(id) = stack.pop() {
        if !seen.insert(id.clone()) {
            continue;
        }
        let Some(commit) = commits.get(&id) else {
            continue;
        };
        let Some(parents) = commit.get("parents").and_then(|value| value.as_array()) else {
            continue;
        };
        for parent in parents {
            if let Some(text) = parent.as_str() {
                stack.push(text.to_string());
            }
        }
    }
    seen
}

fn commits_ahead(
    tip: Option<&str>,
    blocked: &std::collections::BTreeSet<String>,
    commits: &std::collections::BTreeMap<String, Value>,
) -> Vec<Value> {
    let mut ahead = Vec::new();
    let mut current = tip.map(str::to_string);
    let mut guard = 0;
    while let Some(id) = current {
        if blocked.contains(&id) || guard > 1000 {
            break;
        }
        guard += 1;
        let Some(commit) = commits.get(&id) else {
            break;
        };
        ahead.push(commit.clone());
        current = commit
            .get("parents")
            .and_then(|value| value.as_array())
            .and_then(|parents| parents.first())
            .and_then(|parent| parent.as_str())
            .map(str::to_string);
    }
    ahead
}

fn pull_child(rest: &[String]) -> Option<(String, String, &[String])> {
    let index = rest.iter().position(|part| {
        matches!(
            part.to_ascii_lowercase().as_str(),
            "threads" | "statuses" | "iterations" | "reviewers" | "workitems" | "commits" | "labels"
        )
    })?;
    let id = rest.get(index.checked_sub(1)?)?.clone();
    if id.eq_ignore_ascii_case("pullrequests") || id.eq_ignore_ascii_case("repositories") {
        return None;
    }
    Some((rest[index].to_ascii_lowercase(), id, &rest[index + 1..]))
}

fn pull_child_resource(
    state: &AppState,
    method: &Method,
    org: &str,
    project: &str,
    kind: &str,
    pull_id: &str,
    tail: &[String],
    body: &[u8],
) -> Response {
    let key = format!("git/pullrequests/{pull_id}/{kind}");
    if kind == "workitems" {
        return pull_work_items(state, method, org, project, pull_id);
    }
    if kind == "commits" {
        return pull_commits(state, method, org, project, pull_id);
    }
    if kind == "reviewers" {
        return reviewers(state, method, org, project, pull_id, body);
    }
    if kind == "iterations" {
        return pull_iterations(state, method, org, project, pull_id, tail);
    }
    if kind == "labels" {
        return pull_labels(state, method, org, project, pull_id, tail, body);
    }
    if method == Method::GET {
        return Json(page(list(state, org, project, &key))).into_response();
    }
    if method != Method::POST {
        return fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée");
    }
    let payload = parse_json(body).unwrap_or(json!({}));
    let next = list(state, org, project, &key).len() as i64 + 1;
    let item = if kind == "threads" {
        let comments = payload
            .get("comments")
            .and_then(|value| value.as_array())
            .cloned()
            .unwrap_or_else(|| vec![json!({"content": text_of(&payload, "content").unwrap_or_default()})]);
        let comments: Vec<Value> = comments
            .into_iter()
            .enumerate()
            .map(|(index, mut comment)| {
                if let Some(object) = comment.as_object_mut() {
                    object.entry("id").or_insert(json!(index as i64 + 1));
                    object.entry("commentType").or_insert(json!("text"));
                    object.entry("author").or_insert(json!({"displayName": "azurz", "uniqueName": "azurz"}));
                }
                comment
            })
            .collect();
        json!({
            "id": next,
            "publishedDate": "2026-10-29T00:00:00Z",
            "lastUpdatedDate": "2026-10-29T00:00:00Z",
            "status": payload.get("status").cloned().unwrap_or(json!("active")),
            "comments": comments
        })
    } else {
        json!({
            "id": next,
            "state": payload.get("state").cloned().unwrap_or(json!("succeeded")),
            "description": text_of(&payload, "description").unwrap_or_default(),
            "context": payload.get("context").cloned().unwrap_or(json!({"name": "azurz", "genre": "continuous-integration"})),
            "creationDate": "2026-10-29T00:00:00Z",
            "updatedDate": "2026-10-29T00:00:00Z"
        })
    };
    put_item(state, org, project, &key, item.clone());
    Json(item).into_response()
}

fn repo_labels(state: &AppState, method: &Method, org: &str, project: &str, repo_id: &str, tail: &[String]) -> Response {
    if find_repo(state, org, project, repo_id).is_none() {
        return fail(StatusCode::NOT_FOUND, "dépôt introuvable");
    }
    let key = format!("git/repositories/{repo_id}/labels");
    if let Some(label_id) = tail.first() {
        if method != Method::GET {
            return fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée");
        }
        return match label_named(&list(state, org, project, &key), label_id) {
            Some(item) => Json(item).into_response(),
            None => fail(StatusCode::NOT_FOUND, "étiquette introuvable"),
        };
    }
    if method != Method::GET {
        return fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée");
    }
    Json(page(list(state, org, project, &key))).into_response()
}

fn pull_labels(
    state: &AppState,
    method: &Method,
    org: &str,
    project: &str,
    pull_id: &str,
    tail: &[String],
    body: &[u8],
) -> Response {
    let Some(pull) = get_item(state, org, project, "git/pullrequests", pull_id) else {
        return fail(StatusCode::NOT_FOUND, "demande de tirage introuvable");
    };
    let repo_id = text_of(&pull["repository"], "id").unwrap_or_default();
    let key = format!("git/repositories/{repo_id}/pullrequests/{pull_id}/labels");
    let catalog = format!("git/repositories/{repo_id}/labels");
    if let Some(label_id) = tail.first() {
        let found = label_named(&list(state, org, project, &key), label_id);
        if method == Method::GET {
            return match found {
                Some(item) => Json(item).into_response(),
                None => fail(StatusCode::NOT_FOUND, "étiquette introuvable"),
            };
        }
        if method != Method::DELETE {
            return fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée");
        }
        let Some(found) = found else {
            return fail(StatusCode::NOT_FOUND, "étiquette introuvable");
        };
        let stored_id = text_of(&found, "id").unwrap_or_default();
        state.write(|db| {
            if let Some(stored) = db.project_mut(org, project) {
                if let Some(bucket) = stored.buckets.get_mut(&key) {
                    bucket.remove(&stored_id);
                }
            }
        });
        if text_of(&pull, "status").as_deref() != Some("completed") {
            deliver_work_item(state, org, project, "git.pullrequest.updated", &pull);
        }
        return StatusCode::NO_CONTENT.into_response();
    }
    if method == Method::GET {
        return Json(page(list(state, org, project, &key))).into_response();
    }
    if method != Method::POST {
        return fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée");
    }
    let payload = parse_json(body).unwrap_or(json!({}));
    let Some(name) = text_of(&payload, "name").map(|value| value.trim().to_string()).filter(|value| !value.is_empty()) else {
        return fail(StatusCode::BAD_REQUEST, "nom d'étiquette absent");
    };
    if let Some(existing) = label_named(&list(state, org, project, &key), &name) {
        return Json(existing).into_response();
    }
    let label = if let Some(existing) = label_named(&list(state, org, project, &catalog), &name) {
        existing
    } else {
        let id = object_id(&format!("{repo_id}:label:{}", name.to_lowercase()));
        let item = json!({
            "id": id,
            "name": name,
            "active": true,
            "url": format!("/{org}/{project}/_apis/git/repositories/{repo_id}/labels/{id}")
        });
        put_item(state, org, project, &catalog, item.clone());
        item
    };
    put_item(state, org, project, &key, label.clone());
    if text_of(&pull, "status").as_deref() != Some("completed") {
        deliver_work_item(state, org, project, "git.pullrequest.updated", &pull);
    }
    Json(label).into_response()
}

fn label_named(rows: &[Value], wanted: &str) -> Option<Value> {
    rows.iter()
        .find(|item| {
            text_of(item, "id").as_deref() == Some(wanted)
                || text_of(item, "name").is_some_and(|name| name.eq_ignore_ascii_case(wanted))
        })
        .cloned()
}

fn pull_work_items(state: &AppState, method: &Method, org: &str, project: &str, pull_id: &str) -> Response {
    if method != Method::GET {
        return fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée");
    }
    let Some(pull) = get_item(state, org, project, "git/pullrequests", pull_id) else {
        return fail(StatusCode::NOT_FOUND, "demande de tirage introuvable");
    };
    let repo_id = text_of(&pull["repository"], "id").unwrap_or_default();
    let project_id = state.read(|db| db.project(org, project).map(|stored| stored.id.clone())).unwrap_or_default();
    let linked = state.read(|db| {
        db.project(org, project)
            .map(|stored| {
                stored
                    .work_items
                    .values()
                    .filter(|item| work_item_links_pull(item, project, &project_id, &repo_id, pull_id))
                    .filter_map(|item| item.get("id").and_then(|value| value.as_i64()))
                    .map(|id| {
                        json!({
                            "id": id.to_string(),
                            "url": format!("/{org}/{project}/_apis/wit/workitems/{id}")
                        })
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default()
    });
    Json(page(linked)).into_response()
}

fn work_item_links_pull(item: &Value, project: &str, project_id: &str, repo_id: &str, pull_id: &str) -> bool {
    item.get("relations")
        .and_then(|value| value.as_array())
        .is_some_and(|relations| {
            relations.iter().any(|relation| {
                relation.get("rel").and_then(|value| value.as_str()) == Some("ArtifactLink")
                    && relation
                        .get("url")
                        .and_then(|value| value.as_str())
                        .is_some_and(|url| artifact_targets_pull(url, project, project_id, repo_id, pull_id))
            })
        })
}

fn artifact_targets_pull(url: &str, project: &str, project_id: &str, repo_id: &str, pull_id: &str) -> bool {
    let decoded = pct_decode(url);
    let lower = decoded.to_ascii_lowercase();
    let Some(index) = lower.find("pullrequestid/") else {
        return false;
    };
    let rest = &decoded[index + "pullrequestid/".len()..];
    let parts: Vec<&str> = rest.split('/').filter(|part| !part.is_empty()).collect();
    if parts.len() < 3 || repo_id.is_empty() {
        return false;
    }
    let project_ok = parts[0].eq_ignore_ascii_case(project) || parts[0] == project_id;
    let repo_ok = parts[1].eq_ignore_ascii_case(repo_id);
    let pull_ok = parts[2] == pull_id;
    project_ok && repo_ok && pull_ok
}

fn reviewers(state: &AppState, method: &Method, org: &str, project: &str, pull_id: &str, body: &[u8]) -> Response {
    let key = format!("git/pullrequests/{pull_id}/reviewers");
    if method == Method::GET {
        return Json(Value::Array(list(state, org, project, &key))).into_response();
    }
    if method != Method::POST && method != Method::PUT {
        return fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée");
    }
    let payload = parse_json(body).unwrap_or(json!({}));
    let rows = payload.as_array().cloned().unwrap_or_else(|| vec![payload]);
    let mut stored = Vec::new();
    for row in rows {
        let name = text_of(&row, "displayName")
            .or_else(|| text_of(&row, "uniqueName"))
            .or_else(|| text_of(&row, "id"))
            .unwrap_or_else(|| "azurz".into());
        let vote = row.get("vote").and_then(|value| value.as_i64()).unwrap_or(0);
        let item = json!({
            "id": name,
            "displayName": name,
            "uniqueName": text_of(&row, "uniqueName").unwrap_or_else(|| name.clone()),
            "vote": vote,
            "isRequired": row.get("isRequired").cloned().unwrap_or(json!(false)),
            "isContainer": false
        });
        put_item(state, org, project, &key, item.clone());
        stored.push(item);
    }
    if let Some(pull) = get_item(state, org, project, "git/pullrequests", pull_id) {
        if text_of(&pull, "status").as_deref() != Some("completed") {
            deliver_work_item(state, org, project, "git.pullrequest.updated", &pull);
        }
    }
    if stored.len() == 1 {
        Json(stored.pop().unwrap_or(json!({}))).into_response()
    } else {
        Json(Value::Array(stored)).into_response()
    }
}

enum Prepared {
    Squash(SquashPrep),
    Rebase(RebasePrep),
}

struct SquashPrep {
    changes: Vec<Value>,
    message: String,
    delete_source: bool,
}

struct RebasePrep {
    replay: Vec<ReplayCommit>,
    delete_source: bool,
    merge_after: bool,
    merge_message: String,
}

struct ReplayCommit {
    comment: String,
    changes: Vec<Value>,
}

fn known_merge_strategy(strategy: &str) -> bool {
    matches!(
        strategy.to_ascii_lowercase().as_str(),
        "squash" | "nofastforward" | "rebase" | "rebasemerge"
    )
}

fn rebase_requested(pull: &Value) -> bool {
    text_of(&pull["completionOptions"], "mergeStrategy").is_some_and(|strategy| {
        strategy.eq_ignore_ascii_case("rebase") || strategy.eq_ignore_ascii_case("rebaseMerge")
    })
}

fn squash_requested(pull: &Value) -> bool {
    let options = &pull["completionOptions"];
    text_of(options, "mergeStrategy").is_some_and(|strategy| strategy.eq_ignore_ascii_case("squash"))
        || options.get("squashMerge").and_then(|value| value.as_bool()) == Some(true)
}

fn blob_content(state: &AppState, org: &str, project: &str, repo_id: &str, commit_id: Option<&str>, path: &str) -> Option<String> {
    let commit_id = commit_id?;
    get_item(
        state,
        org,
        project,
        &format!("git/repositories/{repo_id}/commits/{commit_id}/blobs"),
        path,
    )
    .and_then(|item| text_of(&item, "content"))
}

fn prepare_squash(state: &AppState, org: &str, project: &str, pull: &Value) -> SquashPrep {
    let repo_id = text_of(&pull["repository"], "id").unwrap_or_default();
    let tips = pull_tips(state, org, project, pull);
    let nets = net_change_entries(state, org, project, &repo_id, &tips.ahead);
    let changes = nets
        .into_iter()
        .map(|entry| {
            let path = entry
                .get("item")
                .and_then(|item| text_of(item, "path"))
                .unwrap_or_default();
            let kind = text_of(&entry, "changeType").unwrap_or_else(|| "add".into());
            let source_content = blob_content(state, org, project, &repo_id, tips.source_id.as_deref(), &path);
            let target_content = blob_content(state, org, project, &repo_id, tips.target_id.as_deref(), &path);
            json!({
                "id": path,
                "changeType": kind,
                "item": {"path": path, "gitObjectType": "blob"},
                "originalContent": target_content.as_ref().map(|text| json!({"content": text, "contentType": "rawtext"})).unwrap_or(Value::Null),
                "newContent": if kind.eq_ignore_ascii_case("delete") {
                    Value::Null
                } else {
                    json!({"content": source_content.unwrap_or_default(), "contentType": "rawtext"})
                }
            })
        })
        .collect();
    let message = text_of(&pull["completionOptions"], "mergeCommitMessage")
        .filter(|text| !text.is_empty())
        .or_else(|| text_of(pull, "title"))
        .unwrap_or_else(|| "Squash".into());
    let delete_source = pull["completionOptions"].get("deleteSourceBranch").and_then(|value| value.as_bool()) == Some(true);
    SquashPrep {
        changes,
        message,
        delete_source,
    }
}

fn prepare_rebase(state: &AppState, org: &str, project: &str, pull: &Value) -> RebasePrep {
    let repo_id = text_of(&pull["repository"], "id").unwrap_or_default();
    let tips = pull_tips(state, org, project, pull);
    let mut replay = Vec::new();
    for commit in tips.ahead.iter().rev() {
        let commit_id = text_of(commit, "commitId").or_else(|| text_of(commit, "id")).unwrap_or_default();
        if commit_id.is_empty() {
            continue;
        }
        replay.push(ReplayCommit {
            comment: text_of(commit, "comment").unwrap_or_else(|| "Mise à jour".into()),
            changes: list(
                state,
                org,
                project,
                &format!("git/repositories/{repo_id}/commits/{commit_id}/changes"),
            ),
        });
    }
    let strategy = text_of(&pull["completionOptions"], "mergeStrategy").unwrap_or_default();
    let source = text_of(pull, "sourceRefName").unwrap_or_else(|| "refs/heads/feature".into());
    let target = text_of(pull, "targetRefName").unwrap_or_else(|| "refs/heads/main".into());
    let merge_message = text_of(&pull["completionOptions"], "mergeCommitMessage")
        .filter(|text| !text.is_empty())
        .unwrap_or_else(|| format!("Fusion de {source} dans {target}"));
    RebasePrep {
        replay,
        delete_source: pull["completionOptions"].get("deleteSourceBranch").and_then(|value| value.as_bool()) == Some(true),
        merge_after: strategy.eq_ignore_ascii_case("rebaseMerge"),
        merge_message,
    }
}

fn replay_commits(
    stored: &mut crate::store::ProjectData,
    org: &str,
    project: &str,
    repo_id: &str,
    pull_id: &str,
    commits_key: &str,
    mut parent: Option<String>,
    replay: &[ReplayCommit],
) -> Option<String> {
    for (index, commit) in replay.iter().enumerate() {
        let commit_id = object_id(&format!(
            "rebase:{pull_id}:{index}:{}:{}",
            commit.comment,
            parent.clone().unwrap_or_default()
        ));
        let parents = parent.clone().map(|id| vec![json!(id)]).unwrap_or_default();
        stored.put_bucket(
            commits_key,
            json!({
                "id": commit_id,
                "commitId": commit_id,
                "comment": commit.comment,
                "author": {"name": "azurz", "date": "2026-10-29T00:00:00Z"},
                "committer": {"name": "azurz", "date": "2026-10-29T00:00:00Z"},
                "parents": parents,
                "changeCounts": change_counts(&commit.changes),
                "url": format!("/{org}/{project}/_apis/git/repositories/{repo_id}/commits/{commit_id}")
            }),
        );
        for change in &commit.changes {
            stored.put_bucket(&format!("{commits_key}/{commit_id}/changes"), change.clone());
        }
        let snapshot = stored.list_bucket(&format!("git/repositories/{repo_id}/items"));
        for blob in snapshot {
            stored.put_bucket(&format!("{commits_key}/{commit_id}/blobs"), blob);
        }
        parent = Some(commit_id);
    }
    parent
}

fn finish_pull(state: &AppState, org: &str, project: &str, pull_id: &str, prepared: Option<Prepared>) -> Option<Value> {
    state.write(|db| {
        let stored = db.project_mut(org, project)?;
        let mut pull = stored.get_bucket("git/pullrequests", pull_id)?;
        if pull
            .get("lastMergeCommit")
            .and_then(|commit| text_of(commit, "commitId"))
            .is_some_and(|id| !id.is_empty())
        {
            return Some(pull);
        }
        let repo_id = text_of(&pull["repository"], "id").unwrap_or_default();
        let source = text_of(&pull, "sourceRefName").unwrap_or_else(|| "refs/heads/feature".into());
        let target = text_of(&pull, "targetRefName").unwrap_or_else(|| "refs/heads/main".into());
        let refs_key = format!("git/repositories/{repo_id}/refs");
        let commits_key = format!("git/repositories/{repo_id}/commits");
        let parent = stored.get_bucket(&refs_key, &target).and_then(|item| text_of(&item, "objectId"));
        let source_id = stored.get_bucket(&refs_key, &source).and_then(|item| text_of(&item, "objectId"));
        if let Some(Prepared::Rebase(prep)) = prepared {
            let base = parent.clone().unwrap_or_default();
            let tip = replay_commits(stored, org, project, &repo_id, pull_id, &commits_key, parent.clone(), &prep.replay);
            let rebased = tip.clone().unwrap_or_else(|| base.clone());
            let landing = if prep.merge_after && !prep.replay.is_empty() {
                let merge_id = object_id(&format!("rebase-merge:{pull_id}:{base}:{rebased}:{}", prep.merge_message));
                stored.put_bucket(
                    &commits_key,
                    json!({
                        "id": merge_id,
                        "commitId": merge_id,
                        "comment": prep.merge_message,
                        "author": {"name": "azurz", "date": "2026-10-29T00:00:00Z"},
                        "committer": {"name": "azurz", "date": "2026-10-29T00:00:00Z"},
                        "parents": [json!(base), json!(rebased)],
                        "changeCounts": change_counts(&[]),
                        "url": format!("/{org}/{project}/_apis/git/repositories/{repo_id}/commits/{merge_id}")
                    }),
                );
                merge_id
            } else {
                rebased.clone()
            };
            stored.put_bucket(
                &refs_key,
                json!({"id": target, "name": target, "objectId": landing, "newObjectId": landing, "success": true}),
            );
            if prep.delete_source {
                if let Some(bucket) = stored.buckets.get_mut(&refs_key) {
                    bucket.remove(&source);
                }
            } else {
                stored.put_bucket(
                    &refs_key,
                    json!({"id": source, "name": source, "objectId": rebased, "newObjectId": rebased, "success": true}),
                );
            }
            pull["status"] = json!("completed");
            pull["mergeStatus"] = json!("succeeded");
            pull["closedDate"] = json!("2026-10-29T00:00:00Z");
            pull["closedBy"] = json!({"displayName": "azurz", "uniqueName": "azurz"});
            pull["lastMergeCommit"] = json!({"commitId": landing});
            pull["lastMergeSourceCommit"] = json!({"commitId": rebased});
            pull["lastMergeTargetCommit"] = json!({"commitId": base});
            return Some(stored.put_bucket("git/pullrequests", pull));
        }
        let squash_prep = match prepared {
            Some(Prepared::Squash(prep)) => Some(prep),
            _ => None,
        };
        let squash = squash_prep.is_some();
        let delete_source = squash_prep.as_ref().is_some_and(|prep| prep.delete_source);
        let (commit_id, comment, parents, changes) = if let Some(prep) = squash_prep {
            let parents = parent.clone().map(|id| vec![json!(id)]).unwrap_or_default();
            let commit_id = object_id(&format!(
                "squash:{pull_id}:{}:{}:{}",
                source_id.clone().unwrap_or_default(),
                parent.clone().unwrap_or_default(),
                prep.message
            ));
            (commit_id, prep.message, parents, prep.changes)
        } else {
            let commit_id = object_id(&format!("merge:{pull_id}:{}", parent.clone().unwrap_or_default()));
            let parents = parent.clone().map(|id| vec![json!(id)]).unwrap_or_default();
            (commit_id, format!("Fusion de {source} dans {target}"), parents, Vec::new())
        };
        stored.put_bucket(
            &commits_key,
            json!({
                "id": commit_id,
                "commitId": commit_id,
                "comment": comment,
                "author": {"name": "azurz", "date": "2026-10-29T00:00:00Z"},
                "committer": {"name": "azurz", "date": "2026-10-29T00:00:00Z"},
                "parents": parents,
                "changeCounts": change_counts(&changes),
                "url": format!("/{org}/{project}/_apis/git/repositories/{repo_id}/commits/{commit_id}")
            }),
        );
        for change in &changes {
            stored.put_bucket(&format!("{commits_key}/{commit_id}/changes"), change.clone());
        }
        let snapshot = stored.list_bucket(&format!("git/repositories/{repo_id}/items"));
        for blob in snapshot {
            stored.put_bucket(&format!("{commits_key}/{commit_id}/blobs"), blob);
        }
        stored.put_bucket(
            &refs_key,
            json!({
                "id": target,
                "name": target,
                "objectId": commit_id,
                "newObjectId": commit_id,
                "success": true
            }),
        );
        if delete_source {
            if let Some(bucket) = stored.buckets.get_mut(&refs_key) {
                bucket.remove(&source);
            }
        }
        pull["status"] = json!("completed");
        pull["mergeStatus"] = json!("succeeded");
        pull["closedDate"] = json!("2026-10-29T00:00:00Z");
        pull["closedBy"] = json!({"displayName": "azurz", "uniqueName": "azurz"});
        pull["lastMergeCommit"] = json!({"commitId": commit_id});
        pull["lastMergeSourceCommit"] = json!({"commitId": if squash { source_id.unwrap_or_default() } else { source_id.clone().unwrap_or_else(|| parent.clone().unwrap_or_default()) }});
        pull["lastMergeTargetCommit"] = json!({"commitId": parent.unwrap_or_default()});
        Some(stored.put_bucket("git/pullrequests", pull))
    })
}

fn security_namespaces(org: &str) -> Value {
    let rows = [
        ("Git", "Git Repositories", "2e9eb7ed-3c0a-47d4-87c1-0ffdd275fd87"),
        ("WorkItemTracking", "Work Item Tracking", "73e71c45-d483-40d5-bdba-1a9ccc132849"),
        ("Build", "Build", "33344d9c-fc72-4d6f-aba5-fa317101a7e9"),
        ("ReleaseManagement", "Release Management", "c788c23e-1b46-4162-8f5e-d7585343b5de"),
        ("Identity", "Identity", "5a27515b-ccd7-42c9-84f1-54c998f03866"),
    ];
    let value: Vec<Value> = rows
        .iter()
        .map(|(name, display, id)| {
            json!({
                "namespaceId": id,
                "name": name,
                "displayName": display,
                "separatorValue": "/",
                "writePermission": 1,
                "readPermission": 1,
                "dataspaceCategory": name,
                "url": format!("/{org}/_apis/securitynamespaces/{id}")
            })
        })
        .collect();
    page(value)
}

fn definitions(state: &AppState, method: &Method, org: &str, project: &str, body: &[u8]) -> Response {
    let key = "build/definitions";
    if method == Method::GET {
        let value = list(state, org, project, key)
            .into_iter()
            .map(|item| dress_definition(project, item))
            .collect();
        return Json(page(value)).into_response();
    }
    if method != Method::POST {
        return fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée");
    }
    let payload = parse_json(body).unwrap_or(json!({}));
    let name = text_of(&payload, "name").unwrap_or_else(|| "CI".into());
    let next = list(state, org, project, key).len() as i64 + 1;
    let item = dress_definition(project, json!({"id": next, "name": name, "path": "\\", "type": "build"}));
    put_item(state, org, project, key, item.clone());
    Json(item).into_response()
}

fn build_artifacts(
    state: &AppState,
    method: &Method,
    org: &str,
    project: &str,
    build_id: &str,
    rest: &[String],
    query: &str,
    body: &[u8],
) -> Response {
    let key = format!("build/builds/{build_id}/artifacts");
    if rest.len() == 4 && method == Method::POST {
        let payload = parse_json(body).unwrap_or(json!({}));
        let name = text_of(&payload, "name").unwrap_or_default().trim().to_string();
        if name.is_empty() {
            return fail(StatusCode::BAD_REQUEST, "nom d'artefact absent");
        }
        if list(state, org, project, &key)
            .iter()
            .any(|item| text_of(item, "name").is_some_and(|stored| stored.eq_ignore_ascii_case(&name)))
        {
            return fail(StatusCode::BAD_REQUEST, "artefact déjà présent");
        }
        let incoming = payload.get("resource").cloned().unwrap_or(json!({"type": "PipelineArtifact"}));
        let content = incoming.get("content").and_then(|value| value.as_str()).unwrap_or("").to_string();
        let mut resource = if incoming.is_object() { incoming } else { json!({"type": "PipelineArtifact", "data": incoming}) };
        if let Some(object) = resource.as_object_mut() {
            object.remove("content");
            object.entry("type").or_insert(json!("PipelineArtifact"));
        }
        let next = list(state, org, project, &key)
            .into_iter()
            .filter_map(|item| item.get("id").and_then(|value| value.as_i64()))
            .max()
            .unwrap_or(0)
            + 1;
        let item = json!({
            "id": next,
            "name": name,
            "source": "build",
            "resource": resource,
            "content": base64::engine::general_purpose::STANDARD.encode(content.as_bytes())
        });
        put_item(state, org, project, &key, item.clone());
        return Json(dress_build_artifact(org, project, build_id, &item)).into_response();
    }
    let named = artifact_query_name(query, rest);
    if method == Method::GET && named.is_none() && rest.len() == 4 {
        let value = list(state, org, project, &key)
            .into_iter()
            .map(|item| dress_build_artifact(org, project, build_id, &item))
            .collect();
        return Json(page(value)).into_response();
    }
    let Some(name) = named else {
        return fail(StatusCode::NOT_FOUND, "artefact introuvable");
    };
    let Some(item) = list(state, org, project, &key)
        .into_iter()
        .find(|item| text_of(item, "name").is_some_and(|stored| stored.eq_ignore_ascii_case(&name)))
    else {
        return fail(StatusCode::NOT_FOUND, "artefact introuvable");
    };
    let download = rest.iter().skip(4).any(|part| part.eq_ignore_ascii_case("download"))
        || query_param(query, "download").is_some_and(|value| pct_decode(value).eq_ignore_ascii_case("true"))
        || query_param(query, "$format").is_some_and(|value| pct_decode(value).eq_ignore_ascii_case("zip"));
    if method == Method::GET && download {
        let encoded = text_of(&item, "content").unwrap_or_default();
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(encoded.as_bytes())
            .unwrap_or_default();
        let content_type = if query_param(query, "$format").is_some_and(|value| pct_decode(value).eq_ignore_ascii_case("zip")) {
            "application/zip"
        } else {
            "application/octet-stream"
        };
        return Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, content_type)
            .body(axum::body::Body::from(bytes))
            .unwrap_or_else(|_| fail(StatusCode::INTERNAL_SERVER_ERROR, "réponse impossible"));
    }
    if method == Method::GET {
        return Json(dress_build_artifact(org, project, build_id, &item)).into_response();
    }
    if method == Method::DELETE {
        let stored_id = value_key(&item["id"]);
        return if delete_item(state, org, project, &key, &stored_id) {
            StatusCode::NO_CONTENT.into_response()
        } else {
            fail(StatusCode::NOT_FOUND, "artefact introuvable")
        };
    }
    fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée")
}

fn artifact_query_name(query: &str, rest: &[String]) -> Option<String> {
    if let Some(part) = rest.get(4) {
        if !part.eq_ignore_ascii_case("download") {
            let name = pct_decode(part).trim().to_string();
            if !name.is_empty() {
                return Some(name);
            }
        }
    }
    query_param(query, "artifactName").and_then(|value| {
        let name = pct_decode(value).trim().to_string();
        (!name.is_empty()).then_some(name)
    })
}

fn dress_build_artifact(org: &str, project: &str, build_id: &str, item: &Value) -> Value {
    let name = text_of(item, "name").unwrap_or_default();
    let encoded = query_encode(&name);
    let url = format!("/{org}/{project}/_apis/build/builds/{build_id}/artifacts?artifactName={encoded}");
    let download = format!("{url}&%24format=zip");
    let mut resource = item.get("resource").cloned().unwrap_or(json!({"type": "PipelineArtifact"}));
    if let Some(object) = resource.as_object_mut() {
        object.remove("content");
        object.entry("type").or_insert(json!("PipelineArtifact"));
        object.insert("downloadUrl".into(), json!(download));
        object.insert("url".into(), json!(url));
    }
    json!({
        "id": item["id"],
        "name": name,
        "source": item.get("source").cloned().unwrap_or(json!("build")),
        "resource": resource,
        "url": url
    })
}

fn query_encode(value: &str) -> String {
    let mut out = String::new();
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(byte as char),
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

fn builds(state: &AppState, method: &Method, org: &str, project: &str, rest: &[String], query: &str, body: &[u8]) -> Response {
    let key = "build/builds";
    if rest.len() == 2 && method == Method::GET {
        let value = list(state, org, project, key)
            .into_iter()
            .map(|item| promote_build(state, org, project, item))
            .collect();
        return Json(page(value)).into_response();
    }
    if rest.len() == 2 && method == Method::POST {
        let payload = parse_json(body).unwrap_or(json!({}));
        let next = list(state, org, project, key).len() as i64 + 1;
        let definition_id = payload
            .get("definition")
            .and_then(|value| value.get("id"))
            .and_then(|value| value.as_i64())
            .unwrap_or(1);
        let definition_name = list(state, org, project, "build/definitions")
            .into_iter()
            .find(|item| item.get("id").and_then(|value| value.as_i64()) == Some(definition_id))
            .and_then(|item| text_of(&item, "name"))
            .unwrap_or_else(|| "CI".into());
        let queue = list(state, org, project, "distributedtask/queues")
            .into_iter()
            .next()
            .unwrap_or_else(|| json!({"id": 1, "name": "Default", "pool": {"id": 1, "name": "Azure Pipelines"}}));
        let item = json!({
            "id": next,
            "buildNumber": format!("20261029.{next}"),
            "status": "notStarted",
            "definition": {"id": definition_id, "name": definition_name},
            "project": {"name": project},
            "sourceBranch": text_of(&payload, "sourceBranch").unwrap_or_else(|| "refs/heads/main".into()),
            "sourceVersion": object_id(&format!("build-{next}")),
            "queue": queue,
            "queueTime": "2026-10-29T00:00:00Z",
            "url": format!("/{org}/{project}/_apis/build/builds/{next}"),
            "requestedBy": {"displayName": "azurz"}
        });
        put_item(state, org, project, key, item.clone());
        return Json(item).into_response();
    }
    let id = rest.get(2).map(String::as_str).unwrap_or("");
    if rest.len() >= 4 && rest[3].eq_ignore_ascii_case("timeline") {
        return build_timeline(state, method, org, project, id, body);
    }
    if rest.len() >= 5 && rest[3].eq_ignore_ascii_case("logs") && method == Method::GET {
        let log_id = rest.get(4).map(String::as_str).unwrap_or("1");
        let text = format!("Build {id} log {log_id}\n##[section]Build succeeded\n");
        return (
            [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
            text,
        )
            .into_response();
    }
    if rest.len() >= 4 && rest[3].eq_ignore_ascii_case("logs") && method == Method::GET {
        return Json(page(vec![json!({"id": 1, "type": "Container", "lineCount": 2, "url": format!("/{org}/{project}/_apis/build/builds/{id}/logs/1")})])).into_response();
    }
    if rest.len() >= 4 && rest[3].eq_ignore_ascii_case("artifacts") {
        return build_artifacts(state, method, org, project, id, rest, query, body);
    }
    if method == Method::GET {
        return match get_item(state, org, project, key, id) {
            Some(item) => Json(promote_build(state, org, project, item)).into_response(),
            None => fail(StatusCode::NOT_FOUND, "build introuvable"),
        };
    }
    if method == Method::PATCH || method == Method::PUT {
        let payload = parse_json(body).unwrap_or(json!({}));
        return match patch_item(state, org, project, key, id, &payload) {
            Some(item) => Json(item).into_response(),
            None => fail(StatusCode::NOT_FOUND, "build introuvable"),
        };
    }
    fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée")
}

fn pipelines(state: &AppState, method: &Method, org: &str, project: &str, rest: &[String], body: &[u8]) -> Response {
    let key = "pipelines/pipelines";
    if rest.len() == 2 && method == Method::GET {
        return Json(page(list(state, org, project, key))).into_response();
    }
    if rest.len() == 2 && method == Method::POST {
        let payload = parse_json(body).unwrap_or(json!({}));
        let name = text_of(&payload, "name").unwrap_or_else(|| "CI".into());
        let next = list(state, org, project, key).len() as i64 + 1;
        let item = json!({"id": next, "name": name, "folder": "\\", "revision": 1});
        put_item(state, org, project, key, item.clone());
        return Json(item).into_response();
    }
    if rest.len() >= 4 && rest[3].eq_ignore_ascii_case("runs") && method == Method::POST {
        let payload = parse_json(body).unwrap_or(json!({}));
        let requested = payload
            .get("resources")
            .and_then(|value| value.get("environments"))
            .and_then(|value| value.as_array())
            .cloned()
            .unwrap_or_default();
        let mut targets = Vec::new();
        for entry in &requested {
            let Some(environment) = resolve_environment(state, org, project, entry) else {
                return fail(StatusCode::BAD_REQUEST, "environnement introuvable");
            };
            targets.push(environment);
        }
        let pipeline_id = rest[2].parse::<i64>().unwrap_or(1);
        let name = get_item(state, org, project, key, &pipeline_id.to_string())
            .and_then(|item| text_of(&item, "name"))
            .unwrap_or_else(|| "CI".into());
        let runs_key = "pipelines/runs";
        let next = list(state, org, project, runs_key).len() as i64 + 1;
        let resources = json!({
            "environments": targets.iter().map(|environment| json!({
                "id": value_key(&environment["id"]),
                "name": text_of(environment, "name").unwrap_or_default()
            })).collect::<Vec<_>>()
        });
        let item = json!({
            "id": next,
            "name": format!("{name}-{next}"),
            "state": "inProgress",
            "pipeline": {"id": pipeline_id, "name": name.clone(), "folder": "\\", "revision": 1},
            "resources": resources,
            "createdDate": "2026-10-29T00:00:00Z",
            "url": format!("/{org}/{project}/_apis/pipelines/runs/{next}")
        });
        let checks = approval_checks(state, org, project, &targets);
        put_item(state, org, project, runs_key, item.clone());
        let mut approval_id = list(state, org, project, "pipelines/approvals")
            .into_iter()
            .filter_map(|row| row.get("id").and_then(|value| value.as_i64()))
            .max()
            .unwrap_or(0);
        for (environment, check) in checks {
            approval_id += 1;
            let settings = check.get("settings").cloned().unwrap_or(json!({}));
            let approvers = settings.get("approvers").cloned().unwrap_or(json!([]));
            let min_required = settings.get("minRequiredApprovers").cloned().unwrap_or(json!(1));
            let instructions = text_of(&settings, "instructions").unwrap_or_default();
            put_item(
                state,
                org,
                project,
                "pipelines/approvals",
                json!({
                    "id": approval_id,
                    "status": "pending",
                    "createdOn": "2026-10-29T00:00:00Z",
                    "lastModifiedOn": "2026-10-29T00:00:00Z",
                    "minRequiredApprovers": min_required,
                    "instructions": instructions,
                    "comment": "",
                    "approvers": approvers,
                    "pipeline": {
                        "owner": {"id": pipeline_id, "name": name},
                        "id": next.to_string(),
                        "name": format!("{name}-{next}")
                    },
                    "resource": {
                        "type": "environment",
                        "id": value_key(&environment["id"]),
                        "name": text_of(&environment, "name").unwrap_or_default()
                    },
                    "url": format!("/{org}/{project}/_apis/pipelines/approvals/{approval_id}")
                }),
            );
        }
        return Json(item).into_response();
    }
    if method == Method::GET {
        let id = rest.get(2).map(String::as_str).unwrap_or("");
        return match get_item(state, org, project, key, id) {
            Some(item) => Json(item).into_response(),
            None => fail(StatusCode::NOT_FOUND, "pipeline introuvable"),
        };
    }
    fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée")
}

fn check_configurations(
    state: &AppState,
    method: &Method,
    org: &str,
    project: &str,
    rest: &[String],
    query: &str,
    body: &[u8],
) -> Response {
    let kind = rest.get(2).map(|value| value.to_ascii_lowercase()).unwrap_or_default();
    if kind == "checktypes" && method == Method::GET {
        return Json(page(check_types())).into_response();
    }
    if kind != "configurations" {
        return fail(StatusCode::NOT_FOUND, "contrôle introuvable");
    }
    let key = "pipelines/checks/configurations";
    if rest.len() == 3 && method == Method::GET {
        let resource_type = query_param(query, "resourceType").map(|value| pct_decode(value).to_ascii_lowercase());
        let resource_id = query_param(query, "resourceId").map(pct_decode);
        let value: Vec<Value> = list(state, org, project, key)
            .into_iter()
            .filter(|item| {
                let resource = item.get("resource");
                let stored_type = resource
                    .and_then(|value| text_of(value, "type"))
                    .unwrap_or_default()
                    .to_ascii_lowercase();
                let stored_id = resource.map(|value| value_key(&value["id"])).unwrap_or_default();
                resource_type.as_ref().is_none_or(|wanted| stored_type == *wanted)
                    && resource_id.as_ref().is_none_or(|wanted| stored_id == *wanted)
            })
            .collect();
        return Json(page(value)).into_response();
    }
    if rest.len() == 3 && method == Method::POST {
        let payload = match parse_json(body) {
            Some(value) if value.is_object() => value,
            _ => return fail(StatusCode::BAD_REQUEST, "ressource absente"),
        };
        let Some((type_id, type_name)) = known_check_type(text_of(&payload["type"], "id").as_deref()) else {
            return fail(StatusCode::BAD_REQUEST, "type de contrôle inconnu");
        };
        let resource = payload.get("resource").cloned().unwrap_or(json!({}));
        let resource_type = text_of(&resource, "type").unwrap_or_default().to_ascii_lowercase();
        if !known_check_resource(&resource_type) {
            return fail(
                StatusCode::BAD_REQUEST,
                if resource_type.is_empty() { "ressource absente" } else { "type de ressource inconnu" },
            );
        }
        let resource_id = value_key(&resource["id"]);
        if resource_id.is_empty() {
            return fail(StatusCode::BAD_REQUEST, "ressource absente");
        }
        let resource_name = if resource_type == "environment" {
            match get_item(state, org, project, "distributedtask/environments", &resource_id) {
                Some(environment) => text_of(&environment, "name").unwrap_or(resource_id.clone()),
                None => return fail(StatusCode::BAD_REQUEST, "environnement introuvable"),
            }
        } else {
            text_of(&resource, "name").unwrap_or_else(|| resource_id.clone())
        };
        let mut settings = payload.get("settings").cloned().unwrap_or(json!({}));
        if type_name == "Approval" {
            let approvers = settings
                .get("approvers")
                .and_then(|value| value.as_array())
                .cloned()
                .unwrap_or_default();
            let approvers: Vec<Value> = approvers
                .into_iter()
                .filter(|item| {
                    text_of(item, "displayName").is_some_and(|name| !name.trim().is_empty())
                        || text_of(item, "id").is_some_and(|id| !id.trim().is_empty())
                })
                .collect();
            if approvers.is_empty() {
                return fail(StatusCode::BAD_REQUEST, "approbateurs absents");
            }
            if let Some(object) = settings.as_object_mut() {
                object.insert("approvers".into(), json!(approvers));
                object.entry("minRequiredApprovers").or_insert(json!(1));
                object.entry("requesterCannotBeApprover").or_insert(json!(false));
            }
        }
        if type_name == "Task Check" && branch_control(&settings) {
            let branches = settings
                .get("inputs")
                .and_then(|inputs| text_of(inputs, "allowedBranches"))
                .unwrap_or_default();
            if branches.trim().is_empty() {
                return fail(StatusCode::BAD_REQUEST, "branches absentes");
            }
        }
        let duplicate = list(state, org, project, key).into_iter().any(|item| {
            text_of(&item["type"], "id").is_some_and(|id| id.eq_ignore_ascii_case(&type_id))
                && text_of(&item["resource"], "type").is_some_and(|kind| kind.eq_ignore_ascii_case(&resource_type))
                && value_key(&item["resource"]["id"]) == resource_id
        });
        if duplicate {
            return fail(StatusCode::BAD_REQUEST, "contrôle déjà présent");
        }
        let next = list(state, org, project, key)
            .into_iter()
            .filter_map(|item| item.get("id").and_then(|value| value.as_i64()))
            .max()
            .unwrap_or(0)
            + 1;
        let timeout = payload.get("timeout").and_then(|value| value.as_i64()).unwrap_or(43200);
        let item = json!({
            "id": next,
            "version": 1,
            "type": {"id": type_id, "name": type_name},
            "settings": settings,
            "resource": {"type": resource_type, "id": resource_id, "name": resource_name},
            "timeout": timeout,
            "isDisabled": payload.get("isDisabled").and_then(|value| value.as_bool()).unwrap_or(false),
            "createdBy": {"displayName": "azurz", "uniqueName": "azurz"},
            "url": format!("/{org}/{project}/_apis/pipelines/checks/configurations/{next}")
        });
        put_item(state, org, project, key, item.clone());
        return Json(item).into_response();
    }
    let id = rest.get(3).map(String::as_str).unwrap_or("");
    if id.is_empty() {
        return fail(StatusCode::NOT_FOUND, "contrôle introuvable");
    }
    if method == Method::GET {
        return match get_item(state, org, project, key, id) {
            Some(item) => Json(item).into_response(),
            None => fail(StatusCode::NOT_FOUND, "contrôle introuvable"),
        };
    }
    if method == Method::DELETE {
        return if delete_item(state, org, project, key, id) {
            StatusCode::NO_CONTENT.into_response()
        } else {
            fail(StatusCode::NOT_FOUND, "contrôle introuvable")
        };
    }
    fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée")
}

fn check_types() -> Vec<Value> {
    [
        ("8c6f20a7-a545-4486-9777-f762fafe0d4d", "Approval"),
        ("fe1de3ee-a436-41b4-bb20-f6eb4cb879a7", "Task Check"),
        ("2ef31ad6-baa0-403a-8b45-2cbc9b4e5563", "ExclusiveLock"),
    ]
    .into_iter()
    .map(|(id, name)| json!({"id": id, "name": name}))
    .collect()
}

fn known_check_type(id: Option<&str>) -> Option<(String, &'static str)> {
    let id = id?.to_ascii_lowercase();
    let name = match id.as_str() {
        "8c6f20a7-a545-4486-9777-f762fafe0d4d" => "Approval",
        "fe1de3ee-a436-41b4-bb20-f6eb4cb879a7" => "Task Check",
        "2ef31ad6-baa0-403a-8b45-2cbc9b4e5563" => "ExclusiveLock",
        _ => return None,
    };
    Some((id, name))
}

fn known_check_resource(kind: &str) -> bool {
    matches!(kind, "environment" | "endpoint" | "queue" | "repository" | "variablegroup" | "securefile")
}

fn branch_control(settings: &Value) -> bool {
    text_of(settings, "displayName").is_some_and(|name| name.eq_ignore_ascii_case("Branch control"))
        || settings.get("inputs").and_then(|inputs| inputs.get("allowedBranches")).is_some()
}

fn pipeline_approvals(
    state: &AppState,
    method: &Method,
    org: &str,
    project: &str,
    rest: &[String],
    query: &str,
    body: &[u8],
) -> Response {
    let key = "pipelines/approvals";
    if rest.len() == 2 && method == Method::GET {
        let wanted = query_param(query, "state").map(|value| pct_decode(value).to_ascii_lowercase());
        let value: Vec<Value> = list(state, org, project, key)
            .into_iter()
            .filter(|item| wanted.as_ref().is_none_or(|state_name| text_of(item, "status").is_some_and(|status| status.eq_ignore_ascii_case(state_name))))
            .collect();
        return Json(page(value)).into_response();
    }
    if rest.len() == 2 && method == Method::PATCH {
        let Some(rows) = parse_json(body).and_then(|value| value.as_array().cloned()) else {
            return fail(StatusCode::BAD_REQUEST, "approbation absente");
        };
        if rows.is_empty() {
            return fail(StatusCode::BAD_REQUEST, "approbation absente");
        }
        let mut updates = Vec::new();
        for row in &rows {
            let approval_id = row.get("approvalId").map(value_key).filter(|id| !id.is_empty()).unwrap_or_default();
            if approval_id.is_empty() {
                return fail(StatusCode::BAD_REQUEST, "approbation absente");
            }
            let Some(status_name) = text_of(row, "status") else {
                return fail(StatusCode::BAD_REQUEST, "statut d'approbation inconnu");
            };
            if !matches!(status_name.to_ascii_lowercase().as_str(), "approved" | "rejected") {
                return fail(StatusCode::BAD_REQUEST, "statut d'approbation inconnu");
            }
            let Some(current) = get_item(state, org, project, key, &approval_id) else {
                return fail(StatusCode::NOT_FOUND, "approbation introuvable");
            };
            if text_of(&current, "status").as_deref() != Some("pending") {
                return fail(StatusCode::BAD_REQUEST, "approbation déjà traitée");
            }
            updates.push((approval_id, status_name.to_ascii_lowercase(), text_of(row, "comment").unwrap_or_default(), current));
        }
        let mut saved = Vec::new();
        for (_approval_id, status_name, comment, mut current) in updates {
            current["status"] = json!(status_name);
            current["comment"] = json!(comment);
            current["lastModifiedOn"] = json!("2026-10-29T00:00:03Z");
            put_item(state, org, project, key, current.clone());
            let run_id = value_key(&current["pipeline"]["id"]);
            settle_pipeline_run(state, org, project, &run_id);
            saved.push(current);
        }
        return Json(json!(saved)).into_response();
    }
    if method == Method::GET {
        let id = rest.get(2).map(String::as_str).unwrap_or("");
        return match get_item(state, org, project, key, id) {
            Some(item) => Json(item).into_response(),
            None => fail(StatusCode::NOT_FOUND, "approbation introuvable"),
        };
    }
    fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée")
}

fn resolve_environment(state: &AppState, org: &str, project: &str, entry: &Value) -> Option<Value> {
    let id = value_key(&entry["id"]);
    if !id.is_empty() {
        return get_item(state, org, project, "distributedtask/environments", &id);
    }
    let name = text_of(entry, "name").unwrap_or_default();
    if name.is_empty() {
        return None;
    }
    get_item(state, org, project, "distributedtask/environments", &name)
}

fn approval_checks(state: &AppState, org: &str, project: &str, environments: &[Value]) -> Vec<(Value, Value)> {
    let checks = list(state, org, project, "pipelines/checks/configurations");
    let mut found = Vec::new();
    for environment in environments {
        let env_id = value_key(&environment["id"]);
        for check in &checks {
            if check.get("isDisabled").and_then(|value| value.as_bool()).unwrap_or(false) {
                continue;
            }
            if text_of(&check["type"], "name").as_deref() != Some("Approval") {
                continue;
            }
            let resource = &check["resource"];
            let same_kind = text_of(resource, "type").is_some_and(|kind| kind.eq_ignore_ascii_case("environment"));
            if same_kind && value_key(&resource["id"]) == env_id {
                found.push((environment.clone(), check.clone()));
            }
        }
    }
    found
}

enum ApprovalOutcome {
    Waiting,
    Rejected,
    Clear,
}

fn approval_outcome(state: &AppState, org: &str, project: &str, run_id: &str) -> ApprovalOutcome {
    let rows: Vec<Value> = list(state, org, project, "pipelines/approvals")
        .into_iter()
        .filter(|item| value_key(&item["pipeline"]["id"]) == run_id)
        .collect();
    if rows.is_empty() {
        return ApprovalOutcome::Clear;
    }
    if rows.iter().any(|item| text_of(item, "status").as_deref() == Some("rejected")) {
        return ApprovalOutcome::Rejected;
    }
    if rows.iter().any(|item| text_of(item, "status").as_deref() == Some("pending")) {
        return ApprovalOutcome::Waiting;
    }
    ApprovalOutcome::Clear
}

fn settle_pipeline_run(state: &AppState, org: &str, project: &str, run_id: &str) {
    let Some(mut run) = get_item(state, org, project, "pipelines/runs", run_id) else {
        return;
    };
    if text_of(&run, "state").as_deref() != Some("inProgress") {
        return;
    }
    let result = match approval_outcome(state, org, project, run_id) {
        ApprovalOutcome::Waiting => return,
        ApprovalOutcome::Rejected => "failed",
        ApprovalOutcome::Clear => "succeeded",
    };
    run["state"] = json!("completed");
    run["result"] = json!(result);
    run["finishedDate"] = json!("2026-10-29T00:00:03Z");
    put_item(state, org, project, "pipelines/runs", run);
}

fn pipeline_runs(state: &AppState, method: &Method, org: &str, project: &str, rest: &[String]) -> Response {
    if method != Method::GET {
        return fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée");
    }
    if rest.len() == 2 {
        let value = list(state, org, project, "pipelines/runs")
            .into_iter()
            .map(|item| promote_pipeline(state, org, project, item))
            .collect();
        return Json(page(value)).into_response();
    }
    let id = rest.get(2).map(String::as_str).unwrap_or("");
    match get_item(state, org, project, "pipelines/runs", id) {
        Some(item) => Json(promote_pipeline(state, org, project, item)).into_response(),
        None => fail(StatusCode::NOT_FOUND, "exécution introuvable"),
    }
}

fn test_plans(state: &AppState, method: &Method, org: &str, project: &str, rest: &[String], body: &[u8]) -> Response {
    let key = "testplan/plans";
    if rest.len() == 2 && method == Method::GET {
        let value = list(state, org, project, key)
            .into_iter()
            .map(|item| dress_plan(project, item))
            .collect();
        return Json(page(value)).into_response();
    }
    if rest.len() == 2 && method == Method::POST {
        let payload = parse_json(body).unwrap_or(json!({}));
        let name = text_of(&payload, "name").unwrap_or_else(|| "Plan".into());
        let next = list(state, org, project, key).len() as i64 + 1;
        let item = dress_plan(project, json!({"id": next, "name": name}));
        put_item(state, org, project, key, item.clone());
        put_item(
            state,
            org,
            project,
            &format!("testplan/plans/{next}/suites"),
            json!({
                "id": next,
                "name": name,
                "suiteType": "staticTestSuite",
                "plan": {"id": next, "name": name}
            }),
        );
        return Json(item).into_response();
    }
    let plan_id = rest.get(2).map(String::as_str).unwrap_or("");
    if rest.len() == 3 && method == Method::GET {
        return match get_item(state, org, project, key, plan_id) {
            Some(item) => Json(dress_plan(project, item)).into_response(),
            None => fail(StatusCode::NOT_FOUND, "plan introuvable"),
        };
    }
    if rest.len() >= 4 && rest[3].eq_ignore_ascii_case("suites") {
        return plan_suites(state, method, org, project, plan_id, rest, body);
    }
    fail(StatusCode::NOT_FOUND, "plan introuvable")
}

fn plan_suites(state: &AppState, method: &Method, org: &str, project: &str, plan_id: &str, rest: &[String], body: &[u8]) -> Response {
    let suites_key = format!("testplan/plans/{plan_id}/suites");
    ensure_root_suite(state, org, project, plan_id);
    if rest.len() == 4 && method == Method::GET {
        return Json(page(list(state, org, project, &suites_key))).into_response();
    }
    if rest.len() == 4 && method == Method::POST {
        let payload = parse_json(body).unwrap_or(json!({}));
        let name = text_of(&payload, "name").unwrap_or_else(|| "Suite".into());
        let next = list(state, org, project, &suites_key).len() as i64 + 1;
        let parent = payload.get("parentSuite").and_then(|value| value.get("id")).cloned().unwrap_or(json!(plan_id.parse::<i64>().unwrap_or(next)));
        let item = json!({
            "id": next,
            "name": name,
            "suiteType": text_of(&payload, "suiteType").unwrap_or_else(|| "staticTestSuite".into()),
            "plan": {"id": plan_id.parse::<i64>().unwrap_or(0)},
            "parentSuite": {"id": parent}
        });
        put_item(state, org, project, &suites_key, item.clone());
        return Json(item).into_response();
    }
    let suite_id = rest.get(4).map(String::as_str).unwrap_or("");
    let child = rest.get(5).map(|value| value.to_ascii_lowercase()).unwrap_or_default();
    if child == "testcase" || child == "testcases" {
        return suite_cases(state, method, org, project, plan_id, suite_id, body);
    }
    if (child == "testpoint" || child == "testpoints") && method == Method::GET {
        return Json(page(list(state, org, project, &format!("testplan/plans/{plan_id}/suites/{suite_id}/points")))).into_response();
    }
    if rest.len() == 5 && method == Method::GET {
        return match get_item(state, org, project, &suites_key, suite_id) {
            Some(item) => Json(item).into_response(),
            None => fail(StatusCode::NOT_FOUND, "suite introuvable"),
        };
    }
    fail(StatusCode::NOT_FOUND, "suite introuvable")
}

fn ensure_root_suite(state: &AppState, org: &str, project: &str, plan_id: &str) {
    let suites_key = format!("testplan/plans/{plan_id}/suites");
    if !list(state, org, project, &suites_key).is_empty() {
        return;
    }
    let Some(plan) = get_item(state, org, project, "testplan/plans", plan_id) else {
        return;
    };
    let name = text_of(&plan, "name").unwrap_or_else(|| "Plan".into());
    put_item(
        state,
        org,
        project,
        &suites_key,
        json!({
            "id": plan_id.parse::<i64>().unwrap_or(1),
            "name": name,
            "suiteType": "staticTestSuite",
            "plan": {"id": plan_id.parse::<i64>().unwrap_or(1), "name": name}
        }),
    );
}

fn record_point_outcomes(state: &AppState, org: &str, project: &str, rows: &[Value]) {
    state.write(|db| {
        let Some(stored) = db.project_mut(org, project) else {
            return;
        };
        let keys: Vec<String> = stored.buckets.keys().filter(|key| key.ends_with("/points")).cloned().collect();
        for key in keys {
            let Some(bucket) = stored.buckets.get_mut(&key) else {
                continue;
            };
            for item in bucket.items.values_mut() {
                let point_id = item.get("id").and_then(|value| value.as_i64());
                let case_id = item.get("testCase").and_then(|case| case.get("id")).and_then(|value| value.as_i64());
                for row in rows {
                    let wanted_point = row.get("testPoint").and_then(|point| point.get("id")).and_then(|value| value.as_i64());
                    let wanted_case = row.get("testCase").and_then(|case| case.get("id")).and_then(|value| value.as_i64());
                    let matches = (wanted_point.is_some() && wanted_point == point_id) || (wanted_case.is_some() && wanted_case == case_id);
                    if !matches {
                        continue;
                    }
                    let Some(outcome) = text_of(row, "outcome") else {
                        continue;
                    };
                    if let Some(object) = item.as_object_mut() {
                        object.insert("outcome".into(), json!(outcome));
                        object.insert("state".into(), json!("Completed"));
                    }
                }
            }
        }
    });
}

fn suite_cases(state: &AppState, method: &Method, org: &str, project: &str, plan_id: &str, suite_id: &str, body: &[u8]) -> Response {
    let cases_key = format!("testplan/plans/{plan_id}/suites/{suite_id}/testcases");
    let points_key = format!("testplan/plans/{plan_id}/suites/{suite_id}/points");
    if method == Method::GET {
        return Json(page(list(state, org, project, &cases_key))).into_response();
    }
    if method != Method::POST {
        return fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée");
    }
    let payload = parse_json(body).unwrap_or(json!([]));
    let rows = payload.as_array().cloned().unwrap_or_else(|| vec![payload]);
    let mut stored = Vec::new();
    for row in rows {
        let work_item = row.get("workItem").cloned().unwrap_or(json!({}));
        let mut work_id = work_item.get("id").and_then(|value| value.as_i64()).unwrap_or(0);
        let mut title = text_of(&row, "title")
            .or_else(|| text_of(&work_item, "name"))
            .unwrap_or_else(|| "Cas de test".into());
        if work_id == 0 {
            let created = state.write(|db| {
                db.ensure_project(org, project);
                let stored = db.project_mut(org, project)?;
                let mut fields = Map::new();
                fields.insert("System.Title".into(), json!(title.clone()));
                Some(stored.create_work_item(project, "Test Case", fields))
            });
            let Some(item) = created else {
                return fail(StatusCode::NOT_FOUND, "projet inconnu");
            };
            deliver_work_item(state, org, project, "workitem.created", &item);
            work_id = item.get("id").and_then(|value| value.as_i64()).unwrap_or(0);
            title = item
                .get("fields")
                .and_then(|fields| text_of(fields, "System.Title"))
                .unwrap_or(title);
        }
        let case = json!({
            "id": work_id,
            "workItem": {"id": work_id, "name": title},
            "pointAssignments": [],
            "project": {"name": project}
        });
        put_item(state, org, project, &cases_key, case.clone());
        if get_item(state, org, project, &points_key, &work_id.to_string()).is_none()
            && list(state, org, project, &points_key).iter().all(|point| point.get("testCase").and_then(|item| item.get("id")).and_then(|id| id.as_i64()) != Some(work_id))
        {
            let point_id = list(state, org, project, &points_key).len() as i64 + 1;
            put_item(
                state,
                org,
                project,
                &points_key,
                json!({
                    "id": point_id,
                    "testCase": {"id": work_id, "name": title},
                    "outcome": "Unspecified",
                    "state": "Ready",
                    "plan": {"id": plan_id},
                    "suite": {"id": suite_id}
                }),
            );
        }
        stored.push(case);
    }
    Json(page(stored)).into_response()
}

fn test_runs(state: &AppState, method: &Method, org: &str, project: &str, rest: &[String], body: &[u8]) -> Response {
    let key = "test/runs";
    if rest.len() == 2 && method == Method::GET {
        return Json(page(list(state, org, project, key))).into_response();
    }
    if rest.len() == 2 && method == Method::POST {
        let payload = parse_json(body).unwrap_or(json!({}));
        let name = text_of(&payload, "name").unwrap_or_else(|| "Exécution".into());
        let next = list(state, org, project, key).len() as i64 + 1;
        let plan = payload.get("plan").cloned().unwrap_or(json!({"id": 0}));
        let item = json!({
            "id": next,
            "name": name,
            "state": "NotStarted",
            "plan": plan,
            "project": {"name": project},
            "totalTests": 0,
            "passedTests": 0,
            "incompleteTests": 0,
            "url": format!("/{org}/{project}/_apis/test/runs/{next}")
        });
        put_item(state, org, project, key, item.clone());
        return Json(item).into_response();
    }
    let run_id = rest.get(2).map(String::as_str).unwrap_or("");
    if rest.len() >= 4 && rest[3].eq_ignore_ascii_case("results") {
        let results_key = format!("test/runs/{run_id}/results");
        if method == Method::GET {
            return Json(page(list(state, org, project, &results_key))).into_response();
        }
        if method == Method::POST {
            let payload = parse_json(body).unwrap_or(json!([]));
            let rows = payload.as_array().cloned().unwrap_or_else(|| vec![payload]);
            let mut stored_rows = Vec::new();
            for (index, row) in rows.into_iter().enumerate() {
                let id = list(state, org, project, &results_key).len() as i64 + 1 + index as i64;
                let mut item = row;
                if let Some(object) = item.as_object_mut() {
                    object.entry("id").or_insert(json!(id));
                    object.entry("outcome").or_insert(json!("Passed"));
                    object.entry("state").or_insert(json!("Completed"));
                    object.entry("testRun").or_insert(json!({"id": run_id}));
                }
                put_item(state, org, project, &results_key, item.clone());
                stored_rows.push(item);
            }
            record_point_outcomes(state, org, project, &stored_rows);
            let passed = list(state, org, project, &results_key)
                .iter()
                .filter(|item| text_of(item, "outcome").as_deref() == Some("Passed"))
                .count();
            if let Some(mut run) = get_item(state, org, project, key, run_id) {
                run["totalTests"] = json!(list(state, org, project, &results_key).len());
                run["passedTests"] = json!(passed);
                run["state"] = json!("Completed");
                put_item(state, org, project, key, run);
            }
            return Json(page(stored_rows)).into_response();
        }
    }
    if method == Method::GET {
        return match get_item(state, org, project, key, run_id) {
            Some(item) => Json(item).into_response(),
            None => fail(StatusCode::NOT_FOUND, "exécution introuvable"),
        };
    }
    fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée")
}

fn wikis(
    state: &AppState,
    method: &Method,
    org: &str,
    project: &str,
    rest: &[String],
    query: &str,
    body: &[u8],
) -> Response {
    let key = "wiki/wikis";
    if rest.len() == 2 && method == Method::GET {
        let value = list(state, org, project, key)
            .into_iter()
            .map(|item| dress_wiki(org, project, item))
            .collect();
        return Json(page(value)).into_response();
    }
    if rest.len() == 2 && method == Method::POST {
        let payload = parse_json(body).unwrap_or(json!({}));
        let name = text_of(&payload, "name").unwrap_or_else(|| format!("{project}.wiki"));
        let next = list(state, org, project, key).len() as i64 + 1;
        let id = format!("00000000-0000-0000-0000-{next:012x}");
        let item = dress_wiki(org, project, json!({"id": id, "name": name, "type": "projectWiki"}));
        put_item(state, org, project, key, item.clone());
        return Json(item).into_response();
    }
    let wiki_id = rest.get(2).map(String::as_str).unwrap_or("");
    if rest.len() >= 4 && rest[3].eq_ignore_ascii_case("pages") && (method == Method::GET || method == Method::PUT || method == Method::POST) {
        let resolved = resolve_wiki(state, org, project, wiki_id);
        let Some(wiki) = resolved else {
            return fail(StatusCode::NOT_FOUND, "wiki introuvable");
        };
        let wiki_key = value_key(&wiki["id"]);
        let pages = format!("wiki/wikis/{wiki_key}/pages");
        let path = query_param(query, "path")
            .map(|value| crate::http::pct_decode(value))
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| "/".into());
        if method == Method::GET && query_param(query, "path").is_none() {
            return Json(page(list(state, org, project, &pages))).into_response();
        }
        if method == Method::GET {
            return match get_item(state, org, project, &pages, &path) {
                Some(item) => Json(item).into_response(),
                None => fail(StatusCode::NOT_FOUND, "page introuvable"),
            };
        }
        let payload = parse_json(body).unwrap_or(json!({}));
        let content = text_of(&payload, "content").unwrap_or_default();
        let item = json!({
            "id": path,
            "path": path,
            "content": content,
            "gitItemPath": path,
            "url": format!("/{org}/{project}/_apis/wiki/wikis/{wiki_key}/pages?path={path}")
        });
        put_item(state, org, project, &pages, item.clone());
        return Json(item).into_response();
    }
    if method == Method::GET {
        return match resolve_wiki(state, org, project, wiki_id) {
            Some(item) => Json(dress_wiki(org, project, item)).into_response(),
            None => fail(StatusCode::NOT_FOUND, "wiki introuvable"),
        };
    }
    fail(StatusCode::NOT_FOUND, "wiki introuvable")
}

fn feeds(state: &AppState, method: &Method, org: &str, project: &str, rest: &[String], body: &[u8]) -> Response {
    let key = "packaging/feeds";
    if rest.len() == 2 && method == Method::GET {
        let value = list(state, org, project, key)
            .into_iter()
            .map(|item| dress_feed(org, project, item))
            .collect();
        return Json(page(value)).into_response();
    }
    if rest.len() == 2 && method == Method::POST {
        let payload = parse_json(body).unwrap_or(json!({}));
        let name = text_of(&payload, "name").unwrap_or_else(|| "feed".into());
        let next = list(state, org, project, key).len() as i64 + 1;
        let id = format!("feed-{next:08}");
        let item = dress_feed(org, project, json!({"id": id, "name": name}));
        put_item(state, org, project, key, item.clone());
        return Json(item).into_response();
    }
    let feed_id = rest.get(2).map(String::as_str).unwrap_or("");
    if rest.len() >= 4 && rest[3].eq_ignore_ascii_case("packages") {
        let packages = format!("packaging/feeds/{feed_id}/packages");
        if method == Method::GET {
            return Json(page(list(state, org, project, &packages))).into_response();
        }
        if method == Method::POST {
            let payload = parse_json(body).unwrap_or(json!({}));
            let name = text_of(&payload, "name").unwrap_or_else(|| "package".into());
            let next = list(state, org, project, &packages).len() as i64 + 1;
            let item = json!({"id": format!("pkg-{next}"), "name": name, "versions": [{"version": text_of(&payload, "version").unwrap_or_else(|| "1.0.0".into())}]});
            put_item(state, org, project, &packages, item.clone());
            return Json(item).into_response();
        }
    }
    if method == Method::GET {
        return match get_item(state, org, project, key, feed_id) {
            Some(item) => Json(dress_feed(org, project, item)).into_response(),
            None => fail(StatusCode::NOT_FOUND, "flux introuvable"),
        };
    }
    fail(StatusCode::NOT_FOUND, "flux introuvable")
}

fn release_definitions(state: &AppState, method: &Method, org: &str, project: &str, rest: &[String], body: &[u8]) -> Response {
    let key = "release/definitions";
    if rest.len() == 2 && method == Method::GET {
        return Json(page(list(state, org, project, key))).into_response();
    }
    if rest.len() == 2 && method == Method::POST {
        let payload = parse_json(body).unwrap_or(json!({}));
        let name = text_of(&payload, "name").unwrap_or_else(|| "Version".into());
        let next = list(state, org, project, key).len() as i64 + 1;
        let environments = payload
            .get("environments")
            .and_then(|value| value.as_array())
            .cloned()
            .filter(|rows| !rows.is_empty())
            .unwrap_or_else(|| vec![json!({"name": "Production", "rank": 1})]);
        let environments: Vec<Value> = environments
            .into_iter()
            .enumerate()
            .map(|(index, mut environment)| {
                let rank = environment.get("rank").and_then(|value| value.as_i64()).unwrap_or(index as i64 + 1);
                if let Some(object) = environment.as_object_mut() {
                    object.insert("id".into(), json!(index as i64 + 1));
                    object.entry("name").or_insert(json!(format!("Environnement {rank}")));
                    object.insert("rank".into(), json!(rank));
                }
                environment
            })
            .collect();
        let item = json!({
            "id": next,
            "name": name,
            "path": "\\",
            "releaseNameFormat": "Release-$(rev:r)",
            "environments": environments,
            "url": format!("/{org}/{project}/_apis/release/definitions/{next}")
        });
        put_item(state, org, project, key, item.clone());
        return Json(item).into_response();
    }
    let id = rest.get(2).map(String::as_str).unwrap_or("");
    if method == Method::GET {
        return match get_item(state, org, project, key, id) {
            Some(item) => Json(item).into_response(),
            None => fail(StatusCode::NOT_FOUND, "définition introuvable"),
        };
    }
    fail(StatusCode::NOT_FOUND, "définition introuvable")
}

fn release_releases(state: &AppState, method: &Method, org: &str, project: &str, rest: &[String], body: &[u8]) -> Response {
    let key = "release/releases";
    if rest.len() == 2 && method == Method::GET {
        return Json(page(list(state, org, project, key))).into_response();
    }
    if rest.len() == 2 && method == Method::POST {
        let payload = parse_json(body).unwrap_or(json!({}));
        let definition_id = payload
            .get("definitionId")
            .and_then(|value| value.as_i64())
            .or_else(|| payload.get("definition").and_then(|value| value.get("id")).and_then(|value| value.as_i64()))
            .unwrap_or(0);
        let Some(definition) = get_item(state, org, project, "release/definitions", &definition_id.to_string()) else {
            return fail(StatusCode::NOT_FOUND, "définition introuvable");
        };
        let next = list(state, org, project, key).len() as i64 + 1;
        let description = text_of(&payload, "description").unwrap_or_default();
        let source = definition.get("environments").and_then(|value| value.as_array()).cloned().unwrap_or_default();
        let (environments, approvals) = open_release_environments(&source);
        let definition_name = text_of(&definition, "name").unwrap_or_else(|| "Version".into());
        let release_name = format!("Release-{next}");
        let item = json!({
            "id": next,
            "name": release_name,
            "status": "active",
            "description": description,
            "releaseDefinition": {"id": definition_id, "name": definition_name},
            "environments": environments,
            "artifacts": payload.get("artifacts").cloned().unwrap_or(json!([])),
            "createdBy": {"displayName": "azurz"},
            "url": format!("/{org}/{project}/_apis/release/releases/{next}")
        });
        put_item(state, org, project, key, item.clone());
        for mut approval in approvals {
            let approval_id = list(state, org, project, "release/approvals").len() as i64 + 1;
            if let Some(object) = approval.as_object_mut() {
                object.insert("id".into(), json!(approval_id));
                object.insert("release".into(), json!({"id": next, "name": release_name}));
                object.insert("releaseDefinition".into(), json!({"id": definition_id, "name": definition_name}));
            }
            put_item(state, org, project, "release/approvals", approval);
        }
        return Json(item).into_response();
    }
    let id = rest.get(2).map(String::as_str).unwrap_or("");
    if method == Method::GET {
        return match get_item(state, org, project, key, id) {
            Some(item) => Json(item).into_response(),
            None => fail(StatusCode::NOT_FOUND, "version introuvable"),
        };
    }
    fail(StatusCode::NOT_FOUND, "version introuvable")
}

fn release_approvals(state: &AppState, method: &Method, org: &str, project: &str, rest: &[String], body: &[u8]) -> Response {
    let key = "release/approvals";
    if rest.len() == 2 && method == Method::GET {
        return Json(page(list(state, org, project, key))).into_response();
    }
    let id = rest.get(2).map(String::as_str).unwrap_or("");
    if method == Method::GET {
        return match get_item(state, org, project, key, id) {
            Some(item) => Json(item).into_response(),
            None => fail(StatusCode::NOT_FOUND, "approbation introuvable"),
        };
    }
    if method != Method::PATCH && method != Method::PUT {
        return fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée");
    }
    let payload = parse_json(body).unwrap_or(json!({}));
    let status = text_of(&payload, "status").unwrap_or_else(|| "approved".into());
    let comments = text_of(&payload, "comments").unwrap_or_default();
    let updated = state.write(|db| {
        let stored = db.project_mut(org, project)?;
        let mut approval = stored.get_bucket(key, id)?;
        if let Some(object) = approval.as_object_mut() {
            object.insert("status".into(), json!(status));
            object.insert("comments".into(), json!(comments));
        }
        let release_id = approval.get("release").and_then(|release| release.get("id")).map(crate::store::value_key).unwrap_or_default();
        let environment_id = approval.get("releaseEnvironment").and_then(|environment| environment.get("id")).and_then(|value| value.as_i64());
        if let Some(mut release) = stored.get_bucket("release/releases", &release_id) {
            if let Some(environments) = release.get_mut("environments").and_then(|value| value.as_array_mut()) {
                for environment in environments.iter_mut() {
                    if environment.get("id").and_then(|value| value.as_i64()) == environment_id {
                        environment["status"] = json!(if status.eq_ignore_ascii_case("approved") { "succeeded" } else { "rejected" });
                    }
                }
            }
            if status.eq_ignore_ascii_case("approved") {
                let mut fresh = Vec::new();
                if let Some(environments) = release.get_mut("environments").and_then(|value| value.as_array_mut()) {
                    let mut gate_open = true;
                    for environment in environments.iter_mut() {
                        let current = text_of(environment, "status").unwrap_or_default();
                        if current == "succeeded" || current == "canceled" || current == "rejected" || current == "partiallySucceeded" {
                            continue;
                        }
                        if !gate_open {
                            break;
                        }
                        if current == "inProgress" {
                            break;
                        }
                        if environment_needs_approval(environment) {
                            environment["status"] = json!("inProgress");
                            fresh.push(environment.clone());
                            gate_open = false;
                        } else {
                            environment["status"] = json!("succeeded");
                        }
                    }
                }
                let release_name = text_of(&release, "name").unwrap_or_default();
                let definition = release.get("releaseDefinition").cloned().unwrap_or(json!({}));
                stored.put_bucket("release/releases", release);
                for environment in fresh {
                    let approval_id = stored.list_bucket(key).len() as i64 + 1;
                    let approver = environment
                        .get("preDeployApprovals")
                        .and_then(|value| value.get("approvals"))
                        .and_then(|value| value.as_array())
                        .and_then(|rows| rows.first())
                        .and_then(|row| row.get("approver"))
                        .cloned()
                        .unwrap_or(json!({"displayName": "azurz", "uniqueName": "azurz"}));
                    stored.put_bucket(
                        key,
                        json!({
                            "id": approval_id,
                            "status": "pending",
                            "approvalType": "preDeploy",
                            "approver": approver,
                            "release": {"id": release_id.parse::<i64>().unwrap_or(0), "name": release_name},
                            "releaseDefinition": definition,
                            "releaseEnvironment": {"id": environment.get("id").cloned().unwrap_or(json!(0)), "name": text_of(&environment, "name").unwrap_or_default()}
                        }),
                    );
                }
            } else {
                stored.put_bucket("release/releases", release);
            }
        }
        Some(stored.put_bucket(key, approval))
    });
    match updated {
        Some(item) => Json(item).into_response(),
        None => fail(StatusCode::NOT_FOUND, "approbation introuvable"),
    }
}

fn open_release_environments(source: &[Value]) -> (Vec<Value>, Vec<Value>) {
    let mut environments = Vec::new();
    let mut approvals = Vec::new();
    let mut gate_open = true;
    for (index, environment) in source.iter().enumerate() {
        let rank = environment.get("rank").and_then(|value| value.as_i64()).unwrap_or(index as i64 + 1);
        let name = text_of(environment, "name").unwrap_or_else(|| format!("Environnement {rank}"));
        let id = environment.get("id").and_then(|value| value.as_i64()).unwrap_or(index as i64 + 1);
        let mut status = "notStarted".to_string();
        if gate_open {
            if environment_needs_approval(environment) {
                status = "inProgress".into();
                let approver = environment
                    .get("preDeployApprovals")
                    .and_then(|value| value.get("approvals"))
                    .and_then(|value| value.as_array())
                    .and_then(|rows| rows.first())
                    .and_then(|row| row.get("approver"))
                    .cloned()
                    .unwrap_or(json!({"displayName": "azurz", "uniqueName": "azurz"}));
                approvals.push(json!({
                    "status": "pending",
                    "approvalType": "preDeploy",
                    "approver": approver,
                    "releaseEnvironment": {"id": id, "name": name}
                }));
                gate_open = false;
            } else {
                status = "succeeded".into();
            }
        }
        environments.push(json!({
            "id": id,
            "name": name,
            "rank": rank,
            "status": status,
            "preDeployApprovals": environment.get("preDeployApprovals").cloned().unwrap_or(json!({"approvals": []}))
        }));
    }
    (environments, approvals)
}

fn environment_needs_approval(environment: &Value) -> bool {
    environment
        .get("preDeployApprovals")
        .and_then(|value| value.get("approvals"))
        .and_then(|value| value.as_array())
        .is_some_and(|rows| !rows.is_empty())
}

fn subscriptions(state: &AppState, method: &Method, org: &str, rest: &[String], body: &[u8]) -> Response {
    let key = "hooks/subscriptions";
    if rest.len() == 2 && method == Method::GET {
        return Json(page(org_list(state, org, key))).into_response();
    }
    if rest.len() == 2 && method == Method::POST {
        let payload = parse_json(body).unwrap_or(json!({}));
        let next = org_list(state, org, key).len() as i64 + 1;
        let mut item = payload;
        if let Some(object) = item.as_object_mut() {
            object.insert("id".into(), json!(format!("sub-{next}")));
            object.entry("status").or_insert(json!("enabled"));
            object.entry("publisherId").or_insert(json!("tfs"));
            object.entry("url").or_insert(json!(format!("/{org}/_apis/hooks/subscriptions/sub-{next}")));
        }
        org_put(state, org, key, item.clone());
        return Json(item).into_response();
    }
    let id = rest.get(2).cloned().unwrap_or_default();
    if rest.get(3).is_some_and(|value| value.eq_ignore_ascii_case("notifications")) && method == Method::GET {
        return Json(page(org_list(state, org, &format!("hooks/subscriptions/{id}/notifications")))).into_response();
    }
    if method == Method::GET && rest.len() == 3 {
        return match org_get(state, org, key, &id) {
            Some(item) => Json(item).into_response(),
            None => fail(StatusCode::NOT_FOUND, "abonnement introuvable"),
        };
    }
    if method == Method::DELETE && rest.len() == 3 {
        let removed = state.write(|db| {
            db.orgs
                .get_mut(org)
                .and_then(|organization| organization.buckets.get_mut(key))
                .map(|bucket| bucket.remove(&id))
                .unwrap_or(false)
        });
        return if removed {
            StatusCode::NO_CONTENT.into_response()
        } else {
            fail(StatusCode::NOT_FOUND, "abonnement introuvable")
        };
    }
    fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée")
}

pub fn receive_hook(state: &AppState, body: &[u8]) -> Response {
    let (org, _) = state.names();
    let payload = parse_json(body).unwrap_or(json!({"raw": String::from_utf8_lossy(body)}));
    store_inbox(state, &org, payload);
    StatusCode::NO_CONTENT.into_response()
}

fn hooks_inbox(state: &AppState, method: &Method, org: &str) -> Response {
    if method != Method::GET {
        return fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée");
    }
    Json(page(org_list(state, org, "hooks/inbox"))).into_response()
}

pub fn deliver_work_item(state: &AppState, org: &str, project: &str, event_type: &str, resource: &Value) {
    let matches = org_list(state, org, "hooks/subscriptions")
        .into_iter()
        .filter(|item| subscription_matches(item, event_type, project, resource))
        .collect::<Vec<_>>();
    for subscription in matches {
        let id = text_of(&subscription, "id").unwrap_or_default();
        let url = subscription
            .get("consumerInputs")
            .and_then(|value| text_of(value, "url"))
            .unwrap_or_default();
        let key = format!("hooks/subscriptions/{id}/notifications");
        let next = org_list(state, org, &key).len() as i64 + 1;
        let payload = json!({
            "subscriptionId": id,
            "notificationId": next,
            "eventType": event_type,
            "publisherId": text_of(&subscription, "publisherId").unwrap_or_else(|| "tfs".into()),
            "resource": resource,
            "createdDate": "2026-10-29T00:00:00Z"
        });
        let (ok, detail) = dispatch_consumer(state, org, &url, &payload);
        org_put(
            state,
            org,
            &key,
            json!({
                "id": next,
                "subscriptionId": id,
                "eventType": event_type,
                "status": if ok { "succeeded" } else { "failed" },
                "result": if ok { "succeeded" } else { "failed" },
                "details": detail,
                "resource": resource,
                "createdDate": "2026-10-29T00:00:00Z"
            }),
        );
    }
}

fn subscription_matches(item: &Value, event_type: &str, project: &str, resource: &Value) -> bool {
    if text_of(item, "status").as_deref() == Some("disabled") {
        return false;
    }
    if text_of(item, "eventType").as_deref() != Some(event_type) {
        return false;
    }
    let Some(inputs) = item.get("publisherInputs").and_then(|value| value.as_object()) else {
        return true;
    };
    for key in ["projectId", "projectName"] {
        if let Some(value) = inputs.get(key).and_then(|value| value.as_str()) {
            if !value.is_empty() && value != project {
                return false;
            }
        }
    }
    if let Some(wanted) = inputs.get("repository").and_then(|value| value.as_str()).filter(|value| !value.is_empty()) {
        let repository = resource.get("repository");
        let id = repository.and_then(|value| text_of(value, "id"));
        let name = repository.and_then(|value| text_of(value, "name"));
        if id.as_deref() != Some(wanted) && name.as_deref() != Some(wanted) {
            return false;
        }
    }
    if let Some(wanted) = inputs.get("definitionName").and_then(|value| value.as_str()).filter(|value| !value.is_empty()) {
        let name = resource.get("definition").and_then(|value| text_of(value, "name")).unwrap_or_default();
        if name != wanted {
            return false;
        }
    }
    true
}

fn dispatch_consumer(state: &AppState, org: &str, url: &str, payload: &Value) -> (bool, String) {
    if url.is_empty() {
        return (false, "url absente".into());
    }
    if url == "/_hooks/inbox" {
        store_inbox(state, org, payload.clone());
        return (true, "livré".into());
    }
    if let Some(target) = url.strip_prefix("http://") {
        return match post_http(target, payload) {
            Ok(code) => (true, format!("HTTP {code}")),
            Err(message) => (false, message),
        };
    }
    (false, "url inconnue".into())
}

fn store_inbox(state: &AppState, org: &str, mut payload: Value) {
    let next = org_list(state, org, "hooks/inbox").len() as i64 + 1;
    if let Some(object) = payload.as_object_mut() {
        object.insert("id".into(), json!(next));
    }
    org_put(state, org, "hooks/inbox", payload);
}

fn post_http(target: &str, payload: &Value) -> Result<u16, String> {
    use std::io::{Read, Write};
    use std::net::{TcpStream, ToSocketAddrs};
    use std::time::Duration;
    let (authority, path) = target.split_once('/').map(|(host, path)| (host, format!("/{path}"))).unwrap_or((target, "/".into()));
    let (host, port) = authority.rsplit_once(':').unwrap_or((authority, "80"));
    let port = port.parse::<u16>().map_err(|_| format!("port invalide: {authority}"))?;
    let address = format!("{host}:{port}")
        .to_socket_addrs()
        .map_err(|error| error.to_string())?
        .next()
        .ok_or_else(|| format!("adresse introuvable: {host}:{port}"))?;
    let mut stream = TcpStream::connect_timeout(&address, Duration::from_secs(1)).map_err(|error| error.to_string())?;
    stream.set_read_timeout(Some(Duration::from_secs(1))).map_err(|error| error.to_string())?;
    stream.set_write_timeout(Some(Duration::from_secs(1))).map_err(|error| error.to_string())?;
    let body = serde_json::to_string(payload).unwrap_or_else(|_| "{}".into());
    let request = format!(
        "POST {path} HTTP/1.1\r\nHost: {authority}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    stream.write_all(request.as_bytes()).map_err(|error| error.to_string())?;
    let mut buffer = [0u8; 128];
    let read = stream.read(&mut buffer).unwrap_or(0);
    let head = String::from_utf8_lossy(&buffer[..read]);
    let code = head.split_whitespace().nth(1).and_then(|value| value.parse().ok()).unwrap_or(0);
    if (200..300).contains(&code) {
        Ok(code)
    } else if read == 0 {
        Err("réponse vide".into())
    } else {
        Err(format!("HTTP {code}"))
    }
}

fn graph(state: &AppState, method: &Method, org: &str, key: &str, name: impl AsRef<str>) -> Response {
    if method != Method::GET {
        return fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée");
    }
    let name = name.as_ref().to_string();
    if org_list(state, org, key).is_empty() {
        org_put(
            state,
            org,
            key,
            json!({"id": format!("{key}-1"), "displayName": name, "principalName": format!("{name}@azurz.lab")}),
        );
    }
    Json(page(org_list(state, org, key))).into_response()
}

fn work_item_types() -> Value {
    let kinds = [
        ("Epic", "Microsoft.VSTS.WorkItemTypes.Epic", "E06C00"),
        ("Feature", "Microsoft.VSTS.WorkItemTypes.Feature", "773B93"),
        ("User Story", "Microsoft.VSTS.WorkItemTypes.UserStory", "0098C7"),
        ("Bug", "Microsoft.VSTS.WorkItemTypes.Bug", "CC293D"),
        ("Task", "Microsoft.VSTS.WorkItemTypes.Task", "F2CB1D"),
        ("Issue", "Microsoft.VSTS.WorkItemTypes.Issue", "B4009E"),
        ("Test Case", "Microsoft.VSTS.WorkItemTypes.TestCase", "004B50"),
    ];
    let value: Vec<Value> = kinds
        .iter()
        .map(|(name, reference, color)| {
            json!({
                "name": name,
                "referenceName": reference,
                "description": name,
                "color": color,
                "isDisabled": false,
                "xmlForm": "",
                "fields": []
            })
        })
        .collect();
    page(value)
}

fn wit_fields() -> Value {
    let fields = [
        ("System.Id", "integer", "Identifiant"),
        ("System.Title", "string", "Titre"),
        ("System.State", "string", "État"),
        ("System.Description", "html", "Description"),
        ("System.WorkItemType", "string", "Type"),
        ("System.Tags", "string", "Tags"),
        ("System.AssignedTo", "identity", "Assigné à"),
        ("System.AreaPath", "treePath", "Chemin de zone"),
        ("System.IterationPath", "treePath", "Chemin d'itération"),
        ("System.TeamProject", "string", "Projet"),
        ("System.CreatedBy", "identity", "Créé par"),
        ("System.ChangedDate", "dateTime", "Modifié le"),
    ];
    let value: Vec<Value> = fields
        .iter()
        .map(|(reference, kind, name)| json!({"referenceName": reference, "name": name, "type": kind, "readOnly": false}))
        .collect();
    page(value)
}

pub fn classification_nodes(
    state: &AppState,
    method: &Method,
    org: &str,
    project: &str,
    rest: &[String],
    query: &str,
    body: &[u8],
) -> Response {
    ensure_classification(state, org, project);
    let kind = rest.get(2).map(|value| value.to_ascii_lowercase()).unwrap_or_default();
    if kind != "areas" && kind != "iterations" {
        if rest.len() == 2 && method == Method::GET {
            let depth = classification_depth(query);
            let value = ["areas", "iterations"]
                .into_iter()
                .filter_map(|group| classification_tree(state, org, project, group, "", depth))
                .collect();
            return Json(page(value)).into_response();
        }
        return fail(StatusCode::NOT_FOUND, "nœud introuvable");
    }
    let relative = rest.get(3..).map(|parts| parts.join("\\")).unwrap_or_default();
    let root = classification_root(project, &kind);
    let path = if relative.is_empty() { root.clone() } else { format!("{root}\\{relative}") };
    if method == Method::GET {
        let depth = classification_depth(query);
        return match classification_tree(state, org, project, &kind, &path, depth) {
            Some(node) => Json(node).into_response(),
            None => fail(StatusCode::NOT_FOUND, "nœud introuvable"),
        };
    }
    if method == Method::POST {
        let payload = parse_json(body).unwrap_or(json!({}));
        let name = text_of(&payload, "name").unwrap_or_default();
        if name.trim().is_empty() || name.contains('\\') || name.contains('/') {
            return fail(StatusCode::BAD_REQUEST, "nom de nœud absent");
        }
        if classification_nodes_of(state, org, project, &kind).iter().any(|node| {
            node_parent(node) == path && text_of(node, "name").is_some_and(|current| current.eq_ignore_ascii_case(&name))
        }) {
            return fail(StatusCode::BAD_REQUEST, "nœud existant");
        }
        if classification_by_path(state, org, project, &kind, &path).is_none() {
            return fail(StatusCode::NOT_FOUND, "nœud introuvable");
        }
        let structure = if kind == "iterations" { "iteration" } else { "area" };
        let created = state.write(|db| {
            let stored = db.project_mut(org, project)?;
            let key = classification_key(&kind);
            let next = stored
                .list_bucket(&key)
                .iter()
                .filter_map(|node| node.get("id").and_then(|value| value.as_i64()))
                .max()
                .unwrap_or(3)
                + 1;
            let mut item = json!({
                "id": next,
                "identifier": format!("node-{next}"),
                "name": name,
                "structureType": structure,
                "path": format!("{path}\\{name}"),
                "url": format!("/{org}/{project}/_apis/wit/classificationnodes/{kind}")
            });
            if let Some(attributes) = payload.get("attributes").filter(|value| value.is_object()) {
                if let Some(object) = item.as_object_mut() {
                    object.insert("attributes".into(), attributes.clone());
                }
            }
            Some(stored.put_bucket(&key, item))
        });
        return match created {
            Some(item) => Json(dress_classification(item, Vec::new())).into_response(),
            None => fail(StatusCode::NOT_FOUND, "projet inconnu"),
        };
    }
    if path == root {
        return fail(StatusCode::BAD_REQUEST, "nœud racine");
    }
    if method == Method::PATCH || method == Method::PUT {
        let payload = parse_json(body).unwrap_or(json!({}));
        let Some(current) = classification_by_path(state, org, project, &kind, &path) else {
            return fail(StatusCode::NOT_FOUND, "nœud introuvable");
        };
        let old_path = text_of(&current, "path").unwrap_or(path.clone());
        let new_name = text_of(&payload, "name").unwrap_or_else(|| text_of(&current, "name").unwrap_or_default());
        if new_name.trim().is_empty() || new_name.contains('\\') || new_name.contains('/') {
            return fail(StatusCode::BAD_REQUEST, "nom de nœud absent");
        }
        let parent = node_parent(&current);
        let new_path = format!("{parent}\\{new_name}");
        let updated = state.write(|db| {
            let stored = db.project_mut(org, project)?;
            let key = classification_key(&kind);
            let nodes = stored.list_bucket(&key);
            let mut renamed = None;
            for mut node in nodes {
                let node_path = text_of(&node, "path").unwrap_or_default();
                let next_path = if node_path == old_path {
                    new_path.clone()
                } else if let Some(rest) = node_path.strip_prefix(&format!("{old_path}\\")) {
                    format!("{new_path}\\{rest}")
                } else {
                    continue;
                };
                if let Some(object) = node.as_object_mut() {
                    object.insert("path".into(), json!(next_path));
                    if node_path == old_path {
                        object.insert("name".into(), json!(new_name));
                        if let Some(attributes) = payload.get("attributes").filter(|value| value.is_object()) {
                            object.insert("attributes".into(), attributes.clone());
                        }
                    }
                }
                let saved = stored.put_bucket(&key, node);
                if node_path == old_path {
                    renamed = Some(saved);
                }
            }
            renamed
        });
        return match updated {
            Some(item) => Json(dress_classification(item, Vec::new())).into_response(),
            None => fail(StatusCode::NOT_FOUND, "nœud introuvable"),
        };
    }
    if method == Method::DELETE {
        let removed = state.write(|db| {
            let stored = db.project_mut(org, project)?;
            let key = classification_key(&kind);
            let ids: Vec<String> = stored
                .list_bucket(&key)
                .into_iter()
                .filter(|node| {
                    let node_path = text_of(node, "path").unwrap_or_default();
                    node_path == path || node_path.starts_with(&format!("{path}\\"))
                })
                .map(|node| value_key(&node["id"]))
                .filter(|id| !id.is_empty())
                .collect();
            if ids.is_empty() {
                return Some(false);
            }
            if let Some(bucket) = stored.buckets.get_mut(&key) {
                for id in ids {
                    bucket.remove(&id);
                }
            }
            Some(true)
        });
        return match removed {
            Some(true) => StatusCode::NO_CONTENT.into_response(),
            Some(false) => fail(StatusCode::NOT_FOUND, "nœud introuvable"),
            None => fail(StatusCode::NOT_FOUND, "projet inconnu"),
        };
    }
    fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée")
}

fn ensure_classification(state: &AppState, org: &str, project: &str) {
    state.write(|db| {
        db.ensure_project(org, project);
        let Some(stored) = db.project_mut(org, project) else {
            return;
        };
        for (kind, id, identifier, structure, path_suffix) in [
            ("areas", 1, "area", "area", "Area"),
            ("iterations", 2, "iteration", "iteration", "Iteration"),
        ] {
            let key = classification_key(kind);
            if stored.list_bucket(&key).is_empty() {
                stored.put_bucket(
                    &key,
                    json!({
                        "id": id,
                        "identifier": identifier,
                        "name": project,
                        "structureType": structure,
                        "path": format!("\\{project}\\{path_suffix}"),
                        "url": format!("/{org}/{project}/_apis/wit/classificationnodes/{kind}")
                    }),
                );
            }
        }
        let iterations = classification_key("iterations");
        let sprint = format!("\\{project}\\Iteration\\Sprint 1");
        if stored.get_bucket(&iterations, "3").is_none()
            && stored.list_bucket(&iterations).iter().all(|node| text_of(node, "path").as_deref() != Some(sprint.as_str()))
        {
            stored.put_bucket(
                &iterations,
                json!({
                    "id": 3,
                    "identifier": "node-3",
                    "name": "Sprint 1",
                    "structureType": "iteration",
                    "path": sprint,
                    "url": format!("/{org}/{project}/_apis/wit/classificationnodes/iterations")
                }),
            );
        }
    });
}

fn classification_tree(state: &AppState, org: &str, project: &str, kind: &str, path: &str, depth: usize) -> Option<Value> {
    let target = if path.is_empty() { classification_root(project, kind) } else { path.to_string() };
    let node = classification_by_path(state, org, project, kind, &target)?;
    let children = if depth == 0 {
        Vec::new()
    } else {
        classification_nodes_of(state, org, project, kind)
            .into_iter()
            .filter(|child| node_parent(child) == target)
            .filter_map(|child| {
                let child_path = text_of(&child, "path")?;
                classification_tree(state, org, project, kind, &child_path, depth - 1)
            })
            .collect()
    };
    Some(dress_classification(node, children))
}

fn dress_classification(mut item: Value, children: Vec<Value>) -> Value {
    if let Some(object) = item.as_object_mut() {
        object.insert("hasChildren".into(), json!(!children.is_empty()));
        if children.is_empty() {
            object.remove("children");
        } else {
            object.insert("children".into(), json!(children));
        }
    }
    item
}

fn classification_nodes_of(state: &AppState, org: &str, project: &str, kind: &str) -> Vec<Value> {
    list(state, org, project, &classification_key(kind))
}

fn classification_by_path(state: &AppState, org: &str, project: &str, kind: &str, path: &str) -> Option<Value> {
    classification_nodes_of(state, org, project, kind)
        .into_iter()
        .find(|node| text_of(node, "path").as_deref() == Some(path))
}

fn classification_root(project: &str, kind: &str) -> String {
    let suffix = if kind == "iterations" { "Iteration" } else { "Area" };
    format!("\\{project}\\{suffix}")
}

fn classification_key(kind: &str) -> String {
    format!("wit/classification/{kind}")
}

fn node_parent(node: &Value) -> String {
    text_of(node, "path")
        .and_then(|path| path.rsplit_once('\\').map(|(parent, _)| parent.to_string()))
        .unwrap_or_default()
}

fn classification_depth(query: &str) -> usize {
    query_param(query, "$depth")
        .or_else(|| query_param(query, "depth"))
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(1)
}

fn dress_repo(org: &str, project: &str, mut item: Value) -> Value {
    let id = value_key(&item["id"]);
    let name = text_of(&item, "name").unwrap_or_else(|| project.to_string());
    if let Some(object) = item.as_object_mut() {
        object.insert("id".into(), json!(id));
        object.insert("name".into(), json!(name));
        object.entry("defaultBranch").or_insert(json!("refs/heads/main"));
        object.entry("size").or_insert(json!(0));
        object.entry("isDisabled").or_insert(json!(false));
        object.entry("isFork").or_insert(json!(false));
        object.entry("project").or_insert(json!({"name": project, "state": "wellFormed", "visibility": "private"}));
        let remote = format!("https://dev.azure.com/{org}/{project}/_git/{}", object["name"].as_str().unwrap_or(project));
        let repo_name = object["name"].as_str().unwrap_or(project).to_string();
        let repo_id = object["id"].as_str().unwrap_or("").to_string();
        object.entry("remoteUrl").or_insert(json!(remote));
        object.entry("webUrl").or_insert(json!(format!("https://dev.azure.com/{org}/{project}/_git/{repo_name}")));
        object.entry("sshUrl").or_insert(json!(format!("git@ssh.dev.azure.com:v3/{org}/{project}/{repo_name}")));
        object.entry("url").or_insert(json!(format!("/{org}/{project}/_apis/git/repositories/{repo_id}")));
    }
    item
}

fn dress_definition(project: &str, mut item: Value) -> Value {
    if let Some(object) = item.as_object_mut() {
        object.entry("path").or_insert(json!("\\"));
        object.entry("type").or_insert(json!("build"));
        object.entry("queueStatus").or_insert(json!("enabled"));
        object.entry("revision").or_insert(json!(1));
        object.entry("project").or_insert(json!({"name": project}));
    }
    item
}

fn dress_plan(project: &str, mut item: Value) -> Value {
    let name = text_of(&item, "name").unwrap_or_else(|| "Plan".into());
    let id = item.get("id").cloned().unwrap_or(json!(1));
    if let Some(object) = item.as_object_mut() {
        object.entry("state").or_insert(json!("Active"));
        object.entry("areaPath").or_insert(json!(project));
        object.entry("iteration").or_insert(json!(format!("{project}\\Sprint 1")));
        object.entry("rootSuite").or_insert(json!({"id": id, "name": name}));
    }
    item
}

fn dress_wiki(org: &str, project: &str, mut item: Value) -> Value {
    let id = value_key(&item["id"]);
    let name = text_of(&item, "name").unwrap_or_else(|| format!("{project}.wiki"));
    if let Some(object) = item.as_object_mut() {
        object.insert("id".into(), json!(id));
        object.insert("name".into(), json!(name));
        let wiki_id = object["id"].as_str().unwrap_or("").to_string();
        let wiki_name = object["name"].as_str().unwrap_or(project).to_string();
        object.entry("type").or_insert(json!("projectWiki"));
        object.entry("url").or_insert(json!(format!("/{org}/{project}/_apis/wiki/wikis/{wiki_id}")));
        object.entry("remoteUrl").or_insert(json!(format!("https://dev.azure.com/{org}/{project}/_git/{wiki_name}")));
    }
    item
}

fn dress_feed(org: &str, project: &str, mut item: Value) -> Value {
    let name = text_of(&item, "name").unwrap_or_else(|| "feed".into());
    if let Some(object) = item.as_object_mut() {
        object.entry("fullyQualifiedName").or_insert(json!(format!("{org}/{project}/{name}")));
        object.entry("upstreamEnabled").or_insert(json!(false));
        object.entry("capabilities").or_insert(json!("default"));
        object.entry("url").or_insert(json!(format!("/{org}/{project}/_apis/packaging/feeds/{name}")));
    }
    item
}

fn policy_type_catalog() -> Vec<Value> {
    vec![
        json!({"id": "fa4e907d-c16b-4a4c-9dfa-4906e5d171dd", "displayName": "Minimum number of reviewers"}),
        json!({"id": "0609b952-1397-4640-95ec-e00a01b2c241", "displayName": "Build"}),
        json!({"id": "cbdc66da-9728-4af8-aada-9a5a32e4a226", "displayName": "Status"}),
        json!({"id": "c6a1889d-b943-4856-b76f-9e46bb6b0df2", "displayName": "Comment requirements"}),
    ]
}

fn policy_types(method: &Method, org: &str, project: &str, rest: &[String]) -> Response {
    if method != Method::GET {
        return fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée");
    }
    let types: Vec<Value> = policy_type_catalog()
        .into_iter()
        .map(|mut item| {
            let id = value_key(&item["id"]);
            if let Some(object) = item.as_object_mut() {
                object.insert("url".into(), json!(format!("/{org}/{project}/_apis/policy/types/{id}")));
            }
            item
        })
        .collect();
    if rest.len() >= 3 {
        let id = &rest[2];
        return match types.into_iter().find(|item| value_key(&item["id"]) == *id) {
            Some(item) => Json(item).into_response(),
            None => fail(StatusCode::NOT_FOUND, "type de stratégie introuvable"),
        };
    }
    Json(page(types)).into_response()
}

fn policy_configurations(state: &AppState, method: &Method, org: &str, project: &str, rest: &[String], body: &[u8]) -> Response {
    let key = "policy/configurations";
    if rest.len() == 2 && method == Method::GET {
        let value = list(state, org, project, key)
            .into_iter()
            .map(|item| dress_policy(org, project, item))
            .collect();
        return Json(page(value)).into_response();
    }
    if rest.len() == 2 && method == Method::POST {
        let payload = parse_json(body).unwrap_or(json!({}));
        let type_id = text_of(&payload["type"], "id").unwrap_or_default();
        if policy_type_catalog().iter().all(|item| text_of(item, "id").as_deref() != Some(type_id.as_str())) {
            return fail(StatusCode::BAD_REQUEST, "type de stratégie inconnu");
        }
        let next = list(state, org, project, key).len() as i64 + 1;
        let item = dress_policy(
            org,
            project,
            json!({
                "id": next,
                "revision": 1,
                "isEnabled": payload.get("isEnabled").cloned().unwrap_or(json!(true)),
                "isBlocking": payload.get("isBlocking").cloned().unwrap_or(json!(true)),
                "isDeleted": false,
                "type": {"id": type_id},
                "settings": payload.get("settings").cloned().unwrap_or(json!({})),
                "createdDate": "2026-10-29T00:00:00Z",
                "createdBy": {"displayName": "azurz"}
            }),
        );
        put_item(state, org, project, key, item.clone());
        return Json(item).into_response();
    }
    let id = rest.get(2).map(String::as_str).unwrap_or("");
    if method == Method::GET {
        return match get_item(state, org, project, key, id) {
            Some(item) => Json(dress_policy(org, project, item)).into_response(),
            None => fail(StatusCode::NOT_FOUND, "stratégie introuvable"),
        };
    }
    if method == Method::DELETE {
        return if delete_item(state, org, project, key, id) {
            StatusCode::NO_CONTENT.into_response()
        } else {
            fail(StatusCode::NOT_FOUND, "stratégie introuvable")
        };
    }
    if method == Method::PUT || method == Method::PATCH {
        let payload = parse_json(body).unwrap_or(json!({}));
        return match get_item(state, org, project, key, id) {
            Some(mut item) => {
                if let Some(object) = item.as_object_mut() {
                    if let Some(value) = payload.get("isEnabled") {
                        object.insert("isEnabled".into(), value.clone());
                    }
                    if let Some(value) = payload.get("isBlocking") {
                        object.insert("isBlocking".into(), value.clone());
                    }
                    if let Some(value) = payload.get("settings") {
                        object.insert("settings".into(), value.clone());
                    }
                    if let Some(type_id) = text_of(&payload["type"], "id") {
                        object.insert("type".into(), json!({"id": type_id}));
                    }
                    let revision = object.get("revision").and_then(|value| value.as_i64()).unwrap_or(1) + 1;
                    object.insert("revision".into(), json!(revision));
                }
                let dressed = dress_policy(org, project, item);
                put_item(state, org, project, key, dressed.clone());
                Json(dressed).into_response()
            }
            None => fail(StatusCode::NOT_FOUND, "stratégie introuvable"),
        };
    }
    fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée")
}

fn policy_evaluations(state: &AppState, method: &Method, org: &str, project: &str, query: &str, body: &[u8]) -> Response {
    if method != Method::GET && method != Method::POST {
        return fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée");
    }
    let payload = parse_json(body).unwrap_or(json!({}));
    let artifact = if method == Method::POST {
        text_of(&payload, "artifactId").unwrap_or_default()
    } else {
        query_param(query, "artifactId").map(crate::http::pct_decode).unwrap_or_default()
    };
    if artifact.is_empty() {
        return fail(StatusCode::BAD_REQUEST, "artefact absent");
    }
    let Some(pull_id) = pull_id_from_artifact(&artifact) else {
        return fail(StatusCode::BAD_REQUEST, "artefact inconnu");
    };
    let Some(pull) = get_item(state, org, project, "git/pullrequests", &pull_id) else {
        return fail(StatusCode::NOT_FOUND, "demande de tirage introuvable");
    };
    let repo_id = text_of(&pull["repository"], "id").unwrap_or_default();
    let target = text_of(&pull, "targetRefName").unwrap_or_else(|| "refs/heads/main".into());
    let value = list(state, org, project, "policy/configurations")
        .into_iter()
        .map(|item| dress_policy(org, project, item))
        .filter(|item| scope_matches(item.get("settings").unwrap_or(&Value::Null), &repo_id, &target))
        .map(|item| {
            let configuration_id = value_key(&item["id"]);
            let status = evaluate_policy(state, org, project, &item, &pull);
            json!({
                "evaluationId": format!("{configuration_id}-{pull_id}"),
                "artifactId": artifact,
                "configuration": item,
                "status": status,
                "startedDate": "2026-10-29T00:00:00Z",
                "completedDate": "2026-10-29T00:00:01Z"
            })
        })
        .collect();
    Json(page(value)).into_response()
}

fn pull_id_from_artifact(artifact: &str) -> Option<String> {
    let tail = artifact.rsplit('/').next().unwrap_or("").trim();
    if tail.is_empty() || tail.eq_ignore_ascii_case("CodeReviewId") {
        return None;
    }
    Some(tail.to_string())
}

fn scope_matches(settings: &Value, repo_id: &str, target_ref: &str) -> bool {
    let Some(scopes) = settings.get("scope").and_then(|value| value.as_array()) else {
        return true;
    };
    if scopes.is_empty() {
        return true;
    }
    scopes.iter().any(|scope| {
        let repository = text_of(scope, "repositoryId").unwrap_or_default();
        if !repository.is_empty() && repository != repo_id {
            return false;
        }
        let ref_name = text_of(scope, "refName").unwrap_or_default();
        if ref_name.is_empty() {
            return true;
        }
        let kind = text_of(scope, "matchKind").unwrap_or_else(|| "Exact".into());
        if kind.eq_ignore_ascii_case("Prefix") {
            target_ref.starts_with(&ref_name)
        } else {
            target_ref == ref_name
        }
    })
}

fn evaluate_policy(state: &AppState, org: &str, project: &str, configuration: &Value, pull: &Value) -> String {
    if configuration.get("isEnabled").and_then(|value| value.as_bool()) == Some(false) {
        return "notApplicable".into();
    }
    let type_id = text_of(&configuration["type"], "id").unwrap_or_default();
    let settings = configuration.get("settings").cloned().unwrap_or(json!({}));
    let pull_id = value_key(&pull["pullRequestId"]);
    let pull_key = if pull_id.is_empty() { value_key(&pull["id"]) } else { pull_id };
    match type_id.as_str() {
        "fa4e907d-c16b-4a4c-9dfa-4906e5d171dd" => {
            let needed = settings.get("minimumApproverCount").and_then(|value| value.as_i64()).unwrap_or(1);
            let approvers = list(state, org, project, &format!("git/pullrequests/{pull_key}/reviewers"))
                .into_iter()
                .filter(|reviewer| reviewer.get("vote").and_then(|value| value.as_i64()).unwrap_or(0) >= 10)
                .count() as i64;
            if approvers >= needed { "approved" } else { "rejected" }
        }
        "0609b952-1397-4640-95ec-e00a01b2c241" => build_policy_status(state, org, project, pull, &settings),
        "cbdc66da-9728-4af8-aada-9a5a32e4a226" => status_policy_status(state, org, project, &pull_key, &settings),
        "c6a1889d-b943-4856-b76f-9e46bb6b0df2" => comment_policy_status(state, org, project, &pull_key),
        _ => "queued",
    }
    .into()
}

fn build_policy_status(state: &AppState, org: &str, project: &str, pull: &Value, settings: &Value) -> &'static str {
    let source = text_of(pull, "sourceRefName").unwrap_or_default();
    let definition = settings.get("buildDefinitionId").and_then(|value| value.as_i64());
    let mut builds: Vec<Value> = list(state, org, project, "build/builds")
        .into_iter()
        .filter(|build| {
            text_of(build, "sourceBranch").as_deref() == Some(source.as_str())
                && definition
                    .map(|id| build.get("definition").and_then(|item| item.get("id")).and_then(|value| value.as_i64()) == Some(id))
                    .unwrap_or(true)
        })
        .collect();
    builds.sort_by_key(|build| build.get("id").and_then(|value| value.as_i64()).unwrap_or(0));
    let Some(build) = builds.last() else {
        return "rejected";
    };
    match text_of(build, "status").as_deref() {
        Some("completed") => {
            if text_of(build, "result").as_deref() == Some("succeeded") {
                "approved"
            } else {
                "rejected"
            }
        }
        _ => "queued",
    }
}

fn status_policy_status(state: &AppState, org: &str, project: &str, pull_id: &str, settings: &Value) -> &'static str {
    let name = text_of(settings, "statusName").or_else(|| text_of(settings, "defaultDisplayName"));
    let genre = text_of(settings, "statusGenre");
    let statuses = list(state, org, project, &format!("git/pullrequests/{pull_id}/statuses"));
    let latest = statuses.into_iter().filter(|item| {
        let context = &item["context"];
        name.as_ref().map(|wanted| text_of(context, "name").as_deref() == Some(wanted.as_str())).unwrap_or(true)
            && genre.as_ref().map(|wanted| text_of(context, "genre").as_deref() == Some(wanted.as_str())).unwrap_or(true)
    }).max_by_key(|item| item.get("id").and_then(|value| value.as_i64()).unwrap_or(0));
    match latest.as_ref().and_then(|item| text_of(item, "state")).as_deref() {
        Some("succeeded") | Some("notApplicable") => "approved",
        Some("pending") | Some("notSet") => "queued",
        Some(_) => "rejected",
        None => "rejected",
    }
}

fn comment_policy_status(state: &AppState, org: &str, project: &str, pull_id: &str) -> &'static str {
    let threads = list(state, org, project, &format!("git/pullrequests/{pull_id}/threads"));
    if threads.is_empty() || threads.iter().all(|thread| text_of(thread, "status").as_deref() != Some("active")) {
        "approved"
    } else {
        "rejected"
    }
}

fn dress_policy(org: &str, project: &str, mut item: Value) -> Value {
    let id = value_key(&item["id"]);
    let type_id = text_of(&item["type"], "id").unwrap_or_default();
    let display = policy_type_catalog()
        .into_iter()
        .find(|kind| text_of(kind, "id").as_deref() == Some(type_id.as_str()))
        .and_then(|kind| text_of(&kind, "displayName"))
        .unwrap_or_else(|| type_id.clone());
    if let Some(object) = item.as_object_mut() {
        object.entry("isEnabled").or_insert(json!(true));
        object.entry("isBlocking").or_insert(json!(true));
        object.entry("isDeleted").or_insert(json!(false));
        object.entry("revision").or_insert(json!(1));
        object.insert("url".into(), json!(format!("/{org}/{project}/_apis/policy/configurations/{id}")));
        object.insert("type".into(), json!({"id": type_id, "displayName": display}));
    }
    item
}

fn secure_files(
    state: &AppState,
    method: &Method,
    org: &str,
    project: &str,
    rest: &[String],
    query: &str,
    body: &[u8],
) -> Response {
    let key = "distributedtask/securefiles";
    if rest.len() == 2 && method == Method::GET {
        let value = list(state, org, project, key).iter().map(dress_secure_file).collect();
        return Json(page(value)).into_response();
    }
    if rest.len() == 2 && method == Method::POST {
        let (name, bytes) = match secure_upload(query, body) {
            Ok(upload) => upload,
            Err(message) => return fail(StatusCode::BAD_REQUEST, message),
        };
        let duplicate = list(state, org, project, key)
            .iter()
            .any(|item| text_of(item, "name").is_some_and(|stored| stored.eq_ignore_ascii_case(&name)));
        if duplicate {
            return fail(StatusCode::BAD_REQUEST, "fichier déjà présent");
        }
        let next = list(state, org, project, key)
            .into_iter()
            .filter_map(|item| item.get("id").and_then(|value| value.as_i64()))
            .max()
            .unwrap_or(0)
            + 1;
        let item = json!({
            "id": next,
            "name": name,
            "content": base64::engine::general_purpose::STANDARD.encode(&bytes),
            "url": format!("/{org}/{project}/_apis/distributedtask/securefiles/{next}")
        });
        put_item(state, org, project, key, item.clone());
        return Json(dress_secure_file(&item)).into_response();
    }
    let id = rest.get(2).map(String::as_str).unwrap_or("");
    if id.is_empty() {
        return fail(StatusCode::NOT_FOUND, "fichier sécurisé introuvable");
    }
    let Some(item) = get_item(state, org, project, key, id) else {
        return fail(StatusCode::NOT_FOUND, "fichier sécurisé introuvable");
    };
    let download = rest.get(3).is_some_and(|part| part.eq_ignore_ascii_case("download"))
        || query_param(query, "download").is_some_and(|value| pct_decode(value).eq_ignore_ascii_case("true"));
    if method == Method::GET && download {
        let encoded = text_of(&item, "content").unwrap_or_default();
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(encoded.as_bytes())
            .unwrap_or_default();
        return Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, "application/octet-stream")
            .body(axum::body::Body::from(bytes))
            .unwrap_or_else(|_| fail(StatusCode::INTERNAL_SERVER_ERROR, "réponse impossible"));
    }
    if method == Method::GET {
        return Json(dress_secure_file(&item)).into_response();
    }
    if method == Method::DELETE {
        return if delete_item(state, org, project, key, id) {
            StatusCode::NO_CONTENT.into_response()
        } else {
            fail(StatusCode::NOT_FOUND, "fichier sécurisé introuvable")
        };
    }
    fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée")
}

fn secure_upload(query: &str, body: &[u8]) -> Result<(String, Vec<u8>), &'static str> {
    if let Some(raw) = query_param(query, "name") {
        let name = pct_decode(raw).trim().to_string();
        if name.is_empty() {
            return Err("nom de fichier absent");
        }
        return Ok((name, body.to_vec()));
    }
    let payload = parse_json(body).unwrap_or(Value::Null);
    let name = text_of(&payload, "name").unwrap_or_default().trim().to_string();
    if name.is_empty() {
        return Err("nom de fichier absent");
    }
    Ok((name, text_of(&payload, "content").unwrap_or_default().into_bytes()))
}

fn dress_secure_file(item: &Value) -> Value {
    let id = value_key(&item["id"]);
    json!({
        "id": item["id"],
        "name": item["name"],
        "createdBy": {"displayName": "azurz", "uniqueName": "azurz"},
        "createdOn": "2026-10-29T00:00:00Z",
        "modifiedBy": {"displayName": "azurz", "uniqueName": "azurz"},
        "modifiedOn": "2026-10-29T00:00:00Z",
        "url": item.get("url").cloned().unwrap_or(json!(format!("_apis/distributedtask/securefiles/{id}")))
    })
}

fn task_agents(
    state: &AppState,
    method: &Method,
    org: &str,
    project: Option<&str>,
    rest: &[String],
    query: &str,
    body: &[u8],
) -> Response {
    let resource = rest.get(1).map(|item| item.to_ascii_lowercase()).unwrap_or_default();
    if resource == "queues" {
        if method != Method::GET {
            return fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée");
        }
        let Some(project) = project else {
            return fail(StatusCode::NOT_FOUND, "file introuvable");
        };
        return Json(page(list(state, org, project, "distributedtask/queues"))).into_response();
    }
    if resource != "pools" {
        return fail(StatusCode::NOT_FOUND, "ressource introuvable");
    }
    if rest.len() <= 2 {
        if method != Method::GET {
            return fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée");
        }
        let value = org_list(state, org, "distributedtask/pools")
            .into_iter()
            .map(|item| dress_pool(state, org, item))
            .collect();
        return Json(page(value)).into_response();
    }
    let pool_id = &rest[2];
    let Some(pool) = org_get(state, org, "distributedtask/pools", pool_id) else {
        return fail(StatusCode::NOT_FOUND, "pool introuvable");
    };
    let pool_key = value_key(&pool["id"]);
    if rest.len() == 3 && method == Method::GET {
        return Json(dress_pool(state, org, pool)).into_response();
    }
    match rest.get(3).map(|value| value.to_ascii_lowercase()).as_deref() {
        Some("agents") => pool_agents(state, method, org, &pool_key, rest, body),
        Some("jobrequests") => pool_jobs(state, method, org, &pool_key, rest, query, body),
        _ => fail(StatusCode::NOT_FOUND, "ressource introuvable"),
    }
}

fn dress_pool(state: &AppState, org: &str, mut pool: Value) -> Value {
    let id = value_key(&pool["id"]);
    let size = org_list(state, org, &format!("distributedtask/pools/{id}/agents")).len();
    if let Some(object) = pool.as_object_mut() {
        object.insert("size".into(), json!(size));
    }
    pool
}

fn pool_agents(state: &AppState, method: &Method, org: &str, pool_id: &str, rest: &[String], body: &[u8]) -> Response {
    let key = format!("distributedtask/pools/{pool_id}/agents");
    if rest.len() == 4 && method == Method::GET {
        return Json(page(org_list(state, org, &key))).into_response();
    }
    if rest.len() == 4 && method == Method::POST {
        let payload = parse_json(body).unwrap_or(json!({}));
        let name = text_of(&payload, "name").unwrap_or_else(|| "agent".into());
        let next = org_list(state, org, &key).len() as i64 + 1;
        let item = json!({
            "id": next,
            "name": name,
            "version": text_of(&payload, "version").unwrap_or_else(|| "3.248.0".into()),
            "osDescription": text_of(&payload, "osDescription").unwrap_or_else(|| "Linux".into()),
            "enabled": payload.get("enabled").and_then(|value| value.as_bool()).unwrap_or(true),
            "status": text_of(&payload, "status").unwrap_or_else(|| "offline".into()),
            "provisioningState": "Provisioned",
            "systemCapabilities": payload.get("systemCapabilities").cloned().unwrap_or(json!({})),
            "poolId": pool_id.parse::<i64>().unwrap_or(1),
            "createdOn": "2026-10-29T00:00:00Z",
            "url": format!("/{org}/_apis/distributedtask/pools/{pool_id}/agents/{next}")
        });
        org_put(state, org, &key, item.clone());
        return Json(item).into_response();
    }
    if rest.len() >= 5 {
        let agent_id = &rest[4];
        let Some(current) = org_get(state, org, &key, agent_id) else {
            return fail(StatusCode::NOT_FOUND, "agent introuvable");
        };
        if method == Method::GET {
            return Json(current).into_response();
        }
        if method == Method::PATCH || method == Method::PUT {
            let payload = parse_json(body).unwrap_or(json!({}));
            let mut item = current;
            if let Some(object) = item.as_object_mut() {
                for field in ["name", "version", "osDescription", "status", "enabled", "systemCapabilities", "provisioningState"] {
                    if let Some(value) = payload.get(field) {
                        object.insert(field.to_string(), value.clone());
                    }
                }
            }
            org_put(state, org, &key, item.clone());
            return Json(item).into_response();
        }
        return fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée");
    }
    fail(StatusCode::NOT_FOUND, "agent introuvable")
}

fn pool_jobs(
    state: &AppState,
    method: &Method,
    org: &str,
    pool_id: &str,
    rest: &[String],
    query: &str,
    body: &[u8],
) -> Response {
    let key = format!("distributedtask/pools/{pool_id}/jobrequests");
    if rest.len() == 4 && method == Method::GET {
        let agent_id = query_param(query, "agentId").map(|value| crate::http::pct_decode(&value));
        let value = org_list(state, org, &key)
            .into_iter()
            .filter(|item| agent_id.as_ref().is_none_or(|wanted| request_agent_id(item) == *wanted))
            .collect();
        return Json(page(value)).into_response();
    }
    if rest.len() == 4 && method == Method::POST {
        let payload = parse_json(body).unwrap_or(json!({}));
        let agent_id = payload
            .get("agentId")
            .map(value_key)
            .filter(|id| !id.is_empty())
            .or_else(|| payload.get("reservedAgent").map(|agent| value_key(&agent["id"])));
        let agent = agent_id
            .as_ref()
            .and_then(|id| org_get(state, org, &format!("distributedtask/pools/{pool_id}/agents"), id));
        if agent_id.is_some() && agent.is_none() {
            return fail(StatusCode::NOT_FOUND, "agent introuvable");
        }
        let next = org_list(state, org, &key).len() as i64 + 1;
        let agent_name = agent.as_ref().and_then(|item| text_of(item, "name")).unwrap_or_default();
        let definition = payload.get("definition").cloned().unwrap_or(json!({"id": 1, "name": "CI"}));
        let item = json!({
            "requestId": next,
            "id": next,
            "poolId": pool_id.parse::<i64>().unwrap_or(1),
            "agentId": agent_id.as_ref().and_then(|id| id.parse::<i64>().ok()),
            "reservedAgent": agent.as_ref().map(|item| json!({"id": item["id"], "name": agent_name})).unwrap_or(Value::Null),
            "planType": text_of(&payload, "planType").unwrap_or_else(|| "Build".into()),
            "definition": definition,
            "demands": payload.get("demands").cloned().unwrap_or(json!([])),
            "status": "queued",
            "result": Value::Null,
            "queueTime": "2026-10-29T00:00:00Z",
            "url": format!("/{org}/_apis/distributedtask/pools/{pool_id}/jobrequests/{next}")
        });
        org_put(state, org, &key, item.clone());
        return Json(item).into_response();
    }
    if rest.len() >= 5 {
        let request_id = &rest[4];
        let Some(current) = org_get(state, org, &key, request_id) else {
            return fail(StatusCode::NOT_FOUND, "demande introuvable");
        };
        if method == Method::GET {
            return Json(current).into_response();
        }
        if method == Method::PATCH || method == Method::PUT {
            let payload = parse_json(body).unwrap_or(json!({}));
            let mut item = current;
            if let Some(object) = item.as_object_mut() {
                if let Some(result) = text_of(&payload, "result") {
                    object.insert("result".into(), json!(result));
                    object.insert("status".into(), json!("completed"));
                    object.insert("finishTime".into(), json!("2026-10-29T00:05:00Z"));
                }
                if let Some(status) = text_of(&payload, "status") {
                    object.insert("status".into(), json!(status));
                }
            }
            org_put(state, org, &key, item.clone());
            return Json(item).into_response();
        }
        return fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée");
    }
    fail(StatusCode::NOT_FOUND, "demande introuvable")
}

fn request_agent_id(item: &Value) -> String {
    item.get("agentId")
        .map(value_key)
        .filter(|id| !id.is_empty())
        .or_else(|| item.get("reservedAgent").map(|agent| value_key(&agent["id"])))
        .unwrap_or_default()
}

fn build_timeline(state: &AppState, method: &Method, org: &str, project: &str, id: &str, body: &[u8]) -> Response {
    if get_item(state, org, project, "build/builds", id).is_none() {
        return fail(StatusCode::NOT_FOUND, "build introuvable");
    }
    let key = format!("build/builds/{id}/timeline");
    if method == Method::PUT || method == Method::PATCH {
        let payload = parse_json(body).unwrap_or(json!({}));
        let incoming = payload.get("records").and_then(|value| value.as_array()).cloned().unwrap_or_default();
        let mut records = stored_or_synthesized_timeline(state, org, project, id);
        for record in incoming {
            let record_id = text_of(&record, "id").unwrap_or_else(|| format!("record-{}", records.len() + 1));
            if let Some(index) = records.iter().position(|item| text_of(item, "id").as_deref() == Some(record_id.as_str())) {
                records[index] = record;
            } else {
                records.push(record);
            }
        }
        let change_id = get_item(state, org, project, &key, "current")
            .and_then(|item| item.get("changeId").and_then(|value| value.as_i64()))
            .unwrap_or(0)
            + 1;
        let stored = json!({
            "id": "current",
            "changeId": change_id,
            "url": format!("/{org}/{project}/_apis/build/builds/{id}/timeline"),
            "records": records
        });
        put_item(state, org, project, &key, stored);
        return Json(timeline_document(org, project, id, change_id, records_of(state, org, project, id))).into_response();
    }
    if method == Method::GET {
        let stored = get_item(state, org, project, &key, "current");
        let change_id = stored.as_ref().and_then(|item| item.get("changeId").and_then(|value| value.as_i64())).unwrap_or(1);
        let records = stored_or_synthesized_timeline(state, org, project, id);
        return Json(timeline_document(org, project, id, change_id, records)).into_response();
    }
    fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée")
}

fn records_of(state: &AppState, org: &str, project: &str, id: &str) -> Vec<Value> {
    let key = format!("build/builds/{id}/timeline");
    get_item(state, org, project, &key, "current")
        .and_then(|item| item.get("records").and_then(|value| value.as_array()).cloned())
        .unwrap_or_default()
}

fn stored_or_synthesized_timeline(state: &AppState, org: &str, project: &str, id: &str) -> Vec<Value> {
    let stored = records_of(state, org, project, id);
    if stored.is_empty() {
        synthesized_timeline(state, org, project, id)
    } else {
        stored
    }
}

fn synthesized_timeline(state: &AppState, org: &str, project: &str, id: &str) -> Vec<Value> {
    let build = get_item(state, org, project, "build/builds", id).unwrap_or(json!({}));
    let status = text_of(&build, "status").unwrap_or_else(|| "notStarted".into());
    let done = status == "completed";
    let state_name = if done {
        "completed"
    } else if status == "notStarted" {
        "pending"
    } else {
        "inProgress"
    };
    let result = if done {
        json!(text_of(&build, "result").unwrap_or_else(|| "succeeded".into()))
    } else {
        Value::Null
    };
    let stage_name = build
        .get("definition")
        .and_then(|item| text_of(item, "name"))
        .unwrap_or_else(|| "CI".into());
    let log = if done {
        json!({"id": 1, "type": "Container", "url": format!("/{org}/{project}/_apis/build/builds/{id}/logs/1")})
    } else {
        Value::Null
    };
    vec![
        json!({"id": "stage-1", "parentId": null, "type": "Stage", "name": stage_name, "state": state_name, "result": result, "order": 1}),
        json!({"id": "phase-1", "parentId": "stage-1", "type": "Phase", "name": "Phase 1", "state": state_name, "result": result, "order": 1}),
        json!({"id": "job-1", "parentId": "phase-1", "type": "Job", "name": "Job", "state": state_name, "result": result, "order": 1, "workerName": "azdo-linux"}),
        json!({"id": "task-build", "parentId": "job-1", "type": "Task", "name": "Build", "state": state_name, "result": result, "order": 1, "log": log}),
    ]
}

fn timeline_document(org: &str, project: &str, id: &str, change_id: i64, records: Vec<Value>) -> Value {
    json!({
        "id": id,
        "changeId": change_id,
        "url": format!("/{org}/{project}/_apis/build/builds/{id}/timeline"),
        "records": records
    })
}

fn promote_build(state: &AppState, org: &str, project: &str, mut item: Value) -> Value {
    if text_of(&item, "status").as_deref() == Some("notStarted") {
        item["status"] = json!("completed");
        item["result"] = json!("succeeded");
        item["startTime"] = json!("2026-10-29T00:00:01Z");
        item["finishTime"] = json!("2026-10-29T00:00:02Z");
        put_item(state, org, project, "build/builds", item.clone());
        deliver_work_item(state, org, project, "build.complete", &item);
    }
    item
}

fn promote_pipeline(state: &AppState, org: &str, project: &str, mut item: Value) -> Value {
    if text_of(&item, "state").as_deref() == Some("inProgress") {
        let run_id = value_key(&item["id"]);
        match approval_outcome(state, org, project, &run_id) {
            ApprovalOutcome::Waiting => return item,
            ApprovalOutcome::Rejected => {
                item["state"] = json!("completed");
                item["result"] = json!("failed");
            }
            ApprovalOutcome::Clear => {
                item["state"] = json!("completed");
                item["result"] = json!("succeeded");
            }
        }
        item["finishedDate"] = json!("2026-10-29T00:00:02Z");
        put_item(state, org, project, "pipelines/runs", item.clone());
    }
    item
}

fn page(value: Vec<Value>) -> Value {
    json!({"count": value.len(), "value": value})
}

fn text_of(value: &Value, key: &str) -> Option<String> {
    value.get(key).and_then(|item| item.as_str()).map(str::to_string)
}

fn list(state: &AppState, org: &str, project: &str, key: &str) -> Vec<Value> {
    state.read(|db| {
        db.project(org, project)
            .and_then(|stored| stored.buckets.get(key))
            .map(|bucket| bucket.items.values().cloned().collect())
            .unwrap_or_default()
    })
}

fn get_item(state: &AppState, org: &str, project: &str, key: &str, id: &str) -> Option<Value> {
    state.read(|db| {
        db.project(org, project)
            .and_then(|stored| stored.buckets.get(key))
            .and_then(|bucket| bucket.get(id))
    })
}

fn put_item(state: &AppState, org: &str, project: &str, key: &str, item: Value) {
    state.write(|db| {
        db.ensure_project(org, project);
        if let Some(stored) = db.project_mut(org, project) {
            let bucket = stored.buckets.entry(key.to_string()).or_insert_with(Bucket::empty);
            bucket.put(item);
        }
    });
}

fn delete_item(state: &AppState, org: &str, project: &str, key: &str, id: &str) -> bool {
    state.write(|db| {
        db.project_mut(org, project)
            .and_then(|stored| stored.buckets.get_mut(key))
            .is_some_and(|bucket| bucket.remove(id))
    })
}

fn patch_item(state: &AppState, org: &str, project: &str, key: &str, id: &str, body: &Value) -> Option<Value> {
    state.write(|db| {
        let stored = db.project_mut(org, project)?;
        let bucket = stored.buckets.get_mut(key)?;
        bucket.patch(id, body)
    })
}

fn find_repo(state: &AppState, org: &str, project: &str, id: &str) -> Option<Value> {
    state.read(|db| {
        db.project(org, project)
            .and_then(|stored| stored.buckets.get("git/repositories"))
            .and_then(|bucket| bucket.get(id))
    })
}

fn default_repo_id(state: &AppState, org: &str, project: &str) -> Option<String> {
    let repos = list(state, org, project, "git/repositories");
    repos
        .iter()
        .find(|item| text_of(item, "name").as_deref() == Some(project))
        .or_else(|| repos.first())
        .map(|item| value_key(&item["id"]))
        .filter(|id| !id.is_empty())
}

fn repo_name(state: &AppState, org: &str, project: &str, repo_id: &str) -> String {
    find_repo(state, org, project, repo_id)
        .and_then(|item| text_of(&item, "name"))
        .unwrap_or_else(|| project.to_string())
}

fn resolve_wiki(state: &AppState, org: &str, project: &str, id: &str) -> Option<Value> {
    let wikis = list(state, org, project, "wiki/wikis");
    wikis.into_iter().find(|item| {
        value_key(&item["id"]) == id || text_of(item, "name").as_deref() == Some(id)
    })
}

fn org_list(state: &AppState, org: &str, key: &str) -> Vec<Value> {
    state.read(|db| {
        db.orgs
            .get(org)
            .and_then(|organization| organization.buckets.get(key))
            .map(|bucket| bucket.items.values().cloned().collect())
            .unwrap_or_default()
    })
}

fn org_get(state: &AppState, org: &str, key: &str, id: &str) -> Option<Value> {
    state.read(|db| {
        db.orgs
            .get(org)
            .and_then(|organization| organization.buckets.get(key))
            .and_then(|bucket| bucket.get(id))
    })
}

fn org_put(state: &AppState, org: &str, key: &str, item: Value) {
    state.write(|db| {
        db.ensure_org(org);
        if let Some(organization) = db.orgs.get_mut(org) {
            let bucket = organization.buckets.entry(key.to_string()).or_insert_with(Bucket::empty);
            bucket.put(item);
        }
    });
}
