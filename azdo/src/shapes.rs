use axum::http::{header, Method, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::{json, Map, Value};

use crate::http::{fail, parse_json, query_param};
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
        ("git", "refs") => git_alias(state, method, org, project, "refs", body),
        ("git", "commits") => git_alias(state, method, org, project, "commits", body),
        ("git", "pushes") => git_alias(state, method, org, project, "pushes", body),
        ("git", "pullrequests") => pull_requests(state, method, org, project, None, rest, body),
        ("build", "definitions") => definitions(state, method, org, project, body),
        ("build", "builds") => builds(state, method, org, project, rest, body),
        ("distributedtask", "queues") | ("distributedtask", "pools") => {
            task_agents(state, method, org, Some(project), rest)
        }
        ("pipelines", "pipelines") => pipelines(state, method, org, project, rest, body),
        ("pipelines", "runs") => pipeline_runs(state, method, org, project, rest),
        ("testplan", "plans") | ("test", "plans") => test_plans(state, method, org, project, rest, body),
        ("test", "runs") => test_runs(state, method, org, project, rest, body),
        ("wiki", "wikis") => wikis(state, method, org, project, rest, query, body),
        ("packaging", "feeds") => feeds(state, method, org, project, rest, body),
        ("release", "definitions") => release_definitions(state, method, org, project, rest, body),
        ("release", "releases") => release_releases(state, method, org, project, rest, body),
        ("release", "approvals") => release_approvals(state, method, org, project, rest, body),
        _ => return None,
    };
    Some(response)
}

pub fn org_dispatch(state: &AppState, method: &Method, org: &str, rest: &[String], body: &[u8]) -> Option<Response> {
    let area = rest.first()?.to_ascii_lowercase();
    if area == "securitynamespaces" && method == Method::GET {
        return Some(Json(security_namespaces(org)).into_response());
    }
    let resource = rest.get(1)?.to_ascii_lowercase();
    let response = match (area.as_str(), resource.as_str()) {
        ("hooks", "subscriptions") => subscriptions(state, method, org, rest, body),
        ("distributedtask", "pools") | ("distributedtask", "queues") => task_agents(state, method, org, None, rest),
        ("graph", "users") => graph(state, method, org, "graph/users", "azurz"),
        ("graph", "groups") => graph(state, method, org, "graph/groups", format!("{org} Team")),
        _ => return None,
    };
    Some(response)
}

pub fn wit_catalog(method: &Method, org: &str, project: &str, rest: &[String]) -> Option<Response> {
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
    if resource == "classificationnodes" {
        return Some(Json(classification(org, project, rest)).into_response());
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
        "refs" => refs(state, method, org, project, repo_id, body),
        "commits" => commits(state, method, org, project, repo_id, tail_after(rest, "commits"), body),
        "pushes" => pushes(state, method, org, project, repo_id, tail_after(rest, "pushes"), body),
        "pullrequests" => pull_requests(state, method, org, project, Some(repo_id), rest, body),
        "items" => items(state, org, project, repo_id, query),
        _ => fail(StatusCode::NOT_FOUND, "ressource git introuvable"),
    }
}

fn git_alias(state: &AppState, method: &Method, org: &str, project: &str, kind: &str, body: &[u8]) -> Response {
    let Some(repo_id) = default_repo_id(state, org, project) else {
        return fail(StatusCode::NOT_FOUND, "dépôt introuvable");
    };
    match kind {
        "refs" => refs(state, method, org, project, &repo_id, body),
        "commits" => commits(state, method, org, project, &repo_id, &[], body),
        "pushes" => pushes(state, method, org, project, &repo_id, &[], body),
        _ => fail(StatusCode::NOT_FOUND, "ressource git introuvable"),
    }
}

fn refs(state: &AppState, method: &Method, org: &str, project: &str, repo_id: &str, body: &[u8]) -> Response {
    let key = format!("git/repositories/{repo_id}/refs");
    if method == Method::GET {
        return Json(page(list(state, org, project, &key))).into_response();
    }
    if method != Method::POST {
        return fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée");
    }
    let payload = parse_json(body).unwrap_or(json!([]));
    let updates = payload.as_array().cloned().unwrap_or_else(|| vec![payload]);
    let created = state.write(|db| {
        db.ensure_project(org, project);
        let stored = db.project_mut(org, project)?;
        let bucket = stored.buckets.entry(key.clone()).or_insert_with(Bucket::empty);
        let mut rows = Vec::new();
        for update in updates {
            let name = text_of(&update, "name").unwrap_or_else(|| "refs/heads/main".into());
            let object = text_of(&update, "newObjectId")
                .filter(|value| !value.is_empty() && !value.chars().all(|ch| ch == '0'))
                .unwrap_or_else(|| object_id(&format!("{repo_id}:{name}")));
            let item = json!({
                "id": name,
                "name": name,
                "objectId": object,
                "oldObjectId": update.get("oldObjectId").cloned().unwrap_or(json!("0000000000000000000000000000000000000000")),
                "success": true
            });
            bucket.put(item.clone());
            rows.push(item);
        }
        Some(rows)
    });
    match created {
        Some(value) => Json(page(value)).into_response(),
        None => fail(StatusCode::NOT_FOUND, "dépôt introuvable"),
    }
}

fn commits(state: &AppState, method: &Method, org: &str, project: &str, repo_id: &str, tail: &[String], body: &[u8]) -> Response {
    let key = format!("git/repositories/{repo_id}/commits");
    if let Some(commit_id) = tail.first() {
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
        return Json(item).into_response();
    }
    match apply_push(state, org, project, repo_id, &payload) {
        PushOutcome::Created(item) => Json(item).into_response(),
        PushOutcome::Conflict => fail(StatusCode::CONFLICT, "la référence a changé"),
        PushOutcome::Missing => fail(StatusCode::NOT_FOUND, "dépôt introuvable"),
    }
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
                let change_type = text_of(&change, "changeType").unwrap_or_else(|| "add".into());
                if change_type.eq_ignore_ascii_case("delete") {
                    if let Some(bucket) = stored.buckets.get_mut(&items_key) {
                        bucket.remove(&path);
                    }
                } else {
                    ensure_tree(stored, &items_key, &path);
                    let content = change
                        .get("newContent")
                        .and_then(|value| text_of(value, "content").or_else(|| value.as_str().map(str::to_string)))
                        .unwrap_or_default();
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
                    "item": {"path": path, "gitObjectType": "blob"}
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

fn items(state: &AppState, org: &str, project: &str, repo_id: &str, query: &str) -> Response {
    let key = format!("git/repositories/{repo_id}/items");
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
    if let Some((kind, pull_id)) = pull_child(rest) {
        return pull_child_resource(state, method, org, project, &kind, &pull_id, body);
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
            return match patch_item(state, org, project, key, &id, &payload) {
                Some(item) => {
                    if text_of(&item, "status").as_deref() == Some("completed") {
                        Json(finish_pull(state, org, project, &id).unwrap_or(item)).into_response()
                    } else {
                        Json(item).into_response()
                    }
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
    Json(item).into_response()
}

fn pull_child(rest: &[String]) -> Option<(String, String)> {
    let index = rest.iter().position(|part| {
        matches!(part.to_ascii_lowercase().as_str(), "threads" | "statuses" | "iterations" | "reviewers")
    })?;
    let id = rest.get(index.checked_sub(1)?)?.clone();
    if id.eq_ignore_ascii_case("pullrequests") || id.eq_ignore_ascii_case("repositories") {
        return None;
    }
    Some((rest[index].to_ascii_lowercase(), id))
}

fn pull_child_resource(
    state: &AppState,
    method: &Method,
    org: &str,
    project: &str,
    kind: &str,
    pull_id: &str,
    body: &[u8],
) -> Response {
    let key = format!("git/pullrequests/{pull_id}/{kind}");
    if kind == "reviewers" {
        return reviewers(state, method, org, project, pull_id, body);
    }
    if kind == "iterations" && method == Method::GET {
        let pull = get_item(state, org, project, "git/pullrequests", pull_id);
        let source = pull.as_ref().and_then(|item| text_of(item, "sourceRefName")).unwrap_or_else(|| "refs/heads/feature".into());
        let target = pull.as_ref().and_then(|item| text_of(item, "targetRefName")).unwrap_or_else(|| "refs/heads/main".into());
        return Json(page(vec![json!({
            "id": 1,
            "sourceRefName": source,
            "targetRefName": target,
            "description": "Iteration 1"
        })]))
        .into_response();
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
    if stored.len() == 1 {
        Json(stored.pop().unwrap_or(json!({}))).into_response()
    } else {
        Json(Value::Array(stored)).into_response()
    }
}

fn finish_pull(state: &AppState, org: &str, project: &str, pull_id: &str) -> Option<Value> {
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
        let commit_id = object_id(&format!("merge:{pull_id}:{}", parent.clone().unwrap_or_default()));
        let parents = parent.clone().map(|id| vec![json!(id)]).unwrap_or_default();
        stored.put_bucket(
            &commits_key,
            json!({
                "id": commit_id,
                "commitId": commit_id,
                "comment": format!("Fusion de {source} dans {target}"),
                "author": {"name": "azurz", "date": "2026-10-29T00:00:00Z"},
                "committer": {"name": "azurz", "date": "2026-10-29T00:00:00Z"},
                "parents": parents
            }),
        );
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
        pull["status"] = json!("completed");
        pull["mergeStatus"] = json!("succeeded");
        pull["closedDate"] = json!("2026-10-29T00:00:00Z");
        pull["closedBy"] = json!({"displayName": "azurz", "uniqueName": "azurz"});
        pull["lastMergeCommit"] = json!({"commitId": commit_id});
        pull["lastMergeSourceCommit"] = json!({"commitId": parent.unwrap_or_default()});
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

fn builds(state: &AppState, method: &Method, org: &str, project: &str, rest: &[String], body: &[u8]) -> Response {
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
        let artifacts = format!("build/builds/{id}/artifacts");
        if method == Method::GET && rest.len() == 4 {
            return Json(page(list(state, org, project, &artifacts))).into_response();
        }
        if method == Method::POST {
            let payload = parse_json(body).unwrap_or(json!({}));
            let name = text_of(&payload, "name").unwrap_or_else(|| "drop".into());
            let item = json!({
                "id": list(state, org, project, &artifacts).len() + 1,
                "name": name,
                "resource": payload.get("resource").cloned().unwrap_or(json!({"type": "PipelineArtifact"}))
            });
            put_item(state, org, project, &artifacts, item.clone());
            return Json(item).into_response();
        }
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
        let pipeline_id = rest[2].parse::<i64>().unwrap_or(1);
        let name = get_item(state, org, project, key, &pipeline_id.to_string())
            .and_then(|item| text_of(&item, "name"))
            .unwrap_or_else(|| "CI".into());
        let runs_key = "pipelines/runs";
        let next = list(state, org, project, runs_key).len() as i64 + 1;
        let item = json!({
            "id": next,
            "name": format!("{name}-{next}"),
            "state": "inProgress",
            "pipeline": {"id": pipeline_id, "name": name, "folder": "\\", "revision": 1},
            "createdDate": "2026-10-29T00:00:00Z",
            "url": format!("/{org}/{project}/_apis/pipelines/runs/{next}")
        });
        put_item(state, org, project, runs_key, item.clone());
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
    if method == Method::GET {
        let id = rest.get(2).map(String::as_str).unwrap_or("");
        return match org_get(state, org, key, id) {
            Some(item) => Json(item).into_response(),
            None => fail(StatusCode::NOT_FOUND, "abonnement introuvable"),
        };
    }
    if method == Method::DELETE {
        let id = rest.get(2).map(String::as_str).unwrap_or("").to_string();
        let removed = state.write(|db| {
            db.orgs.get_mut(org).and_then(|organization| organization.buckets.get_mut(key)).map(|bucket| bucket.remove(&id)).unwrap_or(false)
        });
        return if removed {
            StatusCode::NO_CONTENT.into_response()
        } else {
            fail(StatusCode::NOT_FOUND, "abonnement introuvable")
        };
    }
    fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée")
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

fn classification(org: &str, project: &str, rest: &[String]) -> Value {
    let kind = rest.get(2).map(|value| value.to_ascii_lowercase()).unwrap_or_else(|| "areas".into());
    if kind == "iterations" {
        return json!({
            "id": 2,
            "identifier": "iteration",
            "name": project,
            "structureType": "iteration",
            "hasChildren": true,
            "path": format!("\\{project}\\Iteration"),
            "url": format!("/{org}/{project}/_apis/wit/classificationnodes/iterations"),
            "children": [{
                "id": 3,
                "name": "Sprint 1",
                "structureType": "iteration",
                "hasChildren": false,
                "path": format!("\\{project}\\Iteration\\Sprint 1")
            }]
        });
    }
    json!({
        "id": 1,
        "identifier": "area",
        "name": project,
        "structureType": "area",
        "hasChildren": false,
        "path": format!("\\{project}\\Area"),
        "url": format!("/{org}/{project}/_apis/wit/classificationnodes/areas")
    })
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

fn task_agents(state: &AppState, method: &Method, org: &str, project: Option<&str>, rest: &[String]) -> Response {
    if method != Method::GET {
        return fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée");
    }
    let resource = rest.get(1).map(|item| item.to_ascii_lowercase()).unwrap_or_default();
    if resource == "pools" {
        return Json(page(org_list(state, org, "distributedtask/pools"))).into_response();
    }
    if resource == "queues" {
        let Some(project) = project else {
            return fail(StatusCode::NOT_FOUND, "file introuvable");
        };
        return Json(page(list(state, org, project, "distributedtask/queues"))).into_response();
    }
    fail(StatusCode::NOT_FOUND, "ressource introuvable")
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
    }
    item
}

fn promote_pipeline(state: &AppState, org: &str, project: &str, mut item: Value) -> Value {
    if text_of(&item, "state").as_deref() == Some("inProgress") {
        item["state"] = json!("completed");
        item["result"] = json!("succeeded");
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
