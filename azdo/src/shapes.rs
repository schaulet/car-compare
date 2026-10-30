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
    if area == "distributedtask" && resource == "environments" && rest.get(3).is_some_and(|part| part.eq_ignore_ascii_case("providers")) {
        if rest.get(4).is_some_and(|part| part.eq_ignore_ascii_case("kubernetes")) {
            return Some(environment_kubernetes(state, method, org, project, rest, body));
        }
        return Some(environment_virtual_machines(state, method, org, project, rest, body));
    }
    if area == "distributedtask" && resource == "environments" && method == Method::DELETE {
        return Some(delete_environment(state, org, project, rest));
    }
    let response = match (area.as_str(), resource.as_str()) {
        ("git", "repositories") => git_repositories(state, method, org, project, rest, query, body),
        ("git", "refs") => git_alias(state, method, org, project, "refs", query, body),
        ("git", "commits") => git_alias(state, method, org, project, "commits", query, body),
        ("git", "pushes") => git_alias(state, method, org, project, "pushes", query, body),
        ("git", "pullrequests") => pull_requests(state, method, org, project, None, rest, body),
        ("build", "definitions") => definitions(state, method, org, project, rest, body),
        ("build", "folders") => build_folders(state, method, org, project, rest, body),
        ("build", "builds") => builds(state, method, org, project, rest, query, body),
        ("build", "tags") => project_build_tags(state, method, org, project, rest),
        ("build", "retention") => build_retention(state, method, org, project, rest, query, body),
        ("distributedtask", "queues") | ("distributedtask", "pools") => {
            task_agents(state, method, org, Some(project), rest, query, body)
        }
        ("distributedtask", "variablegroups") => variable_groups(state, method, org, project, rest, body),
        ("distributedtask", "securefiles") => secure_files(state, method, org, project, rest, query, body),
        ("distributedtask", "taskgroups") => task_groups(state, method, org, project, rest, body),
        ("distributedtask", "deploymentgroups") => deployment_groups(state, method, org, project, rest, body),
        ("pipelines", "pipelines") => pipelines(state, method, org, project, rest, body),
        ("pipelines", "runs") => pipeline_runs(state, method, org, project, rest),
        ("pipelines", "checks") => check_configurations(state, method, org, project, rest, query, body),
        ("pipelines", "approvals") => pipeline_approvals(state, method, org, project, rest, query, body),
        ("testplan", "plans") | ("test", "plans") => test_plans(state, method, org, project, rest, body),
        ("test", "configurations") | ("testplan", "configurations") => test_configurations(state, method, org, project, rest, body),
        ("testplan", "variables") => test_variables(state, method, org, project, rest, body),
        ("testplan", "sharedparameters") => shared_parameters(state, method, org, project, rest, body),
        ("test", "runs") => test_runs(state, method, org, project, rest, body),
        ("testresults", "codecoverage") => test_code_coverage(state, method, org, project, rest, query, body),
        ("testresults", "testfailuretype") => test_failure_types(state, method, org, project, rest, body),
        ("testresults", "results") => test_result_metadata(state, method, org, project, rest, body),
        ("testresults", "testmethods") => test_method_links(state, method, org, project, rest, query, body),
        ("testresults", "runs") => {
            if rest.get(3).is_some_and(|part| part.eq_ignore_ascii_case("tags")) {
                test_run_tags(state, method, org, project, rest, body)
            } else if rest.get(3).is_some_and(|part| part.eq_ignore_ascii_case("customfields")) {
                test_run_custom_fields(state, method, org, project, rest, body)
            } else {
                test_log_store(state, method, org, project, rest, query, body)
            }
        }
        ("wiki", "wikis") => wikis(state, method, org, project, rest, query, body),
        ("packaging", "feeds") => feeds(state, method, org, project, rest, body),
        ("release", "folders") => release_folders(state, method, org, project, rest, body),
        ("release", "definitions") => release_definitions(state, method, org, project, rest, body),
        ("release", "releases") => release_releases(state, method, org, project, rest, body),
        ("release", "approvals") => release_approvals(state, method, org, project, rest, body),
        ("policy", "types") => policy_types(method, org, project, rest),
        ("policy", "configurations") => policy_configurations(state, method, org, project, rest, body),
        ("policy", "evaluations") => policy_evaluations(state, method, org, project, query, body),
        ("audit", "streams") => audit_streams(state, method, org, Some(project), rest, body),
        ("extensionmanagement", "installedextensions") => installed_extensions(state, method, org, Some(project), rest, body),
        ("memberentitlementmanagement", "groupentitlements") => group_entitlements(state, method, org, Some(project), rest, body),
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
        ("notification", "subscriptions") => notification_subscriptions(state, method, org, rest, body),
        ("favorite", "favorites") => favorites(state, method, org, rest, query, body),
        ("memberentitlementmanagement", "userentitlements") => user_entitlements(state, method, org, rest, query, body),
        ("distributedtask", "pools") | ("distributedtask", "queues") => {
            task_agents(state, method, org, None, rest, query, body)
        }
        ("serviceendpoint", "types") => crate::tracking::endpoint_types(method),
        ("graph", "users") => graph(state, method, org, "graph/users", "azurz"),
        ("graph", "groups") => graph_groups(state, method, org, rest, query, body),
        ("graph", "memberships") => graph_memberships(state, method, org, rest, query, body),
        ("work", "processes") => {
            let project = query_param(query, "project")
                .map(pct_decode)
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty());
            if rest.iter().any(|part| part.eq_ignore_ascii_case("lists")) {
                match project {
                    Some(project) => crate::tracking::picklists(state, method, org, &project, rest, body),
                    None => crate::tracking::picklists_org(state, method, org, rest, body),
                }
            } else {
                crate::tracking::work_processes(state, method, org, project.as_deref(), rest, body)
            }
        }
        ("wit", "fields") => {
            let project = query_param(query, "project")
                .map(pct_decode)
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty());
            custom_fields(state, method, org, project.as_deref(), rest, body)
        }
        ("audit", "streams") => {
            let project = query_param(query, "project")
                .map(pct_decode)
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty());
            audit_streams(state, method, org, project.as_deref(), rest, body)
        }
        ("extensionmanagement", "installedextensions") => {
            let project = query_param(query, "project")
                .map(pct_decode)
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty());
            installed_extensions(state, method, org, project.as_deref(), rest, body)
        }
        ("memberentitlementmanagement", "groupentitlements") => {
            let project = query_param(query, "project")
                .map(pct_decode)
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty());
            group_entitlements(state, method, org, project.as_deref(), rest, body)
        }
        _ => return None,
    };
    Some(response)
}

pub fn wit_catalog(state: &AppState, method: &Method, org: &str, project: &str, rest: &[String]) -> Option<Response> {
    if method != Method::GET {
        return None;
    }
    let resource = rest.get(1)?.to_ascii_lowercase();
    if resource == "workitemtypes" && rest.len() == 2 {
        return Some(Json(work_item_types()).into_response());
    }
    if resource == "workitemtypes" && rest.len() >= 3 {
        let Some(kind) = canonical_work_item_type(&rest[2]) else {
            return Some(fail(StatusCode::NOT_FOUND, "type introuvable"));
        };
        if rest.len() == 3 {
            return Some(Json(work_item_type_document(kind)).into_response());
        }
        if rest.get(3).is_some_and(|value| value.eq_ignore_ascii_case("states")) {
            let states = type_state_values();
            if rest.len() == 4 {
                return Some(Json(page(states)).into_response());
            }
            if rest.len() == 5 {
                let wanted = rest[4].trim();
                return Some(match states.iter().find(|state| {
                    state.get("name").and_then(|value| value.as_str()).is_some_and(|name| name.eq_ignore_ascii_case(wanted))
                }) {
                    Some(state) => Json(state.clone()).into_response(),
                    None => fail(StatusCode::NOT_FOUND, "état introuvable"),
                });
            }
        }
        if rest.get(3).is_some_and(|value| value.eq_ignore_ascii_case("transitions")) && rest.len() == 4 {
            let extras = mapped_work_item_states(state, org, project, kind);
            return Some(Json(work_item_transitions(&extras)).into_response());
        }
        return Some(fail(StatusCode::NOT_FOUND, "type introuvable"));
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
                let from_items = bucket
                    .items
                    .values()
                    .filter_map(|item| value_key(&item["id"]).strip_prefix("repo-")?.parse::<i64>().ok())
                    .max()
                    .unwrap_or(0);
                let next = bucket.next.max(from_items + 1);
                let id = format!("repo-{next}");
                let item = dress_repo(
                    org,
                    project,
                    json!({"id": id, "name": name, "defaultBranch": "refs/heads/main", "size": 0}),
                );
                bucket.put(item.clone());
                if next >= bucket.next {
                    bucket.next = next + 1;
                }
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
    if method == Method::DELETE && rest.len() == 2 {
        return fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée");
    }
    let repo_id = rest.get(2).map(String::as_str).unwrap_or("");
    if method == Method::DELETE && rest.len() > 3 {
        let sub = rest[3].to_ascii_lowercase();
        let known = matches!(
            sub.as_str(),
            "refs"
                | "annotatedtags"
                | "commits"
                | "pushes"
                | "cherrypicks"
                | "reverts"
                | "pullrequests"
                | "labels"
                | "items"
                | "diffs"
        );
        if !known {
            return fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée");
        }
    }
    if method == Method::DELETE && rest.len() == 3 {
        let Some(found) = find_repo(state, org, project, repo_id) else {
            return fail(StatusCode::NOT_FOUND, "dépôt introuvable");
        };
        let stored_id = value_key(&found["id"]);
        if stored_id == "11111111-1111-1111-1111-111111111111" {
            return fail(StatusCode::BAD_REQUEST, "dépôt par défaut");
        }
        state.write(|db| {
            if let Some(stored) = db.project_mut(org, project) {
                if let Some(bucket) = stored.buckets.get_mut("git/repositories") {
                    bucket.remove(&stored_id);
                }
            }
        });
        return StatusCode::NO_CONTENT.into_response();
    }
    if rest.len() == 3 && method == Method::GET {
        return match find_repo(state, org, project, repo_id) {
            Some(item) => Json(dress_repo(org, project, item)).into_response(),
            None => fail(StatusCode::NOT_FOUND, "dépôt introuvable"),
        };
    }
    if rest.len() > 3 && find_repo(state, org, project, repo_id).is_none() {
        return fail(StatusCode::NOT_FOUND, "dépôt introuvable");
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
    let default_branch = default_branch_name(state, org, project, repo_id);
    if updates.iter().any(|update| {
        text_of(update, "newObjectId").as_deref().is_some_and(is_zero_object_id)
            && text_of(update, "name").unwrap_or_else(|| "refs/heads/main".into()).eq_ignore_ascii_case(&default_branch)
    }) {
        return fail(StatusCode::BAD_REQUEST, "branche par défaut");
    }
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
                let stored_name = stored.buckets.get(&key).and_then(|bucket| {
                    bucket.items.iter().find_map(|(id, item)| {
                        let matches = id.eq_ignore_ascii_case(&name)
                            || text_of(item, "name").is_some_and(|current| current.eq_ignore_ascii_case(&name));
                        matches.then(|| id.clone())
                    })
                });
                let removed = stored_name.as_ref().is_some_and(|stored_name| {
                    stored.buckets.get_mut(&key).is_some_and(|bucket| bucket.remove(stored_name))
                });
                let tag_name = stored_name.as_deref().unwrap_or(&name);
                if let Some(short) = tag_name.strip_prefix("refs/tags/") {
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

fn default_branch_name(state: &AppState, org: &str, project: &str, repo_id: &str) -> String {
    find_repo(state, org, project, repo_id)
        .and_then(|item| text_of(&item, "defaultBranch"))
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| "refs/heads/main".into())
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
        if tail.get(1).is_some_and(|part| part.eq_ignore_ascii_case("comments")) {
            return commit_comments(state, method, org, project, repo_id, commit_id, &tail[2..], body);
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

pub fn comment_reactions(
    state: &AppState,
    method: &Method,
    org: &str,
    project: &str,
    work_item: i64,
    comment_id: i64,
    rest: &[String],
    body: &[u8],
) -> Response {
    match comment_reaction_parent(state, org, project, work_item, comment_id) {
        Ok(()) => {}
        Err(message) => return fail(StatusCode::NOT_FOUND, message),
    }
    let key = format!("wit/workitems/{work_item}/comments/{comment_id}/reactions");
    if rest.is_empty() {
        if method == Method::GET {
            let mut rows = list(state, org, project, &key);
            rows.sort_by_key(|item| json_i64(&item["id"]).unwrap_or(0));
            let value = rows
                .into_iter()
                .map(|item| dress_comment_reaction(org, project, work_item, comment_id, item))
                .collect();
            return Json(page(value)).into_response();
        }
        if method == Method::POST {
            let kind = match reaction_type_of(body) {
                Ok(kind) => kind,
                Err(message) => return fail(StatusCode::BAD_REQUEST, message),
            };
            if let Some(existing) = list(state, org, project, &key)
                .into_iter()
                .find(|item| text_of(item, "type").as_deref() == Some(kind))
            {
                return Json(dress_comment_reaction(org, project, work_item, comment_id, existing)).into_response();
            }
            let next = list(state, org, project, &key)
                .into_iter()
                .filter_map(|item| json_i64(&item["id"]))
                .max()
                .unwrap_or(0)
                + 1;
            let item = json!({ "id": next, "type": kind });
            put_item(state, org, project, &key, item.clone());
            return Json(dress_comment_reaction(org, project, work_item, comment_id, item)).into_response();
        }
        return fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée");
    }
    if rest.len() == 1 {
        let Ok(reaction_id) = rest[0].parse::<i64>() else {
            return fail(StatusCode::NOT_FOUND, "réaction introuvable");
        };
        let id = reaction_id.to_string();
        let Some(item) = get_item(state, org, project, &key, &id) else {
            return fail(StatusCode::NOT_FOUND, "réaction introuvable");
        };
        if method == Method::GET {
            return Json(dress_comment_reaction(org, project, work_item, comment_id, item)).into_response();
        }
        if method == Method::DELETE {
            return if delete_item(state, org, project, &key, &id) {
                StatusCode::NO_CONTENT.into_response()
            } else {
                fail(StatusCode::NOT_FOUND, "réaction introuvable")
            };
        }
        return fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée");
    }
    if method == Method::DELETE {
        return fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée");
    }
    fail(StatusCode::NOT_FOUND, "réaction introuvable")
}

fn comment_reaction_parent(state: &AppState, org: &str, project: &str, work_item: i64, comment_id: i64) -> Result<(), &'static str> {
    state.read(|db| {
        let Some(stored) = db.project(org, project) else {
            return Err("work item introuvable");
        };
        if !stored.work_items.contains_key(&work_item) {
            return Err("work item introuvable");
        }
        let visible = stored.visible_comments(work_item).unwrap_or_default();
        if visible.iter().any(|note| note.get("id").and_then(|value| value.as_i64()) == Some(comment_id)) {
            Ok(())
        } else {
            Err("commentaire introuvable")
        }
    })
}

fn reaction_type_of(body: &[u8]) -> Result<&'static str, &'static str> {
    let Some(payload) = parse_json(body) else {
        return Err("JSON illisible");
    };
    let Some(text) = payload.as_object().and_then(|object| object.get("type")).and_then(|value| value.as_str()) else {
        return Err("réaction absente");
    };
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Err("réaction absente");
    }
    match trimmed.to_ascii_lowercase().as_str() {
        "like" => Ok("like"),
        "dislike" => Ok("dislike"),
        "heart" => Ok("heart"),
        "hooray" => Ok("hooray"),
        "smile" => Ok("smile"),
        "confused" => Ok("confused"),
        _ => Err("réaction inconnue"),
    }
}

fn dress_comment_reaction(org: &str, project: &str, work_item: i64, comment_id: i64, mut item: Value) -> Value {
    let id = value_key(&item["id"]);
    if let Some(object) = item.as_object_mut() {
        object.insert(
            "url".into(),
            json!(format!("/{org}/{project}/_apis/wit/workItems/{work_item}/comments/{comment_id}/reactions/{id}")),
        );
    }
    item
}

fn commit_comment_content(body: &[u8]) -> Result<String, &'static str> {
    let Some(payload) = parse_json(body) else {
        return Err("JSON illisible");
    };
    let Some(text) = payload.as_object().and_then(|object| object.get("content")).and_then(|value| value.as_str()) else {
        return Err("contenu absent");
    };
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Err("contenu absent");
    }
    Ok(trimmed.to_string())
}

fn dress_commit_comment(org: &str, project: &str, repo_id: &str, commit_id: &str, mut item: Value) -> Value {
    let id = value_key(&item["id"]);
    if let Some(object) = item.as_object_mut() {
        object.insert(
            "url".into(),
            json!(format!("/{org}/{project}/_apis/git/repositories/{repo_id}/commits/{commit_id}/comments/{id}")),
        );
    }
    item
}

fn commit_comments(
    state: &AppState,
    method: &Method,
    org: &str,
    project: &str,
    repo_id: &str,
    commit_id: &str,
    rest: &[String],
    body: &[u8],
) -> Response {
    let commits_key = format!("git/repositories/{repo_id}/commits");
    if get_item(state, org, project, &commits_key, commit_id).is_none() {
        return fail(StatusCode::NOT_FOUND, "commit introuvable");
    }
    let key = format!("{commits_key}/{commit_id}/comments");
    if rest.is_empty() {
        if method == Method::GET {
            let mut rows = list(state, org, project, &key);
            rows.sort_by_key(|item| json_i64(&item["id"]).unwrap_or(0));
            let value = rows
                .into_iter()
                .map(|item| dress_commit_comment(org, project, repo_id, commit_id, item))
                .collect();
            return Json(page(value)).into_response();
        }
        if method == Method::POST {
            let content = match commit_comment_content(body) {
                Ok(content) => content,
                Err(message) => return fail(StatusCode::BAD_REQUEST, message),
            };
            let next = list(state, org, project, &key)
                .into_iter()
                .filter_map(|item| json_i64(&item["id"]))
                .max()
                .unwrap_or(0)
                + 1;
            let item = json!({ "id": next, "content": content });
            put_item(state, org, project, &key, item.clone());
            return Json(dress_commit_comment(org, project, repo_id, commit_id, item)).into_response();
        }
        return fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée");
    }
    if rest.len() == 1 {
        let Ok(comment_id) = rest[0].parse::<i64>() else {
            return fail(StatusCode::NOT_FOUND, "commentaire introuvable");
        };
        let id = comment_id.to_string();
        let Some(item) = get_item(state, org, project, &key, &id) else {
            return fail(StatusCode::NOT_FOUND, "commentaire introuvable");
        };
        if method == Method::GET {
            return Json(dress_commit_comment(org, project, repo_id, commit_id, item)).into_response();
        }
        if method == Method::DELETE {
            return if delete_item(state, org, project, &key, &id) {
                StatusCode::NO_CONTENT.into_response()
            } else {
                fail(StatusCode::NOT_FOUND, "commentaire introuvable")
            };
        }
        return fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée");
    }
    if method == Method::DELETE {
        return fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée");
    }
    fail(StatusCode::NOT_FOUND, "commentaire introuvable")
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
    if find_repo(state, org, project, repo_id).is_none() {
        return fail(StatusCode::NOT_FOUND, "dépôt introuvable");
    }
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
    let payload = if body.iter().all(|byte| byte.is_ascii_whitespace()) {
        json!({})
    } else {
        match parse_json(body) {
            Some(value) if value.is_object() => value,
            _ => return fail(StatusCode::BAD_REQUEST, "poussée illisible"),
        }
    };
    if let Err(message) = validate_push_files(state, org, project, repo_id, &payload) {
        return fail(StatusCode::BAD_REQUEST, message);
    }
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
        queue_push_triggers(state, org, project, repo_id, &item);
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
            queue_push_triggers(state, org, project, repo_id, &item);
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
            queue_push_triggers(state, org, project, repo_id, &push);
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

fn validate_push_files(state: &AppState, org: &str, project: &str, repo_id: &str, payload: &Value) -> Result<(), &'static str> {
    let Some(commits) = payload.get("commits") else {
        return Ok(());
    };
    let Some(commits) = commits.as_array() else {
        return Err("poussée illisible");
    };
    let mut files = list(state, org, project, &format!("git/repositories/{repo_id}/items"))
        .into_iter()
        .filter_map(|item| {
            let path = text_of(&item, "path")?;
            let folder = item.get("isFolder").and_then(|value| value.as_bool()).unwrap_or(false);
            Some((path, folder))
        })
        .collect::<BTreeMap<_, _>>();
    for commit in commits {
        if !commit.is_object() {
            return Err("poussée illisible");
        }
        let Some(changes) = commit.get("changes") else {
            continue;
        };
        let Some(changes) = changes.as_array() else {
            return Err("fichier illisible");
        };
        for change in changes {
            let Some(change) = change.as_object() else {
                return Err("fichier illisible");
            };
            let kind = match change.get("changeType") {
                None => "add",
                Some(Value::String(text)) => {
                    let trimmed = text.trim();
                    if trimmed.is_empty() {
                        return Err("type de changement absent");
                    }
                    match trimmed.to_ascii_lowercase().as_str() {
                        "add" => "add",
                        "edit" => "edit",
                        "delete" => "delete",
                        _ => return Err("type de changement inconnu"),
                    }
                }
                Some(_) => return Err("fichier illisible"),
            };
            let path = match change.get("item") {
                None => return Err("chemin de fichier absent"),
                Some(Value::Object(item)) => match item.get("path") {
                    Some(Value::String(text)) => text.clone(),
                    Some(_) => return Err("fichier illisible"),
                    None => return Err("chemin de fichier absent"),
                },
                Some(_) => return Err("fichier illisible"),
            };
            let path = normalize_file_path(&path)?;
            if path == "/" || files.get(&path) == Some(&true) {
                return Err("chemin de fichier refusé");
            }
            if (kind == "edit" || kind == "delete") && files.get(&path) != Some(&false) {
                return Err("fichier introuvable");
            }
            if kind != "delete" {
                match change.get("newContent") {
                    None | Some(Value::String(_)) => {}
                    Some(Value::Object(content)) => match content.get("content") {
                        None | Some(Value::String(_)) => {}
                        Some(_) => return Err("contenu illisible"),
                    },
                    Some(_) => return Err("contenu illisible"),
                }
            }
            if kind == "delete" {
                files.remove(&path);
            } else {
                files.insert(path, false);
            }
        }
    }
    Ok(())
}

fn normalize_file_path(path: &str) -> Result<String, &'static str> {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return Err("chemin de fichier absent");
    }
    let mut path = if trimmed.starts_with('/') { trimmed.to_string() } else { format!("/{trimmed}") };
    while path.len() > 1 && path.ends_with('/') {
        path.pop();
    }
    Ok(path)
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
            let mut payload = parse_json(body).unwrap_or(json!({}));
            normalize_autocomplete(&mut payload);
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
                    let mut item = if completed && previous.as_deref() != Some("completed") {
                        let prepared = if squash_requested(&item) {
                            Some(Prepared::Squash(prepare_squash(state, org, project, &item)))
                        } else if rebase_requested(&item) {
                            Some(Prepared::Rebase(prepare_rebase(state, org, project, &item)))
                        } else {
                            Some(Prepared::Merge(prepare_merge(state, org, project, &item)))
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
                        if let Some(done) = try_autocomplete(state, org, project, &id) {
                            item = done;
                        } else {
                            deliver_work_item(state, org, project, "git.pullrequest.updated", &item);
                        }
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
    if payload.get("isDraft").and_then(|value| value.as_bool()) != Some(true) {
        let paths = pull_change_paths(state, org, project, &item);
        queue_matching_triggers(state, org, project, "pullRequest", &target, &source, &paths);
    }
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

fn pull_threads(
    state: &AppState,
    method: &Method,
    org: &str,
    project: &str,
    pull_id: &str,
    tail: &[String],
    body: &[u8],
) -> Response {
    let key = format!("git/pullrequests/{pull_id}/threads");
    let pull_exists = get_item(state, org, project, "git/pullrequests", pull_id).is_some();
    if !pull_exists {
        return fail(StatusCode::NOT_FOUND, "demande de tirage introuvable");
    }
    if tail.is_empty() {
        if method == Method::GET {
            let value = list(state, org, project, &key).into_iter().map(visible_thread).collect();
            return Json(page(value)).into_response();
        }
        if method != Method::POST {
            return fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée");
        }
        let payload = match thread_object(body) {
            Ok(payload) => payload,
            Err(message) => return fail(StatusCode::BAD_REQUEST, message),
        };
        let status = match thread_status_of(&payload, true) {
            Ok(status) => status,
            Err(message) => return fail(StatusCode::BAD_REQUEST, message),
        };
        let comments = match thread_comments_from(&payload) {
            Ok(comments) => comments,
            Err(message) => return fail(StatusCode::BAD_REQUEST, message),
        };
        let next = list(state, org, project, &key).len() as i64 + 1;
        let thread = json!({
            "id": next,
            "publishedDate": "2026-10-29T00:00:00Z",
            "lastUpdatedDate": "2026-10-29T00:00:00Z",
            "status": status,
            "comments": comments
        });
        put_item(state, org, project, &key, thread.clone());
        let shown = visible_thread(thread);
        deliver_work_item(state, org, project, "git.pullrequest.commented", &shown);
        return Json(shown).into_response();
    }
    let thread_id = &tail[0];
    let stored = get_item(state, org, project, &key, thread_id);
    if tail.len() == 1 {
        if stored.is_none() {
            return fail(StatusCode::NOT_FOUND, "fil introuvable");
        }
        if method == Method::GET {
            return Json(visible_thread(stored.unwrap())).into_response();
        }
        if method != Method::PATCH {
            return fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée");
        }
        let payload = match thread_object(body) {
            Ok(payload) => payload,
            Err(message) => return fail(StatusCode::BAD_REQUEST, message),
        };
        let status = match thread_status_of(&payload, false) {
            Ok(status) => status,
            Err(message) => return fail(StatusCode::BAD_REQUEST, message),
        };
        let mut thread = stored.unwrap();
        if let Some(object) = thread.as_object_mut() {
            object.insert("status".into(), json!(status));
            object.insert("lastUpdatedDate".into(), json!("2026-10-29T00:00:00Z"));
        }
        put_item(state, org, project, &key, thread.clone());
        let shown = visible_thread(thread);
        deliver_work_item(state, org, project, "git.pullrequest.commented", &shown);
        return Json(shown).into_response();
    }
    if tail.len() < 2 || !tail[1].eq_ignore_ascii_case("comments") {
        return fail(StatusCode::NOT_FOUND, "fil introuvable");
    }
    let Some(mut thread) = stored else {
        return fail(StatusCode::NOT_FOUND, "fil introuvable");
    };
    let comment_id = if tail.len() >= 3 {
        match tail[2].parse::<i64>() {
            Ok(value) => Some(value),
            Err(_) => return fail(StatusCode::NOT_FOUND, "commentaire introuvable"),
        }
    } else {
        None
    };
    let notes = thread.get("comments").and_then(|value| value.as_array()).cloned().unwrap_or_default();
    if method == Method::GET && comment_id.is_none() {
        let visible = notes.into_iter().filter(|note| !comment_deleted(note)).collect::<Vec<_>>();
        return Json(page(visible)).into_response();
    }
    if method == Method::POST && comment_id.is_none() {
        let content = match comment_content(body) {
            Ok(content) => content,
            Err(message) => return fail(StatusCode::BAD_REQUEST, message),
        };
        let next = notes.iter().filter_map(|note| note.get("id").and_then(|value| value.as_i64())).max().unwrap_or(0) + 1;
        let note = new_thread_comment(next, &content);
        let mut comments = notes;
        comments.push(note.clone());
        if let Some(object) = thread.as_object_mut() {
            object.insert("comments".into(), json!(comments));
            object.insert("lastUpdatedDate".into(), json!("2026-10-29T00:00:00Z"));
        }
        put_item(state, org, project, &key, thread);
        deliver_work_item(state, org, project, "git.pullrequest.commented", &note);
        return Json(note).into_response();
    }
    let Some(comment_id) = comment_id else {
        return fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée");
    };
    let position = notes.iter().position(|note| note.get("id").and_then(|value| value.as_i64()) == Some(comment_id) && !comment_deleted(note));
    if position.is_none() {
        return fail(StatusCode::NOT_FOUND, "commentaire introuvable");
    }
    let position = position.unwrap();
    if method == Method::GET {
        return Json(notes[position].clone()).into_response();
    }
    if method == Method::DELETE {
        let mut comments = notes;
        let mut note = comments[position].clone();
        if let Some(object) = note.as_object_mut() {
            object.insert("isDeleted".into(), json!(true));
            object.insert("lastUpdatedDate".into(), json!("2026-10-29T00:00:00Z"));
        }
        comments[position] = note.clone();
        if let Some(object) = thread.as_object_mut() {
            object.insert("comments".into(), json!(comments));
            object.insert("lastUpdatedDate".into(), json!("2026-10-29T00:00:00Z"));
        }
        put_item(state, org, project, &key, thread);
        return Json(note).into_response();
    }
    if method != Method::PATCH {
        return fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée");
    }
    let content = match comment_content(body) {
        Ok(content) => content,
        Err(message) => return fail(StatusCode::BAD_REQUEST, message),
    };
    let mut comments = notes;
    let mut note = comments[position].clone();
    let version = note.get("version").and_then(|value| value.as_i64()).unwrap_or(1) + 1;
    if let Some(object) = note.as_object_mut() {
        object.insert("content".into(), json!(content));
        object.insert("version".into(), json!(version));
        object.insert("modifiedBy".into(), json!({"displayName": "azurz", "uniqueName": "azurz"}));
        object.insert("modifiedDate".into(), json!("2026-10-29T00:00:00Z"));
        object.insert("lastUpdatedDate".into(), json!("2026-10-29T00:00:00Z"));
        object.insert("lastContentUpdatedDate".into(), json!("2026-10-29T00:00:00Z"));
    }
    comments[position] = note.clone();
    if let Some(object) = thread.as_object_mut() {
        object.insert("comments".into(), json!(comments));
        object.insert("lastUpdatedDate".into(), json!("2026-10-29T00:00:00Z"));
    }
    put_item(state, org, project, &key, thread);
    deliver_work_item(state, org, project, "git.pullrequest.commented", &note);
    Json(note).into_response()
}

fn thread_object(body: &[u8]) -> Result<Map<String, Value>, &'static str> {
    if body.iter().all(|byte| byte.is_ascii_whitespace()) {
        return Ok(Map::new());
    }
    match parse_json(body) {
        Some(Value::Object(map)) => Ok(map),
        _ => Err("fil illisible"),
    }
}

fn thread_status_of(payload: &Map<String, Value>, default_active: bool) -> Result<&'static str, &'static str> {
    let Some(value) = payload.get("status") else {
        return if default_active { Ok("active") } else { Err("statut de fil absent") };
    };
    let Some(text) = value.as_str() else {
        return Err("statut de fil absent");
    };
    if text.trim().is_empty() {
        return Err("statut de fil absent");
    }
    thread_status(text).ok_or("statut de fil inconnu")
}

fn thread_status(value: &str) -> Option<&'static str> {
    [
        ("active", "active"),
        ("fixed", "fixed"),
        ("wontfix", "wontFix"),
        ("closed", "closed"),
        ("bydesign", "byDesign"),
        ("pending", "pending"),
    ]
    .into_iter()
    .find(|(name, _)| value.eq_ignore_ascii_case(name))
    .map(|(_, canonical)| canonical)
}

fn thread_comments_from(payload: &Map<String, Value>) -> Result<Vec<Value>, &'static str> {
    let Some(raw) = payload.get("comments") else {
        let content = payload.get("content").and_then(|value| value.as_str()).unwrap_or("");
        return Ok(vec![new_thread_comment(1, content)]);
    };
    let Some(list) = raw.as_array() else {
        return Err("commentaire illisible");
    };
    if list.is_empty() {
        return Err("commentaire absent");
    }
    let mut notes = Vec::new();
    for (index, item) in list.iter().enumerate() {
        let Some(text) = item.get("content").and_then(|value| value.as_str()) else {
            return Err("commentaire illisible");
        };
        if text.trim().is_empty() {
            return Err("commentaire absent");
        }
        notes.push(new_thread_comment(index as i64 + 1, text));
    }
    Ok(notes)
}

fn comment_content(body: &[u8]) -> Result<String, &'static str> {
    let Some(object) = parse_json(body).and_then(|value| value.as_object().cloned()) else {
        return Err("commentaire illisible");
    };
    let Some(text) = object.get("content").and_then(|value| value.as_str()) else {
        return Err("commentaire illisible");
    };
    if text.trim().is_empty() {
        return Err("commentaire absent");
    }
    Ok(text.to_string())
}

fn new_thread_comment(id: i64, content: &str) -> Value {
    json!({
        "id": id,
        "parentCommentId": 0,
        "content": content,
        "commentType": "text",
        "version": 1,
        "author": {"displayName": "azurz", "uniqueName": "azurz"},
        "publishedDate": "2026-10-29T00:00:00Z",
        "lastUpdatedDate": "2026-10-29T00:00:00Z",
        "lastContentUpdatedDate": "2026-10-29T00:00:00Z"
    })
}

fn comment_deleted(note: &Value) -> bool {
    note.get("isDeleted").and_then(|value| value.as_bool()) == Some(true)
}

fn visible_thread(mut thread: Value) -> Value {
    if let Some(object) = thread.as_object_mut() {
        let visible = object
            .get("comments")
            .and_then(|value| value.as_array())
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .filter(|note| !comment_deleted(note))
            .collect::<Vec<_>>();
        object.insert("comments".into(), json!(visible));
    }
    thread
}

fn pull_statuses(
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
    let key = format!("git/pullrequests/{pull_id}/statuses");
    if tail.is_empty() {
        if method == Method::GET {
            let mut rows = list(state, org, project, &key);
            rows.sort_by_key(|item| json_i64(&item["id"]).unwrap_or(0));
            let value = rows
                .into_iter()
                .map(|item| dress_pull_status(org, project, &repo_id, pull_id, item))
                .collect();
            return Json(page(value)).into_response();
        }
        if method == Method::POST {
            let item = match pull_status_create(body) {
                Ok(item) => item,
                Err(message) => return fail(StatusCode::BAD_REQUEST, message),
            };
            let next = list(state, org, project, &key)
                .into_iter()
                .filter_map(|row| json_i64(&row["id"]))
                .max()
                .unwrap_or(0)
                + 1;
            let stored = {
                let mut item = item;
                if let Some(object) = item.as_object_mut() {
                    object.insert("id".into(), json!(next));
                }
                item
            };
            put_item(state, org, project, &key, stored.clone());
            return Json(dress_pull_status(org, project, &repo_id, pull_id, stored)).into_response();
        }
        return fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée");
    }
    if tail.len() != 1 {
        if method == Method::DELETE {
            return fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée");
        }
        return fail(StatusCode::NOT_FOUND, "statut introuvable");
    }
    let Ok(status_id) = tail[0].parse::<i64>() else {
        return fail(StatusCode::NOT_FOUND, "statut introuvable");
    };
    let id = status_id.to_string();
    let Some(item) = get_item(state, org, project, &key, &id) else {
        return fail(StatusCode::NOT_FOUND, "statut introuvable");
    };
    if method == Method::GET {
        return Json(dress_pull_status(org, project, &repo_id, pull_id, item)).into_response();
    }
    if method == Method::DELETE {
        return if delete_item(state, org, project, &key, &id) {
            StatusCode::NO_CONTENT.into_response()
        } else {
            fail(StatusCode::NOT_FOUND, "statut introuvable")
        };
    }
    fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée")
}

fn pull_status_create(body: &[u8]) -> Result<Value, &'static str> {
    let Some(payload) = parse_json(body) else {
        return Err("JSON illisible");
    };
    let Some(object) = payload.as_object() else {
        return Err("état absent");
    };
    let state_name = match object.get("state").and_then(|value| value.as_str()).map(str::trim) {
        Some(text) if !text.is_empty() => git_status_state(text).ok_or("état inconnu")?,
        _ => return Err("état absent"),
    };
    if let Some(description) = object.get("description") {
        if !description.is_string() {
            return Err("description absente");
        }
    }
    let Some(context) = object.get("context").and_then(|value| value.as_object()) else {
        return Err("contexte absent");
    };
    let name = context.get("name").and_then(|value| value.as_str()).map(str::trim).unwrap_or("");
    if name.is_empty() || context.get("name").is_some_and(|value| !value.is_string()) {
        return Err("contexte absent");
    }
    let genre = match context.get("genre") {
        None => "",
        Some(value) => value.as_str().map(str::trim).ok_or("contexte absent")?,
    };
    let description = object.get("description").and_then(|value| value.as_str()).unwrap_or("").trim();
    Ok(json!({
        "state": state_name,
        "description": description,
        "context": {"name": name, "genre": genre},
        "creationDate": "2026-10-29T00:00:00Z",
        "updatedDate": "2026-10-29T00:00:00Z",
        "createdBy": {"displayName": "azurz", "uniqueName": "azurz"}
    }))
}

fn dress_pull_status(org: &str, project: &str, repo_id: &str, pull_id: &str, mut item: Value) -> Value {
    let id = value_key(&item["id"]);
    if let Some(object) = item.as_object_mut() {
        object.insert(
            "url".into(),
            json!(format!("/{org}/{project}/_apis/git/repositories/{repo_id}/pullrequests/{pull_id}/statuses/{id}")),
        );
    }
    item
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
        return reviewers(state, method, org, project, pull_id, tail, body);
    }
    if kind == "iterations" {
        return pull_iterations(state, method, org, project, pull_id, tail);
    }
    if kind == "labels" {
        return pull_labels(state, method, org, project, pull_id, tail, body);
    }
    if kind == "threads" {
        return pull_threads(state, method, org, project, pull_id, tail, body);
    }
    if kind == "statuses" {
        return pull_statuses(state, method, org, project, pull_id, tail, body);
    }
    if method == Method::GET {
        return Json(page(list(state, org, project, &key))).into_response();
    }
    if method != Method::POST {
        return fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée");
    }
    let payload = parse_json(body).unwrap_or(json!({}));
    let next = list(state, org, project, &key).len() as i64 + 1;
    let item = json!({
        "id": next,
        "state": payload.get("state").cloned().unwrap_or(json!("succeeded")),
        "description": text_of(&payload, "description").unwrap_or_default(),
        "context": payload.get("context").cloned().unwrap_or(json!({"name": "azurz", "genre": "continuous-integration"})),
        "creationDate": "2026-10-29T00:00:00Z",
        "updatedDate": "2026-10-29T00:00:00Z"
    });
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

fn reviewer_matches(item: &Value, id: &str) -> bool {
    text_of(item, "id").is_some_and(|value| value.eq_ignore_ascii_case(id))
        || text_of(item, "displayName").is_some_and(|value| value.eq_ignore_ascii_case(id))
        || text_of(item, "uniqueName").is_some_and(|value| value.eq_ignore_ascii_case(id))
}

fn reviewers(state: &AppState, method: &Method, org: &str, project: &str, pull_id: &str, tail: &[String], body: &[u8]) -> Response {
    let key = format!("git/pullrequests/{pull_id}/reviewers");
    if method == Method::GET {
        return Json(Value::Array(list(state, org, project, &key))).into_response();
    }
    if method == Method::DELETE {
        if get_item(state, org, project, "git/pullrequests", pull_id).is_none() {
            return fail(StatusCode::NOT_FOUND, "demande de tirage introuvable");
        }
        if tail.is_empty() {
            return fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée");
        }
        if tail.len() != 1 {
            return fail(StatusCode::NOT_FOUND, "relecteur introuvable");
        }
        let Some(found) = list(state, org, project, &key).into_iter().find(|item| reviewer_matches(item, &tail[0])) else {
            return fail(StatusCode::NOT_FOUND, "relecteur introuvable");
        };
        let stored_id = text_of(&found, "id").unwrap_or_default();
        state.write(|db| {
            if let Some(stored) = db.project_mut(org, project) {
                if let Some(bucket) = stored.buckets.get_mut(&key) {
                    bucket.remove(&stored_id);
                }
            }
        });
        if try_autocomplete(state, org, project, pull_id).is_none() {
            if let Some(pull) = get_item(state, org, project, "git/pullrequests", pull_id) {
                if text_of(&pull, "status").as_deref() != Some("completed") {
                    deliver_work_item(state, org, project, "git.pullrequest.updated", &pull);
                }
            }
        }
        return StatusCode::NO_CONTENT.into_response();
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
    if try_autocomplete(state, org, project, pull_id).is_none() {
        if let Some(pull) = get_item(state, org, project, "git/pullrequests", pull_id) {
            if text_of(&pull, "status").as_deref() != Some("completed") {
                deliver_work_item(state, org, project, "git.pullrequest.updated", &pull);
            }
        }
    }
    if stored.len() == 1 {
        Json(stored.pop().unwrap_or(json!({}))).into_response()
    } else {
        Json(Value::Array(stored)).into_response()
    }
}

fn normalize_autocomplete(payload: &mut Value) {
    let Some(value) = payload.get("autoCompleteSetBy").cloned() else {
        return;
    };
    if value.is_null() {
        return;
    }
    let id = text_of(&value, "id")
        .or_else(|| text_of(&value, "uniqueName"))
        .or_else(|| text_of(&value, "displayName"))
        .unwrap_or_else(|| "azurz".into());
    let display = text_of(&value, "displayName").unwrap_or_else(|| id.clone());
    let unique = text_of(&value, "uniqueName").unwrap_or_else(|| id.clone());
    payload["autoCompleteSetBy"] = json!({"id": id, "displayName": display, "uniqueName": unique});
}

fn autocomplete_armed(pull: &Value) -> bool {
    pull.get("autoCompleteSetBy").is_some_and(|value| !value.is_null())
}

fn pull_blocked(state: &AppState, org: &str, project: &str, pull: &Value) -> bool {
    let pull_id = value_key(&pull["pullRequestId"]);
    let pull_key = if pull_id.is_empty() { value_key(&pull["id"]) } else { pull_id };
    let reviewers = list(state, org, project, &format!("git/pullrequests/{pull_key}/reviewers"));
    if reviewers.iter().any(|reviewer| reviewer.get("vote").and_then(|value| value.as_i64()).unwrap_or(0) < 0) {
        return true;
    }
    if reviewers.iter().any(|reviewer| {
        reviewer.get("isRequired").and_then(|value| value.as_bool()) == Some(true)
            && reviewer.get("vote").and_then(|value| value.as_i64()).unwrap_or(0) < 10
    }) {
        return true;
    }
    let repo_id = text_of(&pull["repository"], "id").unwrap_or_default();
    let target = text_of(pull, "targetRefName").unwrap_or_else(|| "refs/heads/main".into());
    list(state, org, project, "policy/configurations").into_iter().any(|item| {
        let configuration = dress_policy(org, project, item);
        if configuration.get("isEnabled").and_then(|value| value.as_bool()) == Some(false) {
            return false;
        }
        if configuration.get("isBlocking").and_then(|value| value.as_bool()) == Some(false) {
            return false;
        }
        let settings = configuration.get("settings").unwrap_or(&Value::Null);
        if !scope_matches(settings, &repo_id, &target) {
            return false;
        }
        let status = evaluate_policy(state, org, project, &configuration, pull);
        status == "rejected" || status == "queued"
    })
}

fn try_autocomplete(state: &AppState, org: &str, project: &str, pull_id: &str) -> Option<Value> {
    let pull = get_item(state, org, project, "git/pullrequests", pull_id)?;
    let status = text_of(&pull, "status").unwrap_or_default();
    if status == "completed" || status == "abandoned" || !autocomplete_armed(&pull) || pull_blocked(state, org, project, &pull) {
        return None;
    }
    let patched = patch_item(state, org, project, "git/pullrequests", pull_id, &json!({"status": "completed"}))?;
    let prepared = if squash_requested(&patched) {
        Some(Prepared::Squash(prepare_squash(state, org, project, &patched)))
    } else if rebase_requested(&patched) {
        Some(Prepared::Rebase(prepare_rebase(state, org, project, &patched)))
    } else {
        Some(Prepared::Merge(prepare_merge(state, org, project, &patched)))
    };
    let item = finish_pull(state, org, project, pull_id, prepared).unwrap_or(patched);
    deliver_work_item(state, org, project, "git.pullrequest.merged", &item);
    if let Some(repo) = text_of(&item["repository"], "id") {
        if let Some(target) = text_of(&item, "targetRefName") {
            advance_pull_iterations(state, org, project, &repo, &[target]);
        }
    }
    Some(item)
}

enum Prepared {
    Squash(SquashPrep),
    Merge(SquashPrep),
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

fn prepare_merge(state: &AppState, org: &str, project: &str, pull: &Value) -> SquashPrep {
    let mut prep = prepare_squash(state, org, project, pull);
    let explicit = text_of(&pull["completionOptions"], "mergeCommitMessage").filter(|text| !text.is_empty());
    if explicit.is_none() {
        let source = text_of(pull, "sourceRefName").unwrap_or_else(|| "refs/heads/feature".into());
        let target = text_of(pull, "targetRefName").unwrap_or_else(|| "refs/heads/main".into());
        prep.message = format!("Fusion de {source} dans {target}");
    }
    prep
}

fn install_tree(
    stored: &mut crate::store::ProjectData,
    items_key: &str,
    blob_key: &str,
    target_blobs: &[Value],
    changes: &[Value],
) {
    let mut tree: BTreeMap<String, Value> = BTreeMap::new();
    for blob in target_blobs {
        let path = text_of(blob, "path").or_else(|| text_of(blob, "id")).unwrap_or_default();
        if !path.is_empty() {
            tree.insert(path, blob.clone());
        }
    }
    for change in changes {
        let path = change
            .get("item")
            .and_then(|item| text_of(item, "path"))
            .unwrap_or_else(|| value_key(&change["id"]));
        if path.is_empty() {
            continue;
        }
        let kind = text_of(change, "changeType").unwrap_or_else(|| "add".into());
        if kind.eq_ignore_ascii_case("delete") {
            tree.remove(&path);
            continue;
        }
        let content = change
            .get("newContent")
            .and_then(|value| text_of(value, "content"))
            .unwrap_or_default();
        tree.insert(
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
    if let Some(bucket) = stored.buckets.get_mut(items_key) {
        bucket.items.clear();
    }
    let paths: Vec<String> = tree.keys().cloned().collect();
    for blob in tree.values() {
        stored.put_bucket(items_key, blob.clone());
        stored.put_bucket(blob_key, blob.clone());
    }
    for path in &paths {
        ensure_tree(stored, items_key, path);
        ensure_tree(stored, blob_key, path);
    }
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
        let (squash, delete_source, comment, changes, parents, commit_id, merged_tree) = match prepared {
            Some(Prepared::Squash(prep)) => {
                let parents = parent.clone().map(|id| vec![json!(id)]).unwrap_or_default();
                let commit_id = object_id(&format!(
                    "squash:{pull_id}:{}:{}:{}",
                    source_id.clone().unwrap_or_default(),
                    parent.clone().unwrap_or_default(),
                    prep.message
                ));
                (true, prep.delete_source, prep.message, prep.changes, parents, commit_id, false)
            }
            Some(Prepared::Merge(prep)) => {
                let mut parents = Vec::new();
                if let Some(id) = parent.clone() {
                    parents.push(json!(id));
                }
                if let Some(id) = source_id.clone() {
                    parents.push(json!(id));
                }
                let commit_id = object_id(&format!(
                    "merge:{pull_id}:{}:{}:{}",
                    parent.clone().unwrap_or_default(),
                    source_id.clone().unwrap_or_default(),
                    prep.message
                ));
                (false, prep.delete_source, prep.message, prep.changes, parents, commit_id, true)
            }
            _ => {
                let parents = parent.clone().map(|id| vec![json!(id)]).unwrap_or_default();
                let commit_id = object_id(&format!("merge:{pull_id}:{}", parent.clone().unwrap_or_default()));
                (
                    false,
                    false,
                    format!("Fusion de {source} dans {target}"),
                    Vec::new(),
                    parents,
                    commit_id,
                    false,
                )
            }
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
        let items_key = format!("git/repositories/{repo_id}/items");
        let blob_key = format!("{commits_key}/{commit_id}/blobs");
        if merged_tree {
            let target_blobs = parent
                .as_ref()
                .map(|id| stored.list_bucket(&format!("{commits_key}/{id}/blobs")))
                .unwrap_or_default();
            install_tree(stored, &items_key, &blob_key, &target_blobs, &changes);
        } else {
            let snapshot = stored.list_bucket(&items_key);
            for blob in snapshot {
                stored.put_bucket(&blob_key, blob);
            }
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

fn definitions(state: &AppState, method: &Method, org: &str, project: &str, rest: &[String], body: &[u8]) -> Response {
    let key = "build/definitions";
    if rest.len() == 2 && method == Method::GET {
        let value = list(state, org, project, key)
            .into_iter()
            .map(|item| dress_definition(project, item))
            .collect();
        return Json(page(value)).into_response();
    }
    if rest.len() == 2 && method == Method::POST {
        let payload = parse_json(body).unwrap_or(json!({}));
        let name = text_of(&payload, "name").unwrap_or_else(|| "CI".into());
        let groups = match definition_groups(state, org, project, payload.get("variableGroups")) {
            Ok(groups) => groups,
            Err(message) => return fail(StatusCode::BAD_REQUEST, message),
        };
        let variables = match definition_variables(payload.get("variables").unwrap_or(&json!({})), None) {
            Ok(variables) => variables,
            Err(message) => return fail(StatusCode::BAD_REQUEST, message),
        };
        let triggers = match definition_triggers(payload.get("triggers")) {
            Ok(triggers) => triggers,
            Err(message) => return fail(StatusCode::BAD_REQUEST, message),
        };
        let queue_status = match definition_queue_status(payload.get("queueStatus"), "enabled") {
            Ok(status) => status,
            Err(message) => return fail(StatusCode::BAD_REQUEST, message),
        };
        let next = list(state, org, project, key)
            .iter()
            .filter_map(|item| item.get("id").and_then(|value| value.as_i64()))
            .max()
            .unwrap_or(0)
            + 1;
        let item = fill_definition(
            project,
            json!({"id": next, "name": name, "path": "\\", "type": "build", "queueStatus": queue_status, "variableGroups": groups, "variables": variables, "triggers": triggers}),
        );
        put_item(state, org, project, key, item.clone());
        return Json(dress_definition(project, item)).into_response();
    }
    if rest.len() == 2 && method == Method::DELETE {
        return fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée");
    }
    let id = rest.get(2).map(String::as_str).unwrap_or("").to_string();
    if id.is_empty() {
        return fail(StatusCode::NOT_FOUND, "définition introuvable");
    }
    if method == Method::GET {
        return match get_item(state, org, project, key, &id) {
            Some(item) => Json(dress_definition(project, item)).into_response(),
            None => fail(StatusCode::NOT_FOUND, "définition introuvable"),
        };
    }
    if method == Method::PUT || method == Method::PATCH {
        let Some(existing) = get_item(state, org, project, key, &id) else {
            return fail(StatusCode::NOT_FOUND, "définition introuvable");
        };
        let payload = parse_json(body).unwrap_or(json!({}));
        let name = text_of(&payload, "name").unwrap_or_else(|| text_of(&existing, "name").unwrap_or_else(|| "CI".into()));
        let groups = if payload.get("variableGroups").is_some() {
            match definition_groups(state, org, project, payload.get("variableGroups")) {
                Ok(groups) => groups,
                Err(message) => return fail(StatusCode::BAD_REQUEST, message),
            }
        } else {
            existing.get("variableGroups").cloned().unwrap_or(json!([])).as_array().cloned().unwrap_or_default()
        };
        let variables = if let Some(incoming) = payload.get("variables") {
            match definition_variables(incoming, existing.get("variables")) {
                Ok(variables) => variables,
                Err(message) => return fail(StatusCode::BAD_REQUEST, message),
            }
        } else {
            existing.get("variables").cloned().unwrap_or(json!({}))
        };
        let triggers = if payload.get("triggers").is_some() {
            match definition_triggers(payload.get("triggers")) {
                Ok(triggers) => triggers,
                Err(message) => return fail(StatusCode::BAD_REQUEST, message),
            }
        } else {
            existing.get("triggers").and_then(|value| value.as_array().cloned()).unwrap_or_default()
        };
        let queue_status = match definition_queue_status(
            payload.get("queueStatus"),
            text_of(&existing, "queueStatus").unwrap_or_else(|| "enabled".into()).as_str(),
        ) {
            Ok(status) => status,
            Err(message) => return fail(StatusCode::BAD_REQUEST, message),
        };
        let mut item = existing;
        item["name"] = json!(name);
        item["variableGroups"] = json!(groups);
        item["variables"] = variables;
        item["triggers"] = json!(triggers);
        item["queueStatus"] = json!(queue_status);
        let item = fill_definition(project, item);
        put_item(state, org, project, key, item.clone());
        return Json(dress_definition(project, item)).into_response();
    }
    if method == Method::DELETE {
        if rest.len() != 3 {
            return fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée");
        }
        if get_item(state, org, project, key, &id).is_none() {
            return fail(StatusCode::NOT_FOUND, "définition introuvable");
        }
        state.write(|db| {
            if let Some(stored) = db.project_mut(org, project) {
                if let Some(bucket) = stored.buckets.get_mut(key) {
                    bucket.remove(&id);
                }
            }
        });
        return StatusCode::NO_CONTENT.into_response();
    }
    fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée")
}

fn definition_queue_status(incoming: Option<&Value>, previous: &str) -> Result<String, &'static str> {
    let Some(incoming) = incoming.and_then(|value| value.as_str()) else {
        return Ok(canonical_queue_status(previous).unwrap_or("enabled").into());
    };
    canonical_queue_status(incoming)
        .map(str::to_string)
        .ok_or("état de file inconnu")
}

fn canonical_queue_status(name: &str) -> Option<&'static str> {
    if name.eq_ignore_ascii_case("enabled") {
        Some("enabled")
    } else if name.eq_ignore_ascii_case("paused") {
        Some("paused")
    } else if name.eq_ignore_ascii_case("disabled") {
        Some("disabled")
    } else {
        None
    }
}

fn definition_triggers(incoming: Option<&Value>) -> Result<Vec<Value>, &'static str> {
    let Some(incoming) = incoming else {
        return Ok(Vec::new());
    };
    let Some(rows) = incoming.as_array() else {
        return Err("déclencheur illisible");
    };
    let mut stored = Vec::new();
    for row in rows {
        let Some(kind) = text_of(row, "triggerType").or_else(|| text_of(row, "type")) else {
            return Err("déclencheur inconnu");
        };
        let Some(kind) = canonical_trigger(&kind) else {
            return Err("déclencheur inconnu");
        };
        let branch_filters = filter_list(row.get("branchFilters"), "filtre de branche absent", "filtre de branche illisible")?;
        let path_filters = filter_list(row.get("pathFilters"), "filtre de chemin absent", "filtre de chemin illisible")?;
        let mut trigger = json!({
            "triggerType": kind,
            "branchFilters": branch_filters,
            "pathFilters": path_filters,
            "batchChanges": row.get("batchChanges").and_then(|value| value.as_bool()).unwrap_or(false)
        });
        if kind == "schedule" {
            trigger["schedules"] = json!(schedule_entries(row)?);
        }
        stored.push(trigger);
    }
    Ok(stored)
}

fn canonical_trigger(name: &str) -> Option<&'static str> {
    if name.eq_ignore_ascii_case("continuousIntegration") {
        Some("continuousIntegration")
    } else if name.eq_ignore_ascii_case("pullRequest") {
        Some("pullRequest")
    } else if name.eq_ignore_ascii_case("schedule") {
        Some("schedule")
    } else {
        None
    }
}

fn canonical_day(name: &str) -> Option<&'static str> {
    match name.to_ascii_lowercase().as_str() {
        "monday" => Some("monday"),
        "tuesday" => Some("tuesday"),
        "wednesday" => Some("wednesday"),
        "thursday" => Some("thursday"),
        "friday" => Some("friday"),
        "saturday" => Some("saturday"),
        "sunday" => Some("sunday"),
        _ => None,
    }
}

fn schedule_entries(row: &Value) -> Result<Vec<Value>, &'static str> {
    let Some(raw) = row.get("schedules") else {
        return Err("horaire absent");
    };
    let Some(rows) = raw.as_array() else {
        return Err("horaire illisible");
    };
    if rows.is_empty() {
        return Err("horaire absent");
    }
    let mut stored = Vec::new();
    for entry in rows {
        let Some(entry) = entry.as_object() else {
            return Err("horaire illisible");
        };
        let Some(days) = entry.get("daysToBuild") else {
            return Err("jour absent");
        };
        let Some(days) = days.as_array() else {
            return Err("horaire illisible");
        };
        if days.is_empty() {
            return Err("jour absent");
        }
        let mut canonical_days = Vec::new();
        for day in days {
            let Some(text) = day.as_str() else {
                return Err("horaire illisible");
            };
            let text = text.trim();
            if text.is_empty() {
                return Err("jour absent");
            }
            let Some(day) = canonical_day(text) else {
                return Err("jour inconnu");
            };
            canonical_days.push(day);
        }
        let hours = match entry.get("startHours") {
            None => 0,
            Some(value) => match value.as_i64() {
                Some(hour) if (0..24).contains(&hour) => hour,
                _ => return Err("heure inconnue"),
            },
        };
        let minutes = match entry.get("startMinutes") {
            None => 0,
            Some(value) => match value.as_i64() {
                Some(minute) if (0..60).contains(&minute) => minute,
                _ => return Err("minute inconnue"),
            },
        };
        let branch_filters = filter_list(
            entry.get("branchFilters"),
            "filtre de branche absent",
            "filtre de branche illisible",
        )?;
        let only_changes = entry
            .get("scheduleOnlyWithChanges")
            .and_then(|value| value.as_bool())
            .unwrap_or(false);
        stored.push(json!({
            "branchFilters": branch_filters,
            "daysToBuild": canonical_days,
            "startHours": hours,
            "startMinutes": minutes,
            "scheduleOnlyWithChanges": only_changes,
            "timeZoneId": entry.get("timeZoneId").and_then(|value| value.as_str()).filter(|value| !value.is_empty()).unwrap_or("UTC")
        }));
    }
    Ok(stored)
}

fn evaluate_schedules(state: &AppState, org: &str, project: &str) {
    let definitions = list(state, org, project, "build/definitions");
    for definition in definitions {
        let status = text_of(&definition, "queueStatus").unwrap_or_else(|| "enabled".into());
        if status.eq_ignore_ascii_case("disabled") || status.eq_ignore_ascii_case("paused") {
            continue;
        }
        let Some(triggers) = definition.get("triggers").and_then(|value| value.as_array()) else {
            continue;
        };
        let definition_id = definition.get("id").and_then(|value| value.as_i64()).unwrap_or(0);
        for trigger in triggers {
            if text_of(trigger, "triggerType").as_deref() != Some("schedule") {
                continue;
            }
            let Some(schedules) = trigger.get("schedules").and_then(|value| value.as_array()) else {
                continue;
            };
            for schedule in schedules {
                let days = schedule.get("daysToBuild").and_then(|value| value.as_array());
                let thursday = days.is_some_and(|rows| rows.iter().any(|day| day.as_str() == Some("thursday")));
                let hours = schedule.get("startHours").and_then(|value| value.as_i64()).unwrap_or(0);
                let minutes = schedule.get("startMinutes").and_then(|value| value.as_i64()).unwrap_or(0);
                if !thursday || hours != 0 || minutes != 0 {
                    continue;
                }
                let branch = "refs/heads/main";
                if !filter_selected(schedule.get("branchFilters"), branch) {
                    continue;
                }
                let slot = format!("{definition_id}|{branch}|thursday|0|0");
                if get_item(state, org, project, "build/schedule-slots", &slot).is_some() {
                    continue;
                }
                if enqueue_build(state, org, project, Some(&definition), definition_id, branch, "schedule", &json!({})).is_err() {
                    continue;
                }
                put_item(state, org, project, "build/schedule-slots", json!({"id": slot}));
            }
        }
    }
}

fn filter_list(value: Option<&Value>, blank: &'static str, illisible: &'static str) -> Result<Vec<String>, &'static str> {
    let Some(value) = value else {
        return Ok(Vec::new());
    };
    let Some(rows) = value.as_array() else {
        return Err(illisible);
    };
    let mut filters = Vec::new();
    for row in rows {
        let text = row.as_str().unwrap_or("").trim();
        if text.is_empty() {
            return Err(blank);
        }
        filters.push(text.to_string());
    }
    Ok(filters)
}

fn definition_groups(state: &AppState, org: &str, project: &str, value: Option<&Value>) -> Result<Vec<Value>, &'static str> {
    let Some(entries) = value else {
        return Ok(Vec::new());
    };
    let Some(entries) = entries.as_array() else {
        return Err("groupe introuvable");
    };
    let mut links = Vec::new();
    for entry in entries {
        let id = entry.as_i64().map(|number| number.to_string()).unwrap_or_else(|| value_key(&entry["id"]));
        if id.is_empty() {
            return Err("groupe introuvable");
        }
        let Some(group) = get_item(state, org, project, "distributedtask/variablegroups", &id) else {
            return Err("groupe introuvable");
        };
        links.push(json!({"id": group.get("id").cloned().unwrap_or(json!(id)), "name": text_of(&group, "name").unwrap_or_default()}));
    }
    Ok(links)
}

fn definition_variables(incoming: &Value, previous: Option<&Value>) -> Result<Value, &'static str> {
    let Some(object) = incoming.as_object() else {
        return Ok(previous.cloned().unwrap_or(json!({})));
    };
    let mut stored = Map::new();
    for (key, spec) in object {
        let name = key.trim();
        if name.is_empty() {
            return Err("nom de variable absent");
        }
        let is_secret = spec.get("isSecret").and_then(|value| value.as_bool()).unwrap_or(false);
        let allow_override = spec.get("allowOverride").and_then(|value| value.as_bool()).unwrap_or(true);
        let incoming_value = spec.get("value").and_then(|value| value.as_str()).unwrap_or("");
        let previous_spec = previous.and_then(|variables| variables.get(name).or_else(|| variables.get(key)));
        let previous_value = previous_spec.and_then(|item| item.get("value")).and_then(|value| value.as_str()).unwrap_or("");
        let previous_secret = previous_spec.and_then(|item| item.get("isSecret")).and_then(|value| value.as_bool()).unwrap_or(false);
        let value = if incoming_value.is_empty() && previous_secret {
            previous_value
        } else {
            incoming_value
        };
        stored.insert(
            name.to_string(),
            json!({"value": value, "isSecret": is_secret, "allowOverride": allow_override}),
        );
    }
    Ok(Value::Object(stored))
}

fn shown_variable(spec: &Value) -> Value {
    let is_secret = spec.get("isSecret").and_then(|value| value.as_bool()).unwrap_or(false);
    let shown = if is_secret {
        ""
    } else {
        spec.get("value").and_then(|value| value.as_str()).unwrap_or("")
    };
    json!({"value": shown, "isSecret": is_secret})
}

fn queue_variables(state: &AppState, org: &str, project: &str, definition: &Value) -> Result<Value, &'static str> {
    let mut variables = Map::new();
    if let Some(links) = definition.get("variableGroups").and_then(|value| value.as_array()) {
        for link in links {
            let id = value_key(&link["id"]);
            let Some(group) = get_item(state, org, project, "distributedtask/variablegroups", &id) else {
                return Err("groupe introuvable");
            };
            let Some(entries) = group.get("variables").and_then(|value| value.as_object()) else {
                continue;
            };
            for (key, spec) in entries {
                variables.insert(key.clone(), shown_variable(spec));
            }
        }
    }
    if let Some(entries) = definition.get("variables").and_then(|value| value.as_object()) {
        for (key, spec) in entries {
            variables.insert(key.clone(), shown_variable(spec));
        }
    }
    Ok(Value::Object(variables))
}

fn enqueue_build(
    state: &AppState,
    org: &str,
    project: &str,
    definition: Option<&Value>,
    definition_id: i64,
    source_branch: &str,
    reason: &str,
    payload: &Value,
) -> Result<Value, &'static str> {
    let next = list(state, org, project, "build/builds").len() as i64 + 1;
    let definition_name = definition
        .and_then(|item| text_of(item, "name"))
        .unwrap_or_else(|| "CI".into());
    let (mut variables, groups) = match definition {
        Some(definition) => (
            queue_variables(state, org, project, definition)?,
            definition.get("variableGroups").cloned().unwrap_or(json!([])),
        ),
        None => (json!({}), json!([])),
    };
    let parameters = match variables.as_object_mut() {
        Some(entries) => apply_queue_parameters(entries, definition.unwrap_or(&json!({})), payload)?,
        None => String::new(),
    };
    let queue = list(state, org, project, "distributedtask/queues")
        .into_iter()
        .next()
        .unwrap_or_else(|| json!({"id": 1, "name": "Default", "pool": {"id": 1, "name": "Azure Pipelines"}}));
    let item = json!({
        "id": next,
        "buildNumber": format!("20261029.{next}"),
        "status": "notStarted",
        "reason": reason,
        "definition": {"id": definition_id, "name": definition_name},
        "project": {"name": project},
        "sourceBranch": source_branch,
        "sourceVersion": object_id(&format!("build-{next}")),
        "queue": queue,
        "variables": variables,
        "variableGroups": groups,
        "parameters": parameters,
        "queueTime": "2026-10-29T00:00:00Z",
        "url": format!("/{org}/{project}/_apis/build/builds/{next}"),
        "requestedBy": {"displayName": "azurz"}
    });
    put_item(state, org, project, "build/builds", item.clone());
    Ok(item)
}

fn queue_push_triggers(state: &AppState, org: &str, project: &str, repo_id: &str, push: &Value) {
    let paths = commit_paths(state, org, project, repo_id, push);
    let refs = push
        .get("refUpdates")
        .and_then(|value| value.as_array())
        .map(|rows| rows.iter().filter_map(|row| text_of(row, "name")).collect::<Vec<_>>())
        .filter(|names| !names.is_empty())
        .or_else(|| text_of(push, "refName").map(|name| vec![name]))
        .unwrap_or_default();
    for branch in refs {
        queue_matching_triggers(state, org, project, "continuousIntegration", &branch, &branch, &paths);
    }
}

fn queue_matching_triggers(
    state: &AppState,
    org: &str,
    project: &str,
    trigger_type: &str,
    filter_branch: &str,
    build_branch: &str,
    paths: &[String],
) {
    let definitions = list(state, org, project, "build/definitions");
    for definition in definitions {
        let status = text_of(&definition, "queueStatus").unwrap_or_else(|| "enabled".into());
        if status.eq_ignore_ascii_case("disabled") || status.eq_ignore_ascii_case("paused") {
            continue;
        }
        let Some(triggers) = definition.get("triggers").and_then(|value| value.as_array()) else {
            continue;
        };
        let Some(trigger) = triggers.iter().find(|trigger| {
            text_of(trigger, "triggerType").as_deref() == Some(trigger_type)
                && filter_selected(trigger.get("branchFilters"), filter_branch)
                && paths_selected(trigger.get("pathFilters"), paths)
        }) else {
            continue;
        };
        let definition_id = definition.get("id").and_then(|value| value.as_i64()).unwrap_or(0);
        if trigger.get("batchChanges").and_then(|value| value.as_bool()) == Some(true)
            && build_already_open(state, org, project, definition_id, build_branch)
        {
            continue;
        }
        let reason = if trigger_type == "pullRequest" { "pullRequest" } else { "individualCI" };
        let _ = enqueue_build(
            state,
            org,
            project,
            Some(&definition),
            definition_id,
            build_branch,
            reason,
            &json!({}),
        );
    }
}

fn build_already_open(state: &AppState, org: &str, project: &str, definition_id: i64, branch: &str) -> bool {
    list(state, org, project, "build/builds").iter().any(|build| {
        let status = text_of(build, "status").unwrap_or_default();
        let open = status.eq_ignore_ascii_case("notStarted") || status.eq_ignore_ascii_case("inProgress");
        open
            && build.get("definition").and_then(|value| value.get("id")).and_then(|value| value.as_i64()) == Some(definition_id)
            && text_of(build, "sourceBranch").as_deref() == Some(branch)
    })
}

fn commit_paths(state: &AppState, org: &str, project: &str, repo_id: &str, push: &Value) -> Vec<String> {
    let Some(commits) = push.get("commits").and_then(|value| value.as_array()) else {
        return Vec::new();
    };
    let mut paths = Vec::new();
    for commit in commits {
        let id = text_of(commit, "commitId").or_else(|| text_of(commit, "id")).unwrap_or_default();
        if id.is_empty() {
            continue;
        }
        for change in list(state, org, project, &format!("git/repositories/{repo_id}/commits/{id}/changes")) {
            if let Some(path) = change
                .get("item")
                .and_then(|item| text_of(item, "path"))
                .or_else(|| text_of(&change, "id"))
            {
                paths.push(path);
            }
        }
    }
    paths
}

fn pull_change_paths(state: &AppState, org: &str, project: &str, pull: &Value) -> Vec<String> {
    let repo_id = text_of(&pull["repository"], "id").unwrap_or_default();
    let tips = pull_tips(state, org, project, pull);
    let mut paths = Vec::new();
    for commit in tips.ahead {
        let id = text_of(&commit, "commitId").or_else(|| text_of(&commit, "id")).unwrap_or_default();
        if id.is_empty() {
            continue;
        }
        for change in list(state, org, project, &format!("git/repositories/{repo_id}/commits/{id}/changes")) {
            if let Some(path) = change.get("item").and_then(|item| text_of(item, "path")) {
                paths.push(path);
            }
        }
    }
    paths
}

fn paths_selected(filters: Option<&Value>, paths: &[String]) -> bool {
    let Some(rows) = filters.and_then(|value| value.as_array()) else {
        return true;
    };
    if rows.is_empty() {
        return true;
    }
    paths.iter().any(|path| filter_selected(filters, path))
}

fn filter_selected(filters: Option<&Value>, value: &str) -> bool {
    let Some(rows) = filters.and_then(|value| value.as_array()) else {
        return true;
    };
    if rows.is_empty() {
        return true;
    }
    let mut included = false;
    let mut has_include = false;
    let mut excluded = false;
    for row in rows {
        let text = row.as_str().unwrap_or("").trim();
        if text.is_empty() {
            continue;
        }
        let (exclude, pattern) = if let Some(rest) = text.strip_prefix('-') {
            (true, rest)
        } else if let Some(rest) = text.strip_prefix('+') {
            (false, rest)
        } else {
            (false, text)
        };
        if pattern.is_empty() || !glob_match(pattern, value) {
            continue;
        }
        if exclude {
            excluded = true;
        } else {
            has_include = true;
            included = true;
        }
    }
    if !has_include {
        has_include = rows.iter().any(|row| {
            let text = row.as_str().unwrap_or("").trim();
            !text.is_empty() && !text.starts_with('-')
        });
    }
    if excluded {
        return false;
    }
    if !has_include {
        return true;
    }
    included
}

fn glob_match(pattern: &str, value: &str) -> bool {
    let parts: Vec<&str> = pattern.split('*').collect();
    if !value.starts_with(parts[0]) {
        return false;
    }
    let mut cursor = parts[0].len();
    if parts.len() == 1 {
        return cursor == value.len();
    }
    for (index, part) in parts.iter().enumerate().skip(1) {
        if index + 1 == parts.len() {
            if part.is_empty() {
                return true;
            }
            return value.len() >= cursor + part.len() && value[cursor..].ends_with(part);
        }
        if part.is_empty() {
            continue;
        }
        let Some(found) = value[cursor..].find(part) else {
            return false;
        };
        cursor += found + part.len();
    }
    true
}

fn parameter_text(value: &Value) -> String {
    value
        .as_str()
        .map(str::to_string)
        .or_else(|| value.get("value").and_then(|item| item.as_str()).map(str::to_string))
        .unwrap_or_default()
}

fn apply_queue_parameters(variables: &mut Map<String, Value>, definition: &Value, payload: &Value) -> Result<String, &'static str> {
    let Some(raw) = payload.get("parameters") else {
        return Ok(String::new());
    };
    let (text, entries) = match raw {
        Value::String(text) if text.trim().is_empty() => return Ok(String::new()),
        Value::String(text) => {
            let parsed: Value = serde_json::from_str(text).map_err(|_| "paramètres de file illisibles")?;
            let Some(entries) = parsed.as_object().cloned() else {
                return Err("paramètres de file illisibles");
            };
            (text.clone(), entries)
        }
        Value::Object(entries) => (
            serde_json::to_string(entries).unwrap_or_else(|_| "{}".into()),
            entries.clone(),
        ),
        _ => return Err("paramètres de file illisibles"),
    };
    let defined = definition.get("variables").and_then(|value| value.as_object());
    for (key, value) in entries {
        let locked = defined.and_then(|vars| vars.get(&key)).is_some_and(|spec| {
            spec.get("allowOverride").and_then(|flag| flag.as_bool()) == Some(false)
        });
        if locked {
            continue;
        }
        let secret = defined
            .and_then(|vars| vars.get(&key))
            .and_then(|spec| spec.get("isSecret"))
            .and_then(|flag| flag.as_bool())
            .unwrap_or(false);
        let shown = if secret { String::new() } else { parameter_text(&value) };
        variables.insert(key, json!({"value": shown, "isSecret": secret}));
    }
    Ok(text)
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
        evaluate_schedules(state, org, project);
        let filters = query_param(query, "tagFilters")
            .map(pct_decode)
            .map(|value| {
                value
                    .split(',')
                    .map(|part| part.trim().to_string())
                    .filter(|part| !part.is_empty())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let value = list(state, org, project, key)
            .into_iter()
            .map(|item| promote_build(state, org, project, item))
            .filter(|item| {
                filters.is_empty() || filters.iter().all(|wanted| tag_names(item).iter().any(|tag| tag == wanted))
            })
            .collect();
        return Json(page(value)).into_response();
    }
    if rest.len() == 2 && method == Method::POST {
        let payload = parse_json(body).unwrap_or(json!({}));
        let definition_id = payload
            .get("definition")
            .and_then(|value| value.get("id"))
            .and_then(|value| value.as_i64())
            .unwrap_or(1);
        let found = list(state, org, project, "build/definitions")
            .into_iter()
            .find(|item| item.get("id").and_then(|value| value.as_i64()) == Some(definition_id));
        let source = text_of(&payload, "sourceBranch").unwrap_or_else(|| "refs/heads/main".into());
        let reason = text_of(&payload, "reason").unwrap_or_else(|| "manual".into());
        return match enqueue_build(state, org, project, found.as_ref(), definition_id, &source, &reason, &payload) {
            Ok(item) => Json(item).into_response(),
            Err(message) => fail(StatusCode::BAD_REQUEST, message),
        };
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
    if rest.len() >= 4 && rest[3].eq_ignore_ascii_case("tags") {
        return build_tags(state, method, org, project, id, rest, body);
    }
    if method == Method::GET {
        return match get_item(state, org, project, key, id) {
            Some(item) => Json(promote_build(state, org, project, item)).into_response(),
            None => fail(StatusCode::NOT_FOUND, "build introuvable"),
        };
    }
    if method == Method::DELETE && rest.len() == 3 {
        let Some(found) = get_item(state, org, project, key, id) else {
            return fail(StatusCode::NOT_FOUND, "build introuvable");
        };
        let stored_id = value_key(&found["id"]);
        state.write(|db| {
            if let Some(stored) = db.project_mut(org, project) {
                if let Some(bucket) = stored.buckets.get_mut(key) {
                    bucket.remove(&stored_id);
                }
            }
        });
        return StatusCode::NO_CONTENT.into_response();
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
        let next = list(state, org, project, key)
            .into_iter()
            .filter_map(|item| item.get("id").and_then(|value| value.as_i64()))
            .max()
            .unwrap_or(0)
            + 1;
        let item = json!({"id": next, "name": name, "folder": "\\", "revision": 1});
        put_item(state, org, project, key, item.clone());
        return Json(item).into_response();
    }
    if method == Method::DELETE {
        if rest.len() != 3 {
            return fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée");
        }
        let Some(found) = get_item(state, org, project, key, &rest[2]) else {
            return fail(StatusCode::NOT_FOUND, "pipeline introuvable");
        };
        let stored_id = value_key(&found["id"]);
        state.write(|db| {
            if let Some(stored) = db.project_mut(org, project) {
                if let Some(bucket) = stored.buckets.get_mut(key) {
                    bucket.remove(&stored_id);
                }
            }
        });
        return StatusCode::NO_CONTENT.into_response();
    }
    if rest.len() >= 4 && rest[3].eq_ignore_ascii_case("runs") && method == Method::POST {
        let Some(pipeline) = get_item(state, org, project, key, &rest[2]) else {
            return fail(StatusCode::NOT_FOUND, "pipeline introuvable");
        };
        let pipeline_id = pipeline.get("id").and_then(|value| value.as_i64()).unwrap_or(1);
        let name = text_of(&pipeline, "name").unwrap_or_else(|| "CI".into());
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

fn environment_virtual_machines(
    state: &AppState,
    method: &Method,
    org: &str,
    project: &str,
    rest: &[String],
    body: &[u8],
) -> Response {
    let provider = rest.get(4).map(|value| value.as_str());
    if provider.is_none_or(|part| !part.eq_ignore_ascii_case("virtualmachines")) {
        return if method == Method::DELETE {
            fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée")
        } else {
            fail(StatusCode::NOT_FOUND, "machine introuvable")
        };
    }
    let machine_tail = if rest.len() > 5 { &rest[5..] } else { &[] };
    if machine_tail.len() > 1 {
        return if method == Method::DELETE {
            fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée")
        } else {
            fail(StatusCode::NOT_FOUND, "machine introuvable")
        };
    }
    let environment_key = pct_decode(rest.get(2).map(String::as_str).unwrap_or(""));
    let Some(environment) = resolve_environment_record(state, org, project, environment_key.trim()) else {
        return fail(StatusCode::NOT_FOUND, "environnement introuvable");
    };
    let environment_id = value_key(&environment["id"]);
    let environment_name = text_of(&environment, "name").unwrap_or_default();
    if machine_tail.is_empty() {
        if method == Method::GET {
            let mut rows = list_virtual_machines(state, org, project, &environment_id);
            rows.sort_by(|left, right| value_key(&left["id"]).cmp(&value_key(&right["id"])));
            let value = rows
                .into_iter()
                .map(|item| dress_virtual_machine(org, project, &environment_name, item))
                .collect();
            return Json(page(value)).into_response();
        }
        if method == Method::POST {
            let parsed = match virtual_machine_payload(body) {
                Ok(parsed) => parsed,
                Err(message) => return fail(StatusCode::BAD_REQUEST, message),
            };
            return match virtual_machine_create(state, org, project, &environment_id, &environment_name, parsed) {
                Ok(item) => Json(item).into_response(),
                Err(message) => fail(StatusCode::BAD_REQUEST, message),
            };
        }
        return fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée");
    }
    let key = pct_decode(&machine_tail[0]);
    if method == Method::GET {
        return match locate_virtual_machine(state, org, project, &environment_id, key.trim()) {
            Some(found) => Json(dress_virtual_machine(org, project, &environment_name, found)).into_response(),
            None => fail(StatusCode::NOT_FOUND, "machine introuvable"),
        };
    }
    if method == Method::DELETE {
        let Some(found) = locate_virtual_machine(state, org, project, &environment_id, key.trim()) else {
            return fail(StatusCode::NOT_FOUND, "machine introuvable");
        };
        let stored_id = value_key(&found["id"]);
        let removed = delete_item(state, org, project, "distributedtask/environments/virtualmachines", &stored_id);
        return if removed {
            StatusCode::NO_CONTENT.into_response()
        } else {
            fail(StatusCode::NOT_FOUND, "machine introuvable")
        };
    }
    fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée")
}

fn resolve_environment_record(state: &AppState, org: &str, project: &str, key: &str) -> Option<Value> {
    if key.is_empty() {
        return None;
    }
    if let Some(found) = get_item(state, org, project, "distributedtask/environments", key) {
        return Some(found);
    }
    list(state, org, project, "distributedtask/environments")
        .into_iter()
        .find(|item| text_of(item, "name").is_some_and(|name| name.eq_ignore_ascii_case(key)))
}

fn list_virtual_machines(state: &AppState, org: &str, project: &str, environment_id: &str) -> Vec<Value> {
    list(state, org, project, "distributedtask/environments/virtualmachines")
        .into_iter()
        .filter(|item| text_of(item, "environmentId").is_some_and(|stored| stored == environment_id))
        .collect()
}

fn locate_virtual_machine(state: &AppState, org: &str, project: &str, environment_id: &str, key: &str) -> Option<Value> {
    list_virtual_machines(state, org, project, environment_id)
        .into_iter()
        .find(|item| value_key(&item["id"]) == key || text_of(item, "name").is_some_and(|stored| stored.eq_ignore_ascii_case(key)))
}

fn next_virtual_machine_id(state: &AppState, org: &str) -> i64 {
    state
        .read(|db| {
            let Some(organization) = db.orgs.get(org) else {
                return Vec::new();
            };
            let mut rows = Vec::new();
            for project in organization.projects.values() {
                if let Some(bucket) = project.buckets.get("distributedtask/environments/virtualmachines") {
                    rows.extend(bucket.items.values().cloned());
                }
            }
            rows
        })
        .iter()
        .filter_map(|item| value_key(&item["id"]).strip_prefix("vm-")?.parse::<i64>().ok())
        .max()
        .unwrap_or(0)
        + 1
}

fn virtual_machine_create(
    state: &AppState,
    org: &str,
    project: &str,
    environment_id: &str,
    environment_name: &str,
    parsed: (String, Vec<String>),
) -> Result<Value, &'static str> {
    let (name, tags) = parsed;
    if list_virtual_machines(state, org, project, environment_id)
        .iter()
        .any(|item| text_of(item, "name").is_some_and(|stored| stored.eq_ignore_ascii_case(&name)))
    {
        return Err("machine existante");
    }
    let id = format!("vm-{}", next_virtual_machine_id(state, org));
    let item = json!({
        "id": id,
        "name": name,
        "type": "virtualMachine",
        "tags": tags,
        "environmentId": environment_id,
        "environmentName": environment_name
    });
    put_item(state, org, project, "distributedtask/environments/virtualmachines", item.clone());
    Ok(dress_virtual_machine(org, project, environment_name, item))
}

fn virtual_machine_payload(body: &[u8]) -> Result<(String, Vec<String>), &'static str> {
    let Some(payload) = parse_json(body) else {
        return Err("JSON illisible");
    };
    let Some(object) = payload.as_object() else {
        return Err("machine illisible");
    };
    let name = match object.get("name") {
        Some(Value::String(value)) => {
            let value = value.trim();
            if value.is_empty() {
                return Err("nom absent");
            }
            value.to_string()
        }
        Some(_) => return Err("nom absent"),
        None => return Err("nom absent"),
    };
    let tags = match object.get("tags") {
        None => Vec::new(),
        Some(Value::Array(items)) => {
            let mut tags = Vec::new();
            for item in items {
                let Some(value) = item.as_str().map(str::trim).filter(|value| !value.is_empty()) else {
                    return Err("étiquette absente");
                };
                tags.push(value.to_string());
            }
            tags
        }
        Some(_) => return Err("étiquette absente"),
    };
    Ok((name, tags))
}

fn dress_virtual_machine(org: &str, project: &str, environment_name: &str, mut item: Value) -> Value {
    let environment_id = text_of(&item, "environmentId").unwrap_or_default();
    let id = value_key(&item["id"]);
    if let Some(object) = item.as_object_mut() {
        object.insert("environmentName".into(), json!(environment_name));
        object.insert("project".into(), json!(project));
        object.insert(
            "url".into(),
            json!(format!("/{org}/{project}/_apis/distributedtask/environments/{environment_id}/providers/virtualmachines/{id}")),
        );
    }
    item
}

fn environment_kubernetes(
    state: &AppState,
    method: &Method,
    org: &str,
    project: &str,
    rest: &[String],
    body: &[u8],
) -> Response {
    let cluster_tail = if rest.len() > 5 { &rest[5..] } else { &[] };
    if cluster_tail.len() > 1 {
        return if method == Method::DELETE {
            fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée")
        } else {
            fail(StatusCode::NOT_FOUND, "cluster introuvable")
        };
    }
    let environment_key = pct_decode(rest.get(2).map(String::as_str).unwrap_or(""));
    let Some(environment) = resolve_environment_record(state, org, project, environment_key.trim()) else {
        return fail(StatusCode::NOT_FOUND, "environnement introuvable");
    };
    let environment_id = value_key(&environment["id"]);
    let environment_name = text_of(&environment, "name").unwrap_or_default();
    if cluster_tail.is_empty() {
        if method == Method::GET {
            let mut rows = list_kubernetes(state, org, project, &environment_id);
            rows.sort_by(|left, right| value_key(&left["id"]).cmp(&value_key(&right["id"])));
            let value = rows
                .into_iter()
                .map(|item| dress_kubernetes(org, project, &environment_name, item))
                .collect();
            return Json(page(value)).into_response();
        }
        if method == Method::POST {
            let parsed = match kubernetes_payload(body) {
                Ok(parsed) => parsed,
                Err(message) => return fail(StatusCode::BAD_REQUEST, message),
            };
            return match kubernetes_create(state, org, project, &environment_id, &environment_name, parsed) {
                Ok(item) => Json(item).into_response(),
                Err(message) => fail(StatusCode::BAD_REQUEST, message),
            };
        }
        return fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée");
    }
    let key = pct_decode(&cluster_tail[0]);
    if method == Method::GET {
        return match locate_kubernetes(state, org, project, &environment_id, key.trim()) {
            Some(found) => Json(dress_kubernetes(org, project, &environment_name, found)).into_response(),
            None => fail(StatusCode::NOT_FOUND, "cluster introuvable"),
        };
    }
    if method == Method::DELETE {
        let Some(found) = locate_kubernetes(state, org, project, &environment_id, key.trim()) else {
            return fail(StatusCode::NOT_FOUND, "cluster introuvable");
        };
        let stored_id = value_key(&found["id"]);
        let removed = delete_item(state, org, project, "distributedtask/environments/kubernetes", &stored_id);
        return if removed {
            StatusCode::NO_CONTENT.into_response()
        } else {
            fail(StatusCode::NOT_FOUND, "cluster introuvable")
        };
    }
    fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée")
}

fn list_kubernetes(state: &AppState, org: &str, project: &str, environment_id: &str) -> Vec<Value> {
    list(state, org, project, "distributedtask/environments/kubernetes")
        .into_iter()
        .filter(|item| text_of(item, "environmentId").is_some_and(|stored| stored == environment_id))
        .collect()
}

fn locate_kubernetes(state: &AppState, org: &str, project: &str, environment_id: &str, key: &str) -> Option<Value> {
    list_kubernetes(state, org, project, environment_id)
        .into_iter()
        .find(|item| value_key(&item["id"]) == key || text_of(item, "name").is_some_and(|stored| stored.eq_ignore_ascii_case(key)))
}

fn next_kubernetes_id(state: &AppState, org: &str) -> i64 {
    state
        .read(|db| {
            let Some(organization) = db.orgs.get(org) else {
                return Vec::new();
            };
            let mut rows = Vec::new();
            for project in organization.projects.values() {
                if let Some(bucket) = project.buckets.get("distributedtask/environments/kubernetes") {
                    rows.extend(bucket.items.values().cloned());
                }
            }
            rows
        })
        .iter()
        .filter_map(|item| value_key(&item["id"]).strip_prefix("k8s-")?.parse::<i64>().ok())
        .max()
        .unwrap_or(0)
        + 1
}

fn kubernetes_create(
    state: &AppState,
    org: &str,
    project: &str,
    environment_id: &str,
    environment_name: &str,
    parsed: (String, String, String),
) -> Result<Value, &'static str> {
    let (name, namespace, cluster_name) = parsed;
    if list_kubernetes(state, org, project, environment_id)
        .iter()
        .any(|item| text_of(item, "name").is_some_and(|stored| stored.eq_ignore_ascii_case(&name)))
    {
        return Err("cluster existant");
    }
    let id = format!("k8s-{}", next_kubernetes_id(state, org));
    let item = json!({
        "id": id,
        "name": name,
        "type": "kubernetes",
        "namespace": namespace,
        "clusterName": cluster_name,
        "environmentId": environment_id,
        "environmentName": environment_name
    });
    put_item(state, org, project, "distributedtask/environments/kubernetes", item.clone());
    Ok(dress_kubernetes(org, project, environment_name, item))
}

fn kubernetes_payload(body: &[u8]) -> Result<(String, String, String), &'static str> {
    let Some(payload) = parse_json(body) else {
        return Err("JSON illisible");
    };
    let Some(object) = payload.as_object() else {
        return Err("cluster illisible");
    };
    let name = match object.get("name") {
        Some(Value::String(value)) => {
            let value = value.trim();
            if value.is_empty() {
                return Err("nom absent");
            }
            value.to_string()
        }
        Some(_) => return Err("nom absent"),
        None => return Err("nom absent"),
    };
    let namespace = match object.get("namespace") {
        None => "default".to_string(),
        Some(Value::String(value)) => {
            let value = value.trim();
            if value.is_empty() {
                return Err("espace absent");
            }
            value.to_string()
        }
        Some(_) => return Err("espace absent"),
    };
    let cluster_name = match object.get("clusterName") {
        None => name.clone(),
        Some(Value::String(value)) => {
            let value = value.trim();
            if value.is_empty() {
                return Err("cluster absent");
            }
            value.to_string()
        }
        Some(_) => return Err("cluster absent"),
    };
    Ok((name, namespace, cluster_name))
}

fn dress_kubernetes(org: &str, project: &str, environment_name: &str, mut item: Value) -> Value {
    let environment_id = text_of(&item, "environmentId").unwrap_or_default();
    let id = value_key(&item["id"]);
    if let Some(object) = item.as_object_mut() {
        object.insert("environmentName".into(), json!(environment_name));
        object.insert("project".into(), json!(project));
        object.insert(
            "url".into(),
            json!(format!("/{org}/{project}/_apis/distributedtask/environments/{environment_id}/providers/kubernetes/{id}")),
        );
    }
    item
}

fn delete_environment(state: &AppState, org: &str, project: &str, rest: &[String]) -> Response {
    let key = "distributedtask/environments";
    if rest.len() == 2 {
        return fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée");
    }
    if rest.len() != 3 {
        return fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée");
    }
    let Some(found) = get_item(state, org, project, key, &rest[2]) else {
        return fail(StatusCode::NOT_FOUND, "environnement introuvable");
    };
    let stored_id = value_key(&found["id"]);
    state.write(|db| {
        if let Some(stored) = db.project_mut(org, project) {
            if let Some(bucket) = stored.buckets.get_mut(key) {
                bucket.remove(&stored_id);
            }
            if let Some(machines) = stored.buckets.get_mut("distributedtask/environments/virtualmachines") {
                let doomed: Vec<String> = machines
                    .items
                    .iter()
                    .filter(|(_, item)| text_of(item, "environmentId").is_some_and(|stored| stored == stored_id))
                    .map(|(stored_key, _)| stored_key.clone())
                    .collect();
                for stored_key in doomed {
                    machines.items.remove(&stored_key);
                }
            }
            if let Some(clusters) = stored.buckets.get_mut("distributedtask/environments/kubernetes") {
                let doomed: Vec<String> = clusters
                    .items
                    .iter()
                    .filter(|(_, item)| text_of(item, "environmentId").is_some_and(|stored| stored == stored_id))
                    .map(|(stored_key, _)| stored_key.clone())
                    .collect();
                for stored_key in doomed {
                    clusters.items.remove(&stored_key);
                }
            }
            if let Some(checks) = stored.buckets.get_mut("pipelines/checks/configurations") {
                let doomed: Vec<String> = checks
                    .items
                    .iter()
                    .filter_map(|(stored_key, item)| {
                        let resource = item.get("resource")?;
                        let kind = text_of(resource, "type").unwrap_or_default();
                        if kind.eq_ignore_ascii_case("environment") && value_key(&resource["id"]) == stored_id {
                            Some(stored_key.clone())
                        } else {
                            None
                        }
                    })
                    .collect();
                for stored_key in doomed {
                    checks.items.remove(&stored_key);
                }
            }
        }
    });
    StatusCode::NO_CONTENT.into_response()
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

pub fn work_item_tags(state: &AppState, method: &Method, org: &str, project: &str, rest: &[String], body: &[u8]) -> Response {
    let key = "wit/tags";
    if rest.len() == 2 {
        if method == &Method::GET {
            let mut rows = list(state, org, project, key);
            rows.sort_by_key(|item| item.get("id").and_then(json_i64).unwrap_or(0));
            let value = rows.into_iter().map(|item| dress_work_item_tag(org, project, item)).collect();
            return Json(page(value)).into_response();
        }
        if method == &Method::POST {
            return create_work_item_tag(state, org, project, body);
        }
        return fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée");
    }
    if rest.len() != 3 {
        if method == &Method::DELETE {
            return fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée");
        }
        return fail(StatusCode::NOT_FOUND, "étiquette introuvable");
    }
    let wanted = pct_decode(&rest[2]).trim().to_string();
    if wanted.is_empty() {
        return fail(StatusCode::NOT_FOUND, "étiquette introuvable");
    }
    let Some(item) = find_work_item_tag(&list(state, org, project, key), &wanted) else {
        return fail(StatusCode::NOT_FOUND, "étiquette introuvable");
    };
    let id = value_key(&item["id"]);
    if method == &Method::GET {
        return Json(dress_work_item_tag(org, project, item)).into_response();
    }
    if method == &Method::DELETE {
        return if delete_item(state, org, project, key, &id) {
            StatusCode::NO_CONTENT.into_response()
        } else {
            fail(StatusCode::NOT_FOUND, "étiquette introuvable")
        };
    }
    fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée")
}

fn create_work_item_tag(state: &AppState, org: &str, project: &str, body: &[u8]) -> Response {
    let Some(payload) = parse_json(body) else {
        return fail(StatusCode::BAD_REQUEST, "JSON illisible");
    };
    let Some(object) = payload.as_object() else {
        return fail(StatusCode::BAD_REQUEST, "nom absent");
    };
    let name = object.get("name").and_then(|value| value.as_str()).map(str::trim).unwrap_or("");
    if name.is_empty() {
        return fail(StatusCode::BAD_REQUEST, "nom absent");
    }
    let existing = list(state, org, project, "wit/tags");
    if existing.iter().any(|item| {
        item.get("name")
            .and_then(|value| value.as_str())
            .is_some_and(|current| current.eq_ignore_ascii_case(name))
    }) {
        return fail(StatusCode::BAD_REQUEST, "étiquette déjà présente");
    }
    let next = existing.iter().filter_map(|item| item.get("id").and_then(json_i64)).max().unwrap_or(0) + 1;
    let item = json!({"id": next, "name": name});
    put_item(state, org, project, "wit/tags", item.clone());
    Json(dress_work_item_tag(org, project, item)).into_response()
}

fn find_work_item_tag(rows: &[Value], wanted: &str) -> Option<Value> {
    rows.iter()
        .find(|item| value_key(&item["id"]) == wanted)
        .cloned()
        .or_else(|| {
            rows.iter()
                .find(|item| {
                    item.get("name")
                        .and_then(|value| value.as_str())
                        .is_some_and(|name| name.eq_ignore_ascii_case(wanted))
                })
                .cloned()
        })
}

fn dress_work_item_tag(org: &str, project: &str, mut item: Value) -> Value {
    let id = value_key(&item["id"]);
    if let Some(object) = item.as_object_mut() {
        object.insert("project".into(), json!({"name": project}));
        object.insert("url".into(), json!(format!("/{org}/{project}/_apis/wit/tags/{id}")));
    }
    item
}

fn test_variables(state: &AppState, method: &Method, org: &str, project: &str, rest: &[String], body: &[u8]) -> Response {
    let key = "testplan/variables";
    if rest.len() == 2 {
        if method == &Method::GET {
            let mut rows = list(state, org, project, key);
            rows.sort_by_key(|item| item.get("id").and_then(json_i64).unwrap_or(0));
            let value = rows.into_iter().map(|item| dress_test_variable(org, project, item)).collect();
            return Json(page(value)).into_response();
        }
        if method == &Method::POST {
            return create_test_variable(state, org, project, body);
        }
        return fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée");
    }
    if rest.len() != 3 {
        if method == &Method::DELETE {
            return fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée");
        }
        return fail(StatusCode::NOT_FOUND, "variable introuvable");
    }
    let id = pct_decode(&rest[2]).trim().to_string();
    if id.is_empty() {
        return fail(StatusCode::NOT_FOUND, "variable introuvable");
    }
    if method == &Method::GET {
        return match get_item(state, org, project, key, &id) {
            Some(item) => Json(dress_test_variable(org, project, item)).into_response(),
            None => fail(StatusCode::NOT_FOUND, "variable introuvable"),
        };
    }
    if method == &Method::DELETE {
        return if delete_item(state, org, project, key, &id) {
            StatusCode::NO_CONTENT.into_response()
        } else {
            fail(StatusCode::NOT_FOUND, "variable introuvable")
        };
    }
    fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée")
}

fn create_test_variable(state: &AppState, org: &str, project: &str, body: &[u8]) -> Response {
    let Some(payload) = parse_json(body) else {
        return fail(StatusCode::BAD_REQUEST, "JSON illisible");
    };
    let Some(object) = payload.as_object() else {
        return fail(StatusCode::BAD_REQUEST, "nom absent");
    };
    let name = object.get("name").and_then(|value| value.as_str()).map(str::trim).unwrap_or("");
    if name.is_empty() {
        return fail(StatusCode::BAD_REQUEST, "nom absent");
    }
    let values = match test_variable_values(object.get("values")) {
        Ok(values) => values,
        Err(message) => return fail(StatusCode::BAD_REQUEST, message),
    };
    let description = object
        .get("description")
        .and_then(|value| value.as_str())
        .map(str::trim)
        .unwrap_or("")
        .to_string();
    let next = list(state, org, project, "testplan/variables")
        .iter()
        .filter_map(|item| item.get("id").and_then(json_i64))
        .max()
        .unwrap_or(0)
        + 1;
    let item = json!({
        "id": next,
        "name": name,
        "description": description,
        "values": values
    });
    put_item(state, org, project, "testplan/variables", item.clone());
    Json(dress_test_variable(org, project, item)).into_response()
}

fn test_variable_values(value: Option<&Value>) -> Result<Vec<String>, &'static str> {
    let Some(value) = value else {
        return Ok(Vec::new());
    };
    let Some(rows) = value.as_array() else {
        return Err("valeur absente");
    };
    let mut out = Vec::new();
    for row in rows {
        let Some(text) = row.as_str().map(str::trim) else {
            return Err("valeur absente");
        };
        if text.is_empty() {
            return Err("valeur absente");
        }
        out.push(text.to_string());
    }
    Ok(out)
}

fn dress_test_variable(org: &str, project: &str, mut item: Value) -> Value {
    let id = value_key(&item["id"]);
    if let Some(object) = item.as_object_mut() {
        object.entry("description").or_insert(json!(""));
        object.entry("values").or_insert(json!([]));
        object.insert("project".into(), json!({"name": project}));
        object.insert(
            "url".into(),
            json!(format!("/{org}/{project}/_apis/testplan/variables/{id}")),
        );
    }
    item
}

fn test_configurations(state: &AppState, method: &Method, org: &str, project: &str, rest: &[String], body: &[u8]) -> Response {
    let key = "test/configurations";
    let area = rest.first().map(|value| value.to_ascii_lowercase()).unwrap_or_else(|| "test".into());
    if rest.len() == 2 {
        if method == &Method::GET {
            let mut rows = list(state, org, project, key);
            rows.sort_by_key(|item| item.get("id").and_then(json_i64).unwrap_or(0));
            let value = rows
                .into_iter()
                .map(|item| dress_configuration(org, project, &area, item))
                .collect();
            return Json(page(value)).into_response();
        }
        if method == &Method::POST {
            return create_configuration(state, org, project, &area, body);
        }
        return fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée");
    }
    if rest.len() != 3 {
        if method == &Method::DELETE {
            return fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée");
        }
        return fail(StatusCode::NOT_FOUND, "configuration introuvable");
    }
    let id = pct_decode(&rest[2]).trim().to_string();
    if id.is_empty() {
        return fail(StatusCode::NOT_FOUND, "configuration introuvable");
    }
    if method == &Method::GET {
        return match get_item(state, org, project, key, &id) {
            Some(item) => Json(dress_configuration(org, project, &area, item)).into_response(),
            None => fail(StatusCode::NOT_FOUND, "configuration introuvable"),
        };
    }
    if method == &Method::DELETE {
        let Some(item) = get_item(state, org, project, key, &id) else {
            return fail(StatusCode::NOT_FOUND, "configuration introuvable");
        };
        if item.get("isDefault").and_then(|value| value.as_bool()) == Some(true) {
            return fail(StatusCode::BAD_REQUEST, "configuration par défaut");
        }
        return if delete_item(state, org, project, key, &id) {
            StatusCode::NO_CONTENT.into_response()
        } else {
            fail(StatusCode::NOT_FOUND, "configuration introuvable")
        };
    }
    if method == &Method::PATCH {
        return patch_configuration(state, org, project, &area, &id, body);
    }
    fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée")
}

fn create_configuration(state: &AppState, org: &str, project: &str, area: &str, body: &[u8]) -> Response {
    let Some(payload) = parse_json(body) else {
        return fail(StatusCode::BAD_REQUEST, "JSON illisible");
    };
    let Some(object) = payload.as_object() else {
        return fail(StatusCode::BAD_REQUEST, "nom absent");
    };
    let name = object.get("name").and_then(|value| value.as_str()).map(str::trim).unwrap_or("");
    if name.is_empty() {
        return fail(StatusCode::BAD_REQUEST, "nom absent");
    }
    let values = match configuration_values(object.get("values")) {
        Ok(values) => values,
        Err(message) => return fail(StatusCode::BAD_REQUEST, message),
    };
    let is_default = object.get("isDefault").and_then(|value| value.as_bool()).unwrap_or(false);
    if is_default && configuration_default_id(state, org, project).is_some() {
        return fail(StatusCode::BAD_REQUEST, "configuration par défaut");
    }
    let description = object.get("description").and_then(|value| value.as_str()).unwrap_or("").trim().to_string();
    let next = list(state, org, project, "test/configurations")
        .iter()
        .filter_map(|item| item.get("id").and_then(json_i64))
        .max()
        .unwrap_or(0)
        + 1;
    let item = json!({
        "id": next,
        "name": name,
        "description": description,
        "isDefault": is_default,
        "state": "active",
        "values": values
    });
    put_item(state, org, project, "test/configurations", item.clone());
    Json(dress_configuration(org, project, area, item)).into_response()
}

fn patch_configuration(state: &AppState, org: &str, project: &str, area: &str, id: &str, body: &[u8]) -> Response {
    let Some(mut current) = get_item(state, org, project, "test/configurations", id) else {
        return fail(StatusCode::NOT_FOUND, "configuration introuvable");
    };
    let Some(payload) = parse_json(body) else {
        return fail(StatusCode::BAD_REQUEST, "JSON illisible");
    };
    let Some(object) = payload.as_object() else {
        return fail(StatusCode::BAD_REQUEST, "nom absent");
    };
    if let Some(name) = object.get("name") {
        let name = name.as_str().map(str::trim).unwrap_or("");
        if name.is_empty() {
            return fail(StatusCode::BAD_REQUEST, "nom absent");
        }
        current["name"] = json!(name);
    }
    if let Some(description) = object.get("description").and_then(|value| value.as_str()) {
        current["description"] = json!(description.trim());
    }
    if object.contains_key("values") {
        let values = match configuration_values(object.get("values")) {
            Ok(values) => values,
            Err(message) => return fail(StatusCode::BAD_REQUEST, message),
        };
        current["values"] = json!(values);
    }
    if let Some(is_default) = object.get("isDefault").and_then(|value| value.as_bool()) {
        if is_default {
            if let Some(existing) = configuration_default_id(state, org, project) {
                if existing != id {
                    return fail(StatusCode::BAD_REQUEST, "configuration par défaut");
                }
            }
        }
        current["isDefault"] = json!(is_default);
    }
    put_item(state, org, project, "test/configurations", current.clone());
    Json(dress_configuration(org, project, area, current)).into_response()
}

fn configuration_default_id(state: &AppState, org: &str, project: &str) -> Option<String> {
    list(state, org, project, "test/configurations").into_iter().find_map(|item| {
        (item.get("isDefault").and_then(|value| value.as_bool()) == Some(true)).then(|| value_key(&item["id"]))
    })
}

fn configuration_values(value: Option<&Value>) -> Result<Vec<Value>, &'static str> {
    let Some(value) = value else {
        return Ok(Vec::new());
    };
    let Some(rows) = value.as_array() else {
        return Err("valeur absente");
    };
    let mut out = Vec::new();
    for row in rows {
        let Some(object) = row.as_object() else {
            return Err("valeur absente");
        };
        let name = object.get("name").and_then(|item| item.as_str()).map(str::trim).unwrap_or("");
        let stored = object.get("value").and_then(|item| item.as_str()).map(str::trim).unwrap_or("");
        if name.is_empty() || stored.is_empty() {
            return Err("valeur absente");
        }
        out.push(json!({"name": name, "value": stored}));
    }
    Ok(out)
}

fn dress_configuration(org: &str, project: &str, area: &str, mut item: Value) -> Value {
    let id = value_key(&item["id"]);
    if let Some(object) = item.as_object_mut() {
        object.entry("description").or_insert(json!(""));
        object.entry("isDefault").or_insert(json!(false));
        object.entry("state").or_insert(json!("active"));
        object.entry("values").or_insert(json!([]));
        object.insert("project".into(), json!({"name": project}));
        object.insert("url".into(), json!(format!("/{org}/{project}/_apis/{area}/configurations/{id}")));
    }
    item
}

fn shared_parameters(state: &AppState, method: &Method, org: &str, project: &str, rest: &[String], body: &[u8]) -> Response {
    let key = "testplan/sharedparameters";
    if rest.len() == 2 {
        if method == Method::GET {
            let mut rows = list(state, org, project, key);
            rows.sort_by(|left, right| value_key(&left["id"]).cmp(&value_key(&right["id"])));
            let value = rows.into_iter().map(|item| dress_shared_parameter(org, project, item)).collect();
            return Json(page(value)).into_response();
        }
        if method == Method::POST {
            let parsed = match shared_parameter_payload(body) {
                Ok(parsed) => parsed,
                Err(message) => return fail(StatusCode::BAD_REQUEST, message),
            };
            return match shared_parameter_create(state, org, project, parsed) {
                Ok(item) => Json(item).into_response(),
                Err(message) => fail(StatusCode::BAD_REQUEST, message),
            };
        }
        return fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée");
    }
    if rest.len() != 3 {
        return if method == Method::DELETE {
            fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée")
        } else {
            fail(StatusCode::NOT_FOUND, "paramètre introuvable")
        };
    }
    let id = pct_decode(&rest[2]);
    if method == Method::GET {
        return match locate_shared_parameter(state, org, project, id.trim()) {
            Some(found) => Json(dress_shared_parameter(org, project, found)).into_response(),
            None => fail(StatusCode::NOT_FOUND, "paramètre introuvable"),
        };
    }
    if method == Method::DELETE {
        let Some(found) = locate_shared_parameter(state, org, project, id.trim()) else {
            return fail(StatusCode::NOT_FOUND, "paramètre introuvable");
        };
        let stored_id = value_key(&found["id"]);
        let removed = delete_item(state, org, project, key, &stored_id);
        return if removed {
            StatusCode::NO_CONTENT.into_response()
        } else {
            fail(StatusCode::NOT_FOUND, "paramètre introuvable")
        };
    }
    fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée")
}

fn locate_shared_parameter(state: &AppState, org: &str, project: &str, key: &str) -> Option<Value> {
    list(state, org, project, "testplan/sharedparameters")
        .into_iter()
        .find(|item| value_key(&item["id"]) == key || text_of(item, "name").is_some_and(|stored| stored.eq_ignore_ascii_case(key)))
}

fn next_shared_parameter_id(state: &AppState, org: &str) -> i64 {
    state
        .read(|db| {
            let Some(organization) = db.orgs.get(org) else {
                return Vec::new();
            };
            let mut rows = Vec::new();
            for project in organization.projects.values() {
                if let Some(bucket) = project.buckets.get("testplan/sharedparameters") {
                    rows.extend(bucket.items.values().cloned());
                }
            }
            rows
        })
        .iter()
        .filter_map(|item| value_key(&item["id"]).strip_prefix("sp-")?.parse::<i64>().ok())
        .max()
        .unwrap_or(0)
        + 1
}

fn shared_parameter_create(
    state: &AppState,
    org: &str,
    project: &str,
    parsed: (String, Vec<String>, Vec<Value>),
) -> Result<Value, &'static str> {
    let (name, param_names, param_data) = parsed;
    if list(state, org, project, "testplan/sharedparameters")
        .iter()
        .any(|item| text_of(item, "name").is_some_and(|stored| stored.eq_ignore_ascii_case(&name)))
    {
        return Err("paramètre existant");
    }
    let id = format!("sp-{}", next_shared_parameter_id(state, org));
    let item = json!({
        "id": id,
        "name": name,
        "paramNames": param_names,
        "paramData": param_data
    });
    put_item(state, org, project, "testplan/sharedparameters", item.clone());
    Ok(dress_shared_parameter(org, project, item))
}

fn shared_parameter_name(object: &Map<String, Value>) -> Result<String, &'static str> {
    for key in ["name", "sharedParameterDataSetName"] {
        if let Some(value) = object.get(key) {
            let Some(text) = value.as_str().map(str::trim).filter(|text| !text.is_empty()) else {
                return Err("nom absent");
            };
            return Ok(text.to_string());
        }
    }
    Err("nom absent")
}

fn shared_parameter_payload(body: &[u8]) -> Result<(String, Vec<String>, Vec<Value>), &'static str> {
    let Some(payload) = parse_json(body) else {
        return Err("JSON illisible");
    };
    let Some(object) = payload.as_object() else {
        return Err("paramètre illisible");
    };
    let name = shared_parameter_name(object)?;
    let param_names = match object.get("paramNames") {
        None => vec![name.clone()],
        Some(Value::Array(items)) if !items.is_empty() => {
            let mut names = Vec::new();
            for item in items {
                let Some(value) = item.as_str().map(str::trim).filter(|value| !value.is_empty()) else {
                    return Err("nom de paramètre absent");
                };
                if names.iter().any(|stored: &String| stored.eq_ignore_ascii_case(value)) {
                    return Err("nom de paramètre existant");
                }
                names.push(value.to_string());
            }
            names
        }
        Some(_) => return Err("nom de paramètre absent"),
    };
    let param_data = match object.get("paramData") {
        None => {
            let mut row = Map::new();
            for param in &param_names {
                row.insert(param.clone(), json!(""));
            }
            vec![Value::Object(row)]
        }
        Some(Value::Array(items)) => {
            let mut rows = Vec::new();
            for item in items {
                let Some(row) = item.as_object() else {
                    return Err("valeur absente");
                };
                if row.len() != param_names.len() || param_names.iter().any(|param| !row.contains_key(param)) {
                    return Err("valeur absente");
                }
                let mut stored = Map::new();
                for param in &param_names {
                    let Some(value) = row.get(param).and_then(|value| value.as_str()) else {
                        return Err("valeur absente");
                    };
                    stored.insert(param.clone(), json!(value));
                }
                rows.push(Value::Object(stored));
            }
            rows
        }
        Some(_) => return Err("valeur absente"),
    };
    Ok((name, param_names, param_data))
}

fn dress_shared_parameter(org: &str, project: &str, mut item: Value) -> Value {
    let id = value_key(&item["id"]);
    if let Some(object) = item.as_object_mut() {
        object.insert("project".into(), json!(project));
        object.insert("url".into(), json!(format!("/{org}/{project}/_apis/testplan/sharedparameters/{id}")));
    }
    item
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
    if rest.len() == 2 && method == Method::DELETE {
        return fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée");
    }
    let plan_id = rest.get(2).map(String::as_str).unwrap_or("");
    if rest.len() == 3 && method == Method::GET {
        return match get_item(state, org, project, key, plan_id) {
            Some(item) => Json(dress_plan(project, item)).into_response(),
            None => fail(StatusCode::NOT_FOUND, "plan introuvable"),
        };
    }
    if rest.len() == 3 && method == Method::DELETE {
        let Some(found) = get_item(state, org, project, key, plan_id) else {
            return fail(StatusCode::NOT_FOUND, "plan introuvable");
        };
        let stored_id = value_key(&found["id"]);
        let prefix = format!("testplan/plans/{stored_id}/");
        state.write(|db| {
            let Some(stored) = db.project_mut(org, project) else {
                return;
            };
            if let Some(bucket) = stored.buckets.get_mut(key) {
                bucket.remove(&stored_id);
            }
            let nested: Vec<String> = stored.buckets.keys().filter(|name| name.starts_with(&prefix)).cloned().collect();
            for name in nested {
                stored.buckets.remove(&name);
            }
        });
        return StatusCode::NO_CONTENT.into_response();
    }
    if rest.len() >= 4 && rest[3].eq_ignore_ascii_case("suites") {
        return plan_suites(state, method, org, project, plan_id, rest, body);
    }
    if method == Method::DELETE {
        return fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée");
    }
    fail(StatusCode::NOT_FOUND, "plan introuvable")
}

fn plan_suites(state: &AppState, method: &Method, org: &str, project: &str, plan_id: &str, rest: &[String], body: &[u8]) -> Response {
    let suites_key = format!("testplan/plans/{plan_id}/suites");
    if get_item(state, org, project, "testplan/plans", plan_id).is_none() {
        return fail(StatusCode::NOT_FOUND, "plan introuvable");
    }
    ensure_root_suite(state, org, project, plan_id);
    if rest.len() == 4 && method == Method::GET {
        return Json(page(list(state, org, project, &suites_key))).into_response();
    }
    if rest.len() == 4 && method == Method::POST {
        let payload = parse_json(body).unwrap_or(json!({}));
        let name = text_of(&payload, "name").unwrap_or_else(|| "Suite".into());
        let next = next_bucket_id(state, org, project, &suites_key);
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
        return suite_cases(state, method, org, project, plan_id, suite_id, rest.get(6).map(String::as_str), body);
    }
    if (child == "testpoint" || child == "testpoints") && method == Method::GET {
        if get_item(state, org, project, &suites_key, suite_id).is_none() {
            return fail(StatusCode::NOT_FOUND, "suite introuvable");
        }
        return Json(page(list(state, org, project, &format!("testplan/plans/{plan_id}/suites/{suite_id}/points")))).into_response();
    }
    if rest.len() == 5 && method == Method::GET {
        return match get_item(state, org, project, &suites_key, suite_id) {
            Some(item) => Json(item).into_response(),
            None => fail(StatusCode::NOT_FOUND, "suite introuvable"),
        };
    }
    if rest.len() == 5 && (method == Method::PATCH || method == Method::DELETE) {
        return update_suite(state, method, org, project, plan_id, suite_id, body);
    }
    fail(StatusCode::NOT_FOUND, "suite introuvable")
}

fn next_bucket_id(state: &AppState, org: &str, project: &str, key: &str) -> i64 {
    state
        .read(|db| db.project(org, project).and_then(|stored| stored.buckets.get(key)).map(|bucket| bucket.next))
        .unwrap_or(1)
}

fn suite_is_root(plan_id: &str, suite: &Value) -> bool {
    value_key(&suite["id"]) == plan_id || suite.get("parentSuite").is_none()
}

fn suite_tree(suites: &[Value], root: &str) -> Vec<String> {
    let mut doomed = vec![root.to_string()];
    let mut grew = true;
    while grew {
        grew = false;
        for item in suites {
            let id = value_key(&item["id"]);
            let parent = item.get("parentSuite").map(|suite| value_key(&suite["id"])).unwrap_or_default();
            if doomed.iter().any(|kept| kept == &parent) && doomed.iter().all(|kept| kept != &id) {
                doomed.push(id);
                grew = true;
            }
        }
    }
    doomed
}

fn update_suite(state: &AppState, method: &Method, org: &str, project: &str, plan_id: &str, suite_id: &str, body: &[u8]) -> Response {
    let suites_key = format!("testplan/plans/{plan_id}/suites");
    let Some(suite) = get_item(state, org, project, &suites_key, suite_id) else {
        return fail(StatusCode::NOT_FOUND, "suite introuvable");
    };
    if method == Method::DELETE {
        if suite_is_root(plan_id, &suite) {
            return fail(StatusCode::BAD_REQUEST, "suite racine");
        }
        let doomed = suite_tree(&list(state, org, project, &suites_key), suite_id);
        state.write(|db| {
            let Some(stored) = db.project_mut(org, project) else {
                return;
            };
            if let Some(bucket) = stored.buckets.get_mut(&suites_key) {
                for id in &doomed {
                    bucket.remove(id);
                }
            }
            for id in &doomed {
                stored.buckets.remove(&format!("testplan/plans/{plan_id}/suites/{id}/testcases"));
                stored.buckets.remove(&format!("testplan/plans/{plan_id}/suites/{id}/points"));
            }
        });
        deliver_work_item(state, org, project, "test.suite.deleted", &suite);
        return StatusCode::NO_CONTENT.into_response();
    }
    let Some(payload) = parse_json(body) else {
        return fail(StatusCode::BAD_REQUEST, "suite illisible");
    };
    let Some(object) = payload.as_object() else {
        return fail(StatusCode::BAD_REQUEST, "suite illisible");
    };
    let Some(name) = object.get("name").and_then(|value| value.as_str()) else {
        return fail(StatusCode::BAD_REQUEST, "suite illisible");
    };
    let name = name.trim();
    if name.is_empty() {
        return fail(StatusCode::BAD_REQUEST, "nom de suite absent");
    }
    let mut renamed = suite;
    if let Some(fields) = renamed.as_object_mut() {
        fields.insert("name".into(), json!(name));
    }
    put_item(state, org, project, &suites_key, renamed.clone());
    deliver_work_item(state, org, project, "test.suite.updated", &renamed);
    Json(renamed).into_response()
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

fn suite_cases(state: &AppState, method: &Method, org: &str, project: &str, plan_id: &str, suite_id: &str, case_id: Option<&str>, body: &[u8]) -> Response {
    let suites_key = format!("testplan/plans/{plan_id}/suites");
    if get_item(state, org, project, &suites_key, suite_id).is_none() {
        return fail(StatusCode::NOT_FOUND, "suite introuvable");
    }
    let cases_key = format!("testplan/plans/{plan_id}/suites/{suite_id}/testcases");
    let points_key = format!("testplan/plans/{plan_id}/suites/{suite_id}/points");
    if method == Method::DELETE {
        let Some(id) = case_id.filter(|value| !value.is_empty()) else {
            return fail(StatusCode::NOT_FOUND, "cas introuvable");
        };
        let Some(case) = get_item(state, org, project, &cases_key, id) else {
            return fail(StatusCode::NOT_FOUND, "cas introuvable");
        };
        state.write(|db| {
            let Some(stored) = db.project_mut(org, project) else {
                return;
            };
            if let Some(bucket) = stored.buckets.get_mut(&cases_key) {
                bucket.remove(id);
            }
            if let Some(bucket) = stored.buckets.get_mut(&points_key) {
                let gone: Vec<String> = bucket
                    .items
                    .iter()
                    .filter(|(_, item)| item.get("testCase").map(|case| value_key(&case["id"])).as_deref() == Some(id))
                    .map(|(key, _)| key.clone())
                    .collect();
                for key in gone {
                    bucket.remove(&key);
                }
            }
        });
        deliver_work_item(state, org, project, "test.case.removed", &case);
        return StatusCode::NO_CONTENT.into_response();
    }
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
    if rest.get(3).is_some_and(|part| part.eq_ignore_ascii_case("attachments")) {
        return run_attachments(state, method, org, project, run_id, rest, body);
    }
    if rest.get(3).is_some_and(|part| part.eq_ignore_ascii_case("results"))
        && rest.get(5).is_some_and(|part| part.eq_ignore_ascii_case("attachments"))
    {
        return result_attachments(state, method, org, project, run_id, rest, body);
    }
    if rest.get(3).is_some_and(|part| part.eq_ignore_ascii_case("results"))
        && rest.get(5).is_some_and(|part| part.eq_ignore_ascii_case("bugs"))
    {
        return result_bugs(state, method, org, project, run_id, rest, body);
    }
    if rest.get(3).is_some_and(|part| part.eq_ignore_ascii_case("results"))
        && rest.get(5).is_some_and(|part| part.eq_ignore_ascii_case("iterations"))
        && rest.get(7).is_some_and(|part| part.eq_ignore_ascii_case("actionresults"))
        && rest.get(9).is_some_and(|part| part.eq_ignore_ascii_case("attachments"))
    {
        return action_attachments(state, method, org, project, run_id, rest, body);
    }
    if rest.get(3).is_some_and(|part| part.eq_ignore_ascii_case("results"))
        && rest.get(5).is_some_and(|part| part.eq_ignore_ascii_case("iterations"))
        && rest.get(7).is_some_and(|part| part.eq_ignore_ascii_case("actionresults"))
        && rest.get(9).is_some_and(|part| part.eq_ignore_ascii_case("parameters"))
    {
        return action_parameters(state, method, org, project, run_id, rest, body);
    }
    if rest.get(3).is_some_and(|part| part.eq_ignore_ascii_case("results"))
        && rest.get(5).is_some_and(|part| part.eq_ignore_ascii_case("iterations"))
        && rest.get(7).is_some_and(|part| part.eq_ignore_ascii_case("actionresults"))
    {
        return result_actions(state, method, org, project, run_id, rest, body);
    }
    if rest.get(3).is_some_and(|part| part.eq_ignore_ascii_case("results"))
        && rest.get(5).is_some_and(|part| part.eq_ignore_ascii_case("iterations"))
        && rest.get(7).is_some_and(|part| part.eq_ignore_ascii_case("attachments"))
    {
        return iteration_attachments(state, method, org, project, run_id, rest, body);
    }
    if rest.get(3).is_some_and(|part| part.eq_ignore_ascii_case("results"))
        && rest.get(5).is_some_and(|part| part.eq_ignore_ascii_case("iterations"))
        && rest.get(7).is_some_and(|part| part.eq_ignore_ascii_case("parameterresults"))
    {
        return iteration_parameters(state, method, org, project, run_id, rest, body);
    }
    if rest.get(3).is_some_and(|part| part.eq_ignore_ascii_case("results"))
        && rest.get(5).is_some_and(|part| part.eq_ignore_ascii_case("iterations"))
    {
        return result_iterations(state, method, org, project, run_id, rest, body);
    }
    if rest.get(3).is_some_and(|part| part.eq_ignore_ascii_case("results"))
        && rest.get(5).is_some_and(|part| part.eq_ignore_ascii_case("subresults"))
        && rest.get(7).is_some_and(|part| part.eq_ignore_ascii_case("attachments"))
    {
        return sub_result_attachments(state, method, org, project, run_id, rest, body);
    }
    if rest.get(3).is_some_and(|part| part.eq_ignore_ascii_case("results"))
        && rest.get(5).is_some_and(|part| part.eq_ignore_ascii_case("subresults"))
    {
        return sub_results(state, method, org, project, run_id, rest, body);
    }
    if rest.get(3).is_some_and(|part| part.eq_ignore_ascii_case("results"))
        && rest.get(5).is_some_and(|part| part.eq_ignore_ascii_case("logs"))
    {
        return result_logs(state, method, org, project, run_id, rest, body);
    }
    if rest.len() >= 4 && rest[3].eq_ignore_ascii_case("results") {
        if get_item(state, org, project, key, run_id).is_none() {
            return fail(StatusCode::NOT_FOUND, "exécution introuvable");
        }
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
    if method == Method::DELETE && rest.len() == 3 {
        let Some(run) = get_item(state, org, project, key, run_id) else {
            return fail(StatusCode::NOT_FOUND, "exécution introuvable");
        };
        let stored_run = value_key(&run["id"]);
        let results_key = format!("test/runs/{stored_run}/results");
        state.write(|db| {
            if let Some(stored) = db.project_mut(org, project) {
                if let Some(bucket) = stored.buckets.get_mut(key) {
                    bucket.remove(&stored_run);
                }
                stored.buckets.remove(&results_key);
                stored.buckets.remove(&format!("test/runs/{stored_run}/attachments"));
                stored.buckets.remove(&format!("testresults/runs/{stored_run}/testattachments"));
                stored.buckets.remove(&format!("testresults/runs/{stored_run}/tags"));
                stored.buckets.remove(&format!("testresults/runs/{stored_run}/customfields"));
                let prefix = format!("test/runs/{stored_run}/results/");
                let nested: Vec<String> = stored.buckets.keys().filter(|key| key.starts_with(&prefix)).cloned().collect();
                for key in nested {
                    stored.buckets.remove(&key);
                }
            }
        });
        return StatusCode::NO_CONTENT.into_response();
    }
    fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée")
}

fn run_attachments(
    state: &AppState,
    method: &Method,
    org: &str,
    project: &str,
    run_id: &str,
    rest: &[String],
    body: &[u8],
) -> Response {
    let attachment_tail = if rest.len() > 4 { &rest[4..] } else { &[] };
    if attachment_tail.len() > 1 {
        return if method == Method::DELETE {
            fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée")
        } else {
            fail(StatusCode::NOT_FOUND, "pièce introuvable")
        };
    }
    let Some(run) = get_item(state, org, project, "test/runs", run_id) else {
        return fail(StatusCode::NOT_FOUND, "exécution introuvable");
    };
    let stored_run = value_key(&run["id"]);
    let run_name = text_of(&run, "name").unwrap_or_default();
    let bucket = format!("test/runs/{stored_run}/attachments");
    if attachment_tail.is_empty() {
        if method == Method::GET {
            let mut rows = list(state, org, project, &bucket);
            rows.sort_by(|left, right| value_key(&left["id"]).cmp(&value_key(&right["id"])));
            let value = rows.into_iter().map(|item| dress_run_attachment(org, project, &stored_run, &run_name, item)).collect();
            return Json(page(value)).into_response();
        }
        if method == Method::POST {
            let parsed = match run_attachment_payload(body) {
                Ok(parsed) => parsed,
                Err(message) => return fail(StatusCode::BAD_REQUEST, message),
            };
            return match run_attachment_create(state, org, project, &stored_run, &run_name, &bucket, parsed) {
                Ok(item) => Json(item).into_response(),
                Err(message) => fail(StatusCode::BAD_REQUEST, message),
            };
        }
        return fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée");
    }
    let key = pct_decode(&attachment_tail[0]);
    if method == Method::GET {
        return match locate_run_attachment(state, org, project, &bucket, key.trim()) {
            Some(found) => Json(dress_run_attachment(org, project, &stored_run, &run_name, found)).into_response(),
            None => fail(StatusCode::NOT_FOUND, "pièce introuvable"),
        };
    }
    if method == Method::DELETE {
        let Some(found) = locate_run_attachment(state, org, project, &bucket, key.trim()) else {
            return fail(StatusCode::NOT_FOUND, "pièce introuvable");
        };
        let stored_id = value_key(&found["id"]);
        let removed = delete_item(state, org, project, &bucket, &stored_id);
        return if removed {
            StatusCode::NO_CONTENT.into_response()
        } else {
            fail(StatusCode::NOT_FOUND, "pièce introuvable")
        };
    }
    fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée")
}

fn locate_run_attachment(state: &AppState, org: &str, project: &str, bucket: &str, key: &str) -> Option<Value> {
    list(state, org, project, bucket)
        .into_iter()
        .find(|item| value_key(&item["id"]) == key || text_of(item, "fileName").is_some_and(|stored| stored.eq_ignore_ascii_case(key)))
}

fn next_run_attachment_id(state: &AppState, org: &str) -> i64 {
    state
        .read(|db| {
            let Some(organization) = db.orgs.get(org) else {
                return Vec::new();
            };
            let mut rows = Vec::new();
            for project in organization.projects.values() {
                for (key, bucket) in &project.buckets {
                    if key.contains("/attachments") && key.starts_with("test/runs/") {
                        rows.extend(bucket.items.values().cloned());
                    }
                }
            }
            rows
        })
        .iter()
        .filter_map(|item| value_key(&item["id"]).strip_prefix("ta-")?.parse::<i64>().ok())
        .max()
        .unwrap_or(0)
        + 1
}

fn run_attachment_create(
    state: &AppState,
    org: &str,
    project: &str,
    run_id: &str,
    run_name: &str,
    bucket: &str,
    parsed: (String, String, String, Option<String>),
) -> Result<Value, &'static str> {
    let (file_name, comment, attachment_type, stream) = parsed;
    if list(state, org, project, bucket)
        .iter()
        .any(|item| text_of(item, "fileName").is_some_and(|stored| stored.eq_ignore_ascii_case(&file_name)))
    {
        return Err("pièce existante");
    }
    let id = format!("ta-{}", next_run_attachment_id(state, org));
    let mut item = json!({
        "id": id,
        "fileName": file_name,
        "comment": comment,
        "attachmentType": attachment_type,
        "runId": run_id
    });
    if let Some(stream) = stream {
        item["stream"] = json!(stream);
    }
    put_item(state, org, project, bucket, item.clone());
    Ok(dress_run_attachment(org, project, run_id, run_name, item))
}

fn run_attachment_payload(body: &[u8]) -> Result<(String, String, String, Option<String>), &'static str> {
    let Some(payload) = parse_json(body) else {
        return Err("JSON illisible");
    };
    let Some(object) = payload.as_object() else {
        return Err("pièce illisible");
    };
    let file_name = match object.get("fileName") {
        Some(Value::String(value)) => {
            let value = value.trim();
            if value.is_empty() {
                return Err("nom absent");
            }
            value.to_string()
        }
        Some(_) => return Err("nom absent"),
        None => return Err("nom absent"),
    };
    let comment = match object.get("comment") {
        None => String::new(),
        Some(Value::String(value)) => value.trim().to_string(),
        Some(_) => return Err("commentaire absent"),
    };
    let attachment_type = match object.get("attachmentType") {
        None => "GeneralAttachment".to_string(),
        Some(Value::String(value)) => {
            let value = value.trim();
            if value.is_empty() {
                return Err("type absent");
            }
            value.to_string()
        }
        Some(_) => return Err("type absent"),
    };
    let stream = match object.get("stream") {
        None => None,
        Some(Value::String(value)) if !value.is_empty() => Some(value.clone()),
        Some(_) => return Err("contenu absent"),
    };
    Ok((file_name, comment, attachment_type, stream))
}

fn dress_run_attachment(org: &str, project: &str, run_id: &str, run_name: &str, mut item: Value) -> Value {
    let id = value_key(&item["id"]);
    if let Some(object) = item.as_object_mut() {
        object.insert("runName".into(), json!(run_name));
        object.insert("project".into(), json!(project));
        object.insert("url".into(), json!(format!("/{org}/{project}/_apis/test/runs/{run_id}/attachments/{id}")));
    }
    item
}

fn result_attachments(
    state: &AppState,
    method: &Method,
    org: &str,
    project: &str,
    run_id: &str,
    rest: &[String],
    body: &[u8],
) -> Response {
    let attachment_tail = if rest.len() > 6 { &rest[6..] } else { &[] };
    if attachment_tail.len() > 1 {
        return if method == Method::DELETE {
            fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée")
        } else {
            fail(StatusCode::NOT_FOUND, "pièce introuvable")
        };
    }
    let Some(run) = get_item(state, org, project, "test/runs", run_id) else {
        return fail(StatusCode::NOT_FOUND, "exécution introuvable");
    };
    let stored_run = value_key(&run["id"]);
    let result_key = pct_decode(rest.get(4).map(String::as_str).unwrap_or(""));
    let results_key = format!("test/runs/{stored_run}/results");
    let Some(result) = get_item(state, org, project, &results_key, result_key.trim()) else {
        return fail(StatusCode::NOT_FOUND, "résultat introuvable");
    };
    let stored_result = value_key(&result["id"]);
    let result_name = text_of(&result, "testCaseTitle")
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| stored_result.clone());
    let bucket = format!("test/runs/{stored_run}/results/{stored_result}/attachments");
    if attachment_tail.is_empty() {
        if method == Method::GET {
            let mut rows = list(state, org, project, &bucket);
            rows.sort_by(|left, right| value_key(&left["id"]).cmp(&value_key(&right["id"])));
            let value = rows
                .into_iter()
                .map(|item| dress_result_attachment(org, project, &stored_run, &stored_result, &result_name, item))
                .collect();
            return Json(page(value)).into_response();
        }
        if method == Method::POST {
            let parsed = match run_attachment_payload(body) {
                Ok(parsed) => parsed,
                Err(message) => return fail(StatusCode::BAD_REQUEST, message),
            };
            return match result_attachment_create(state, org, project, &stored_run, &stored_result, &result_name, &bucket, parsed) {
                Ok(item) => Json(item).into_response(),
                Err(message) => fail(StatusCode::BAD_REQUEST, message),
            };
        }
        return fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée");
    }
    let key = pct_decode(&attachment_tail[0]);
    if method == Method::GET {
        return match locate_run_attachment(state, org, project, &bucket, key.trim()) {
            Some(found) => Json(dress_result_attachment(org, project, &stored_run, &stored_result, &result_name, found)).into_response(),
            None => fail(StatusCode::NOT_FOUND, "pièce introuvable"),
        };
    }
    if method == Method::DELETE {
        let Some(found) = locate_run_attachment(state, org, project, &bucket, key.trim()) else {
            return fail(StatusCode::NOT_FOUND, "pièce introuvable");
        };
        let stored_id = value_key(&found["id"]);
        let removed = delete_item(state, org, project, &bucket, &stored_id);
        return if removed {
            StatusCode::NO_CONTENT.into_response()
        } else {
            fail(StatusCode::NOT_FOUND, "pièce introuvable")
        };
    }
    fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée")
}

fn next_result_attachment_id(state: &AppState, org: &str) -> i64 {
    state
        .read(|db| {
            let Some(organization) = db.orgs.get(org) else {
                return Vec::new();
            };
            let mut rows = Vec::new();
            for project in organization.projects.values() {
                for (key, bucket) in &project.buckets {
                    if key.starts_with("test/runs/") && key.contains("/results/") && key.ends_with("/attachments") {
                        rows.extend(bucket.items.values().cloned());
                    }
                }
            }
            rows
        })
        .iter()
        .filter_map(|item| value_key(&item["id"]).strip_prefix("ra-")?.parse::<i64>().ok())
        .max()
        .unwrap_or(0)
        + 1
}

fn result_attachment_create(
    state: &AppState,
    org: &str,
    project: &str,
    run_id: &str,
    result_id: &str,
    result_name: &str,
    bucket: &str,
    parsed: (String, String, String, Option<String>),
) -> Result<Value, &'static str> {
    let (file_name, comment, attachment_type, stream) = parsed;
    if list(state, org, project, bucket)
        .iter()
        .any(|item| text_of(item, "fileName").is_some_and(|stored| stored.eq_ignore_ascii_case(&file_name)))
    {
        return Err("pièce existante");
    }
    let id = format!("ra-{}", next_result_attachment_id(state, org));
    let mut item = json!({
        "id": id,
        "fileName": file_name,
        "comment": comment,
        "attachmentType": attachment_type,
        "runId": run_id,
        "resultId": result_id
    });
    if let Some(stream) = stream {
        item["stream"] = json!(stream);
    }
    put_item(state, org, project, bucket, item.clone());
    Ok(dress_result_attachment(org, project, run_id, result_id, result_name, item))
}

fn dress_result_attachment(
    org: &str,
    project: &str,
    run_id: &str,
    result_id: &str,
    result_name: &str,
    mut item: Value,
) -> Value {
    let id = value_key(&item["id"]);
    if let Some(object) = item.as_object_mut() {
        object.insert("runId".into(), json!(run_id));
        object.insert("resultId".into(), json!(result_id));
        object.insert("resultName".into(), json!(result_name));
        object.insert("project".into(), json!(project));
        object.insert(
            "url".into(),
            json!(format!("/{org}/{project}/_apis/test/runs/{run_id}/results/{result_id}/attachments/{id}")),
        );
    }
    item
}

fn result_bugs(
    state: &AppState,
    method: &Method,
    org: &str,
    project: &str,
    run_id: &str,
    rest: &[String],
    body: &[u8],
) -> Response {
    let bug_tail = if rest.len() > 6 { &rest[6..] } else { &[] };
    if bug_tail.len() > 1 {
        return if method == Method::DELETE {
            fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée")
        } else {
            fail(StatusCode::NOT_FOUND, "bogue introuvable")
        };
    }
    let Some(run) = get_item(state, org, project, "test/runs", run_id) else {
        return fail(StatusCode::NOT_FOUND, "exécution introuvable");
    };
    let stored_run = value_key(&run["id"]);
    let result_key = pct_decode(rest.get(4).map(String::as_str).unwrap_or(""));
    let results_key = format!("test/runs/{stored_run}/results");
    let Some(result) = get_item(state, org, project, &results_key, result_key.trim()) else {
        return fail(StatusCode::NOT_FOUND, "résultat introuvable");
    };
    let stored_result = value_key(&result["id"]);
    let result_name = text_of(&result, "testCaseTitle")
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| stored_result.clone());
    let bucket = format!("test/runs/{stored_run}/results/{stored_result}/bugs");
    if bug_tail.is_empty() {
        if method == Method::GET {
            let mut rows = list(state, org, project, &bucket);
            rows.sort_by(|left, right| value_key(&left["id"]).cmp(&value_key(&right["id"])));
            let value = rows
                .into_iter()
                .map(|item| dress_result_bug(org, project, &stored_run, &stored_result, &result_name, item))
                .collect();
            return Json(page(value)).into_response();
        }
        if method == Method::POST {
            let parsed = match result_bug_payload(body) {
                Ok(parsed) => parsed,
                Err(message) => return fail(StatusCode::BAD_REQUEST, message),
            };
            return match result_bug_create(state, org, project, &stored_run, &stored_result, &result_name, &bucket, parsed) {
                Ok(item) => Json(item).into_response(),
                Err(message) => fail(StatusCode::BAD_REQUEST, message),
            };
        }
        return fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée");
    }
    let key = pct_decode(&bug_tail[0]);
    if method == Method::GET {
        return match locate_result_bug(state, org, project, &bucket, key.trim()) {
            Some(found) => Json(dress_result_bug(org, project, &stored_run, &stored_result, &result_name, found)).into_response(),
            None => fail(StatusCode::NOT_FOUND, "bogue introuvable"),
        };
    }
    if method == Method::DELETE {
        let Some(found) = locate_result_bug(state, org, project, &bucket, key.trim()) else {
            return fail(StatusCode::NOT_FOUND, "bogue introuvable");
        };
        let stored_id = value_key(&found["id"]);
        let removed = delete_item(state, org, project, &bucket, &stored_id);
        return if removed {
            StatusCode::NO_CONTENT.into_response()
        } else {
            fail(StatusCode::NOT_FOUND, "bogue introuvable")
        };
    }
    fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée")
}

fn locate_result_bug(state: &AppState, org: &str, project: &str, bucket: &str, key: &str) -> Option<Value> {
    list(state, org, project, bucket).into_iter().find(|item| {
        value_key(&item["id"]) == key || text_of(item, "name").is_some_and(|stored| stored.eq_ignore_ascii_case(key))
    })
}

fn next_result_bug_id(state: &AppState, org: &str) -> i64 {
    state
        .read(|db| {
            let Some(organization) = db.orgs.get(org) else {
                return Vec::new();
            };
            let mut rows = Vec::new();
            for project in organization.projects.values() {
                for (key, bucket) in &project.buckets {
                    if key.starts_with("test/runs/") && key.contains("/results/") && key.ends_with("/bugs") {
                        rows.extend(bucket.items.values().cloned());
                    }
                }
            }
            rows
        })
        .iter()
        .filter_map(|item| value_key(&item["id"]).strip_prefix("rb-")?.parse::<i64>().ok())
        .max()
        .unwrap_or(0)
        + 1
}

fn result_bug_payload(body: &[u8]) -> Result<(String, String), &'static str> {
    let Some(payload) = parse_json(body) else {
        return Err("JSON illisible");
    };
    let Some(object) = payload.as_object() else {
        return Err("bogue illisible");
    };
    let name = match object.get("name") {
        Some(Value::String(value)) => {
            let value = value.trim();
            if value.is_empty() {
                return Err("nom absent");
            }
            value.to_string()
        }
        Some(_) => return Err("nom absent"),
        None => return Err("nom absent"),
    };
    let comment = match object.get("comment") {
        None => String::new(),
        Some(Value::String(value)) => value.trim().to_string(),
        Some(_) => return Err("commentaire absent"),
    };
    Ok((name, comment))
}

fn result_bug_create(
    state: &AppState,
    org: &str,
    project: &str,
    run_id: &str,
    result_id: &str,
    result_name: &str,
    bucket: &str,
    parsed: (String, String),
) -> Result<Value, &'static str> {
    let (name, comment) = parsed;
    if list(state, org, project, bucket)
        .iter()
        .any(|item| text_of(item, "name").is_some_and(|stored| stored.eq_ignore_ascii_case(&name)))
    {
        return Err("bogue existant");
    }
    let id = format!("rb-{}", next_result_bug_id(state, org));
    let item = json!({
        "id": id,
        "name": name,
        "comment": comment,
        "runId": run_id,
        "resultId": result_id
    });
    put_item(state, org, project, bucket, item.clone());
    Ok(dress_result_bug(org, project, run_id, result_id, result_name, item))
}

fn dress_result_bug(org: &str, project: &str, run_id: &str, result_id: &str, result_name: &str, mut item: Value) -> Value {
    let id = value_key(&item["id"]);
    if let Some(object) = item.as_object_mut() {
        object.insert("runId".into(), json!(run_id));
        object.insert("resultId".into(), json!(result_id));
        object.insert("resultName".into(), json!(result_name));
        object.insert("project".into(), json!(project));
        object.insert(
            "url".into(),
            json!(format!("/{org}/{project}/_apis/test/runs/{run_id}/results/{result_id}/bugs/{id}")),
        );
    }
    item
}

fn result_iterations(
    state: &AppState,
    method: &Method,
    org: &str,
    project: &str,
    run_id: &str,
    rest: &[String],
    body: &[u8],
) -> Response {
    let iteration_tail = if rest.len() > 6 { &rest[6..] } else { &[] };
    if iteration_tail.len() > 1 {
        return if method == Method::DELETE {
            fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée")
        } else {
            fail(StatusCode::NOT_FOUND, "itération introuvable")
        };
    }
    let Some(run) = get_item(state, org, project, "test/runs", run_id) else {
        return fail(StatusCode::NOT_FOUND, "exécution introuvable");
    };
    let stored_run = value_key(&run["id"]);
    let result_key = pct_decode(rest.get(4).map(String::as_str).unwrap_or(""));
    let results_key = format!("test/runs/{stored_run}/results");
    let Some(result) = get_item(state, org, project, &results_key, result_key.trim()) else {
        return fail(StatusCode::NOT_FOUND, "résultat introuvable");
    };
    let stored_result = value_key(&result["id"]);
    let result_name = text_of(&result, "testCaseTitle")
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| stored_result.clone());
    let bucket = format!("test/runs/{stored_run}/results/{stored_result}/iterations");
    if iteration_tail.is_empty() {
        if method == Method::GET {
            let mut rows = list(state, org, project, &bucket);
            rows.sort_by(|left, right| value_key(&left["id"]).cmp(&value_key(&right["id"])));
            let value = rows
                .into_iter()
                .map(|item| dress_result_iteration(org, project, &stored_run, &stored_result, &result_name, item))
                .collect();
            return Json(page(value)).into_response();
        }
        if method == Method::POST {
            let parsed = match result_iteration_payload(body) {
                Ok(parsed) => parsed,
                Err(message) => return fail(StatusCode::BAD_REQUEST, message),
            };
            return match result_iteration_create(state, org, project, &stored_run, &stored_result, &result_name, &bucket, parsed)
            {
                Ok(item) => Json(item).into_response(),
                Err(message) => fail(StatusCode::BAD_REQUEST, message),
            };
        }
        return fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée");
    }
    let key = pct_decode(&iteration_tail[0]);
    if method == Method::GET {
        return match locate_result_iteration(state, org, project, &bucket, key.trim()) {
            Some(found) => Json(dress_result_iteration(org, project, &stored_run, &stored_result, &result_name, found)).into_response(),
            None => fail(StatusCode::NOT_FOUND, "itération introuvable"),
        };
    }
    if method == Method::DELETE {
        let Some(found) = locate_result_iteration(state, org, project, &bucket, key.trim()) else {
            return fail(StatusCode::NOT_FOUND, "itération introuvable");
        };
        let stored_id = value_key(&found["id"]);
        let removed = delete_item(state, org, project, &bucket, &stored_id);
        if removed {
            let child_prefix = format!("test/runs/{stored_run}/results/{stored_result}/iterations/{stored_id}/");
            state.write(|db| {
                if let Some(stored) = db.project_mut(org, project) {
                    stored.buckets.retain(|key, _| !key.starts_with(&child_prefix));
                }
            });
            return StatusCode::NO_CONTENT.into_response();
        }
        return fail(StatusCode::NOT_FOUND, "itération introuvable");
    }
    fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée")
}

fn locate_result_iteration(state: &AppState, org: &str, project: &str, bucket: &str, key: &str) -> Option<Value> {
    list(state, org, project, bucket).into_iter().find(|item| {
        value_key(&item["id"]) == key || text_of(item, "name").is_some_and(|stored| stored.eq_ignore_ascii_case(key))
    })
}

fn next_result_iteration_id(state: &AppState, org: &str) -> i64 {
    state
        .read(|db| {
            let Some(organization) = db.orgs.get(org) else {
                return Vec::new();
            };
            let mut rows = Vec::new();
            for project in organization.projects.values() {
                for (key, bucket) in &project.buckets {
                    if key.starts_with("test/runs/") && key.contains("/results/") && key.ends_with("/iterations") {
                        rows.extend(bucket.items.values().cloned());
                    }
                }
            }
            rows
        })
        .iter()
        .filter_map(|item| value_key(&item["id"]).strip_prefix("ri-")?.parse::<i64>().ok())
        .max()
        .unwrap_or(0)
        + 1
}

fn result_iteration_payload(body: &[u8]) -> Result<(String, String, String), &'static str> {
    let Some(payload) = parse_json(body) else {
        return Err("JSON illisible");
    };
    let Some(object) = payload.as_object() else {
        return Err("itération illisible");
    };
    let name = match object.get("name") {
        Some(Value::String(value)) => {
            let value = value.trim();
            if value.is_empty() {
                return Err("nom absent");
            }
            value.to_string()
        }
        Some(_) => return Err("nom absent"),
        None => return Err("nom absent"),
    };
    let outcome = match object.get("outcome") {
        None => "Passed".to_string(),
        Some(Value::String(value)) => {
            let value = value.trim();
            if value.is_empty() {
                return Err("issue absente");
            }
            value.to_string()
        }
        Some(_) => return Err("issue absente"),
    };
    let comment = match object.get("comment") {
        None => String::new(),
        Some(Value::String(value)) => value.trim().to_string(),
        Some(_) => return Err("commentaire absent"),
    };
    Ok((name, outcome, comment))
}

fn result_iteration_create(
    state: &AppState,
    org: &str,
    project: &str,
    run_id: &str,
    result_id: &str,
    result_name: &str,
    bucket: &str,
    parsed: (String, String, String),
) -> Result<Value, &'static str> {
    let (name, outcome, comment) = parsed;
    if list(state, org, project, bucket)
        .iter()
        .any(|item| text_of(item, "name").is_some_and(|stored| stored.eq_ignore_ascii_case(&name)))
    {
        return Err("itération existante");
    }
    let id = format!("ri-{}", next_result_iteration_id(state, org));
    let item = json!({
        "id": id,
        "name": name,
        "outcome": outcome,
        "comment": comment,
        "runId": run_id,
        "resultId": result_id
    });
    put_item(state, org, project, bucket, item.clone());
    Ok(dress_result_iteration(org, project, run_id, result_id, result_name, item))
}

fn dress_result_iteration(
    org: &str,
    project: &str,
    run_id: &str,
    result_id: &str,
    result_name: &str,
    mut item: Value,
) -> Value {
    let id = value_key(&item["id"]);
    if let Some(object) = item.as_object_mut() {
        object.insert("runId".into(), json!(run_id));
        object.insert("resultId".into(), json!(result_id));
        object.insert("resultName".into(), json!(result_name));
        object.insert("project".into(), json!(project));
        object.insert(
            "url".into(),
            json!(format!("/{org}/{project}/_apis/test/runs/{run_id}/results/{result_id}/iterations/{id}")),
        );
    }
    item
}

fn result_actions(
    state: &AppState,
    method: &Method,
    org: &str,
    project: &str,
    run_id: &str,
    rest: &[String],
    body: &[u8],
) -> Response {
    let action_tail = if rest.len() > 8 { &rest[8..] } else { &[] };
    if action_tail.len() > 1 {
        return if method == Method::DELETE {
            fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée")
        } else {
            fail(StatusCode::NOT_FOUND, "action introuvable")
        };
    }
    let Some(run) = get_item(state, org, project, "test/runs", run_id) else {
        return fail(StatusCode::NOT_FOUND, "exécution introuvable");
    };
    let stored_run = value_key(&run["id"]);
    let result_key = pct_decode(rest.get(4).map(String::as_str).unwrap_or(""));
    let results_key = format!("test/runs/{stored_run}/results");
    let Some(result) = get_item(state, org, project, &results_key, result_key.trim()) else {
        return fail(StatusCode::NOT_FOUND, "résultat introuvable");
    };
    let stored_result = value_key(&result["id"]);
    let result_name = text_of(&result, "testCaseTitle")
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| stored_result.clone());
    let iteration_key = pct_decode(rest.get(6).map(String::as_str).unwrap_or(""));
    let iterations_key = format!("test/runs/{stored_run}/results/{stored_result}/iterations");
    let Some(iteration) = get_item(state, org, project, &iterations_key, iteration_key.trim()) else {
        return fail(StatusCode::NOT_FOUND, "itération introuvable");
    };
    let stored_iteration = value_key(&iteration["id"]);
    let iteration_name = text_of(&iteration, "name")
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| stored_iteration.clone());
    let bucket = format!("test/runs/{stored_run}/results/{stored_result}/iterations/{stored_iteration}/actionresults");
    if action_tail.is_empty() {
        if method == Method::GET {
            let mut rows = list(state, org, project, &bucket);
            rows.sort_by(|left, right| value_key(&left["id"]).cmp(&value_key(&right["id"])));
            let value = rows
                .into_iter()
                .map(|item| dress_result_action(org, project, &stored_run, &stored_result, &result_name, &stored_iteration, &iteration_name, item))
                .collect();
            return Json(page(value)).into_response();
        }
        if method == Method::POST {
            let parsed = match result_action_payload(body) {
                Ok(parsed) => parsed,
                Err(message) => return fail(StatusCode::BAD_REQUEST, message),
            };
            return match result_action_create(
                state,
                org,
                project,
                &stored_run,
                &stored_result,
                &result_name,
                &stored_iteration,
                &iteration_name,
                &bucket,
                parsed,
            ) {
                Ok(item) => Json(item).into_response(),
                Err(message) => fail(StatusCode::BAD_REQUEST, message),
            };
        }
        return fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée");
    }
    let key = pct_decode(&action_tail[0]);
    if method == Method::GET {
        return match locate_result_action(state, org, project, &bucket, key.trim()) {
            Some(found) => {
                Json(dress_result_action(org, project, &stored_run, &stored_result, &result_name, &stored_iteration, &iteration_name, found)).into_response()
            }
            None => fail(StatusCode::NOT_FOUND, "action introuvable"),
        };
    }
    if method == Method::DELETE {
        let Some(found) = locate_result_action(state, org, project, &bucket, key.trim()) else {
            return fail(StatusCode::NOT_FOUND, "action introuvable");
        };
        let stored_id = value_key(&found["id"]);
        let removed = delete_item(state, org, project, &bucket, &stored_id);
        if removed {
            let child_prefix = format!("{bucket}/{stored_id}/");
            state.write(|db| {
                if let Some(stored) = db.project_mut(org, project) {
                    stored.buckets.retain(|key, _| !key.starts_with(&child_prefix));
                }
            });
            return StatusCode::NO_CONTENT.into_response();
        }
        return fail(StatusCode::NOT_FOUND, "action introuvable");
    }
    fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée")
}

fn locate_result_action(state: &AppState, org: &str, project: &str, bucket: &str, key: &str) -> Option<Value> {
    list(state, org, project, bucket).into_iter().find(|item| {
        value_key(&item["id"]) == key || text_of(item, "name").is_some_and(|stored| stored.eq_ignore_ascii_case(key))
    })
}

fn next_result_action_id(state: &AppState, org: &str) -> i64 {
    state
        .read(|db| {
            let Some(organization) = db.orgs.get(org) else {
                return Vec::new();
            };
            let mut rows = Vec::new();
            for project in organization.projects.values() {
                for (key, bucket) in &project.buckets {
                    if key.starts_with("test/runs/") && key.contains("/iterations/") && key.ends_with("/actionresults") {
                        rows.extend(bucket.items.values().cloned());
                    }
                }
            }
            rows
        })
        .iter()
        .filter_map(|item| value_key(&item["id"]).strip_prefix("ar-")?.parse::<i64>().ok())
        .max()
        .unwrap_or(0)
        + 1
}

fn result_action_payload(body: &[u8]) -> Result<(String, String, String), &'static str> {
    let Some(payload) = parse_json(body) else {
        return Err("JSON illisible");
    };
    let Some(object) = payload.as_object() else {
        return Err("action illisible");
    };
    let name = match object.get("name") {
        Some(Value::String(value)) => {
            let value = value.trim();
            if value.is_empty() {
                return Err("nom absent");
            }
            value.to_string()
        }
        Some(_) => return Err("nom absent"),
        None => return Err("nom absent"),
    };
    let outcome = match object.get("outcome") {
        None => "Passed".to_string(),
        Some(Value::String(value)) => {
            let value = value.trim();
            if value.is_empty() {
                return Err("issue absente");
            }
            value.to_string()
        }
        Some(_) => return Err("issue absente"),
    };
    let comment = match object.get("comment") {
        None => String::new(),
        Some(Value::String(value)) => value.trim().to_string(),
        Some(_) => return Err("commentaire absent"),
    };
    Ok((name, outcome, comment))
}

fn result_action_create(
    state: &AppState,
    org: &str,
    project: &str,
    run_id: &str,
    result_id: &str,
    result_name: &str,
    iteration_id: &str,
    iteration_name: &str,
    bucket: &str,
    parsed: (String, String, String),
) -> Result<Value, &'static str> {
    let (name, outcome, comment) = parsed;
    if list(state, org, project, bucket)
        .iter()
        .any(|item| text_of(item, "name").is_some_and(|stored| stored.eq_ignore_ascii_case(&name)))
    {
        return Err("action existante");
    }
    let id = format!("ar-{}", next_result_action_id(state, org));
    let item = json!({
        "id": id,
        "name": name,
        "outcome": outcome,
        "comment": comment,
        "runId": run_id,
        "resultId": result_id,
        "iterationId": iteration_id
    });
    put_item(state, org, project, bucket, item.clone());
    Ok(dress_result_action(org, project, run_id, result_id, result_name, iteration_id, iteration_name, item))
}

fn dress_result_action(
    org: &str,
    project: &str,
    run_id: &str,
    result_id: &str,
    result_name: &str,
    iteration_id: &str,
    iteration_name: &str,
    mut item: Value,
) -> Value {
    let id = value_key(&item["id"]);
    if let Some(object) = item.as_object_mut() {
        object.insert("runId".into(), json!(run_id));
        object.insert("resultId".into(), json!(result_id));
        object.insert("resultName".into(), json!(result_name));
        object.insert("iterationId".into(), json!(iteration_id));
        object.insert("iterationName".into(), json!(iteration_name));
        object.insert("project".into(), json!(project));
        object.insert(
            "url".into(),
            json!(format!("/{org}/{project}/_apis/test/runs/{run_id}/results/{result_id}/iterations/{iteration_id}/actionresults/{id}")),
        );
    }
    item
}

fn iteration_attachments(
    state: &AppState,
    method: &Method,
    org: &str,
    project: &str,
    run_id: &str,
    rest: &[String],
    body: &[u8],
) -> Response {
    let file_tail = if rest.len() > 8 { &rest[8..] } else { &[] };
    if file_tail.len() > 1 {
        return if method == Method::DELETE {
            fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée")
        } else {
            fail(StatusCode::NOT_FOUND, "pièce introuvable")
        };
    }
    let Some(run) = get_item(state, org, project, "test/runs", run_id) else {
        return fail(StatusCode::NOT_FOUND, "exécution introuvable");
    };
    let stored_run = value_key(&run["id"]);
    let result_key = pct_decode(rest.get(4).map(String::as_str).unwrap_or(""));
    let results_key = format!("test/runs/{stored_run}/results");
    let Some(result) = get_item(state, org, project, &results_key, result_key.trim()) else {
        return fail(StatusCode::NOT_FOUND, "résultat introuvable");
    };
    let stored_result = value_key(&result["id"]);
    let result_name = text_of(&result, "testCaseTitle")
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| stored_result.clone());
    let iteration_key = pct_decode(rest.get(6).map(String::as_str).unwrap_or(""));
    let iterations_key = format!("test/runs/{stored_run}/results/{stored_result}/iterations");
    let Some(iteration) = get_item(state, org, project, &iterations_key, iteration_key.trim()) else {
        return fail(StatusCode::NOT_FOUND, "itération introuvable");
    };
    let stored_iteration = value_key(&iteration["id"]);
    let iteration_name = text_of(&iteration, "name")
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| stored_iteration.clone());
    let bucket = format!("test/runs/{stored_run}/results/{stored_result}/iterations/{stored_iteration}/attachments");
    if file_tail.is_empty() {
        if method == Method::GET {
            let mut rows = list(state, org, project, &bucket);
            rows.sort_by(|left, right| value_key(&left["id"]).cmp(&value_key(&right["id"])));
            let value = rows
                .into_iter()
                .map(|item| dress_iteration_attachment(org, project, &stored_run, &stored_result, &result_name, &stored_iteration, &iteration_name, item))
                .collect();
            return Json(page(value)).into_response();
        }
        if method == Method::POST {
            let parsed = match run_attachment_payload(body) {
                Ok(parsed) => parsed,
                Err(message) => return fail(StatusCode::BAD_REQUEST, message),
            };
            return match iteration_attachment_create(
                state,
                org,
                project,
                &stored_run,
                &stored_result,
                &result_name,
                &stored_iteration,
                &iteration_name,
                &bucket,
                parsed,
            ) {
                Ok(item) => Json(item).into_response(),
                Err(message) => fail(StatusCode::BAD_REQUEST, message),
            };
        }
        return fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée");
    }
    let key = pct_decode(&file_tail[0]);
    if method == Method::GET {
        return match locate_run_attachment(state, org, project, &bucket, key.trim()) {
            Some(found) => Json(dress_iteration_attachment(
                org,
                project,
                &stored_run,
                &stored_result,
                &result_name,
                &stored_iteration,
                &iteration_name,
                found,
            ))
            .into_response(),
            None => fail(StatusCode::NOT_FOUND, "pièce introuvable"),
        };
    }
    if method == Method::DELETE {
        let Some(found) = locate_run_attachment(state, org, project, &bucket, key.trim()) else {
            return fail(StatusCode::NOT_FOUND, "pièce introuvable");
        };
        let stored_id = value_key(&found["id"]);
        let removed = delete_item(state, org, project, &bucket, &stored_id);
        return if removed {
            StatusCode::NO_CONTENT.into_response()
        } else {
            fail(StatusCode::NOT_FOUND, "pièce introuvable")
        };
    }
    fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée")
}

fn next_iteration_attachment_id(state: &AppState, org: &str) -> i64 {
    state
        .read(|db| {
            let Some(organization) = db.orgs.get(org) else {
                return Vec::new();
            };
            let mut rows = Vec::new();
            for project in organization.projects.values() {
                for (key, bucket) in &project.buckets {
                    if key.starts_with("test/runs/") && key.contains("/iterations/") && key.ends_with("/attachments") {
                        rows.extend(bucket.items.values().cloned());
                    }
                }
            }
            rows
        })
        .iter()
        .filter_map(|item| value_key(&item["id"]).strip_prefix("ia-")?.parse::<i64>().ok())
        .max()
        .unwrap_or(0)
        + 1
}

fn iteration_attachment_create(
    state: &AppState,
    org: &str,
    project: &str,
    run_id: &str,
    result_id: &str,
    result_name: &str,
    iteration_id: &str,
    iteration_name: &str,
    bucket: &str,
    parsed: (String, String, String, Option<String>),
) -> Result<Value, &'static str> {
    let (file_name, comment, attachment_type, stream) = parsed;
    if list(state, org, project, bucket)
        .iter()
        .any(|item| text_of(item, "fileName").is_some_and(|stored| stored.eq_ignore_ascii_case(&file_name)))
    {
        return Err("pièce existante");
    }
    let id = format!("ia-{}", next_iteration_attachment_id(state, org));
    let mut item = json!({
        "id": id,
        "fileName": file_name,
        "comment": comment,
        "attachmentType": attachment_type,
        "runId": run_id,
        "resultId": result_id,
        "iterationId": iteration_id
    });
    if let Some(stream) = stream {
        item["stream"] = json!(stream);
    }
    put_item(state, org, project, bucket, item.clone());
    Ok(dress_iteration_attachment(org, project, run_id, result_id, result_name, iteration_id, iteration_name, item))
}

fn dress_iteration_attachment(
    org: &str,
    project: &str,
    run_id: &str,
    result_id: &str,
    result_name: &str,
    iteration_id: &str,
    iteration_name: &str,
    mut item: Value,
) -> Value {
    let id = value_key(&item["id"]);
    if let Some(object) = item.as_object_mut() {
        object.insert("runId".into(), json!(run_id));
        object.insert("resultId".into(), json!(result_id));
        object.insert("resultName".into(), json!(result_name));
        object.insert("iterationId".into(), json!(iteration_id));
        object.insert("iterationName".into(), json!(iteration_name));
        object.insert("project".into(), json!(project));
        object.insert(
            "url".into(),
            json!(format!("/{org}/{project}/_apis/test/runs/{run_id}/results/{result_id}/iterations/{iteration_id}/attachments/{id}")),
        );
    }
    item
}

fn iteration_parameters(
    state: &AppState,
    method: &Method,
    org: &str,
    project: &str,
    run_id: &str,
    rest: &[String],
    body: &[u8],
) -> Response {
    let parameter_tail = if rest.len() > 8 { &rest[8..] } else { &[] };
    if parameter_tail.len() > 1 {
        return if method == Method::DELETE {
            fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée")
        } else {
            fail(StatusCode::NOT_FOUND, "paramètre introuvable")
        };
    }
    let Some(run) = get_item(state, org, project, "test/runs", run_id) else {
        return fail(StatusCode::NOT_FOUND, "exécution introuvable");
    };
    let stored_run = value_key(&run["id"]);
    let result_key = pct_decode(rest.get(4).map(String::as_str).unwrap_or(""));
    let results_key = format!("test/runs/{stored_run}/results");
    let Some(result) = get_item(state, org, project, &results_key, result_key.trim()) else {
        return fail(StatusCode::NOT_FOUND, "résultat introuvable");
    };
    let stored_result = value_key(&result["id"]);
    let result_name = text_of(&result, "testCaseTitle")
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| stored_result.clone());
    let iteration_key = pct_decode(rest.get(6).map(String::as_str).unwrap_or(""));
    let iterations_key = format!("test/runs/{stored_run}/results/{stored_result}/iterations");
    let Some(iteration) = get_item(state, org, project, &iterations_key, iteration_key.trim()) else {
        return fail(StatusCode::NOT_FOUND, "itération introuvable");
    };
    let stored_iteration = value_key(&iteration["id"]);
    let iteration_name = text_of(&iteration, "name")
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| stored_iteration.clone());
    let bucket = format!("test/runs/{stored_run}/results/{stored_result}/iterations/{stored_iteration}/parameterresults");
    if parameter_tail.is_empty() {
        if method == Method::GET {
            let mut rows = list(state, org, project, &bucket);
            rows.sort_by(|left, right| value_key(&left["id"]).cmp(&value_key(&right["id"])));
            let value = rows
                .into_iter()
                .map(|item| dress_iteration_parameter(org, project, &stored_run, &stored_result, &result_name, &stored_iteration, &iteration_name, item))
                .collect();
            return Json(page(value)).into_response();
        }
        if method == Method::POST {
            let parsed = match iteration_parameter_payload(body) {
                Ok(parsed) => parsed,
                Err(message) => return fail(StatusCode::BAD_REQUEST, message),
            };
            return match iteration_parameter_create(
                state,
                org,
                project,
                &stored_run,
                &stored_result,
                &result_name,
                &stored_iteration,
                &iteration_name,
                &bucket,
                parsed,
            ) {
                Ok(item) => Json(item).into_response(),
                Err(message) => fail(StatusCode::BAD_REQUEST, message),
            };
        }
        return fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée");
    }
    let key = pct_decode(&parameter_tail[0]);
    if method == Method::GET {
        return match locate_iteration_parameter(state, org, project, &bucket, key.trim()) {
            Some(found) => Json(dress_iteration_parameter(
                org,
                project,
                &stored_run,
                &stored_result,
                &result_name,
                &stored_iteration,
                &iteration_name,
                found,
            ))
            .into_response(),
            None => fail(StatusCode::NOT_FOUND, "paramètre introuvable"),
        };
    }
    if method == Method::DELETE {
        let Some(found) = locate_iteration_parameter(state, org, project, &bucket, key.trim()) else {
            return fail(StatusCode::NOT_FOUND, "paramètre introuvable");
        };
        let stored_id = value_key(&found["id"]);
        let removed = delete_item(state, org, project, &bucket, &stored_id);
        return if removed {
            StatusCode::NO_CONTENT.into_response()
        } else {
            fail(StatusCode::NOT_FOUND, "paramètre introuvable")
        };
    }
    fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée")
}

fn locate_iteration_parameter(state: &AppState, org: &str, project: &str, bucket: &str, key: &str) -> Option<Value> {
    list(state, org, project, bucket).into_iter().find(|item| {
        value_key(&item["id"]) == key || text_of(item, "name").is_some_and(|stored| stored.eq_ignore_ascii_case(key))
    })
}

fn next_iteration_parameter_id(state: &AppState, org: &str) -> i64 {
    state
        .read(|db| {
            let Some(organization) = db.orgs.get(org) else {
                return Vec::new();
            };
            let mut rows = Vec::new();
            for project in organization.projects.values() {
                for (key, bucket) in &project.buckets {
                    if key.starts_with("test/runs/") && key.contains("/iterations/") && key.ends_with("/parameterresults") {
                        rows.extend(bucket.items.values().cloned());
                    }
                }
            }
            rows
        })
        .iter()
        .filter_map(|item| value_key(&item["id"]).strip_prefix("ip-")?.parse::<i64>().ok())
        .max()
        .unwrap_or(0)
        + 1
}

fn iteration_parameter_payload(body: &[u8]) -> Result<(String, String), &'static str> {
    let Some(payload) = parse_json(body) else {
        return Err("JSON illisible");
    };
    let Some(object) = payload.as_object() else {
        return Err("paramètre illisible");
    };
    let name = match object.get("name") {
        Some(Value::String(value)) => {
            let value = value.trim();
            if value.is_empty() {
                return Err("nom absent");
            }
            value.to_string()
        }
        Some(_) => return Err("nom absent"),
        None => return Err("nom absent"),
    };
    let value = match object.get("value") {
        None => String::new(),
        Some(Value::String(value)) => value.trim().to_string(),
        Some(_) => return Err("valeur absente"),
    };
    Ok((name, value))
}

fn iteration_parameter_create(
    state: &AppState,
    org: &str,
    project: &str,
    run_id: &str,
    result_id: &str,
    result_name: &str,
    iteration_id: &str,
    iteration_name: &str,
    bucket: &str,
    parsed: (String, String),
) -> Result<Value, &'static str> {
    let (name, value) = parsed;
    if list(state, org, project, bucket)
        .iter()
        .any(|item| text_of(item, "name").is_some_and(|stored| stored.eq_ignore_ascii_case(&name)))
    {
        return Err("paramètre existant");
    }
    let id = format!("ip-{}", next_iteration_parameter_id(state, org));
    let item = json!({
        "id": id,
        "name": name,
        "value": value,
        "runId": run_id,
        "resultId": result_id,
        "iterationId": iteration_id
    });
    put_item(state, org, project, bucket, item.clone());
    Ok(dress_iteration_parameter(org, project, run_id, result_id, result_name, iteration_id, iteration_name, item))
}

fn dress_iteration_parameter(
    org: &str,
    project: &str,
    run_id: &str,
    result_id: &str,
    result_name: &str,
    iteration_id: &str,
    iteration_name: &str,
    mut item: Value,
) -> Value {
    let id = value_key(&item["id"]);
    if let Some(object) = item.as_object_mut() {
        object.insert("runId".into(), json!(run_id));
        object.insert("resultId".into(), json!(result_id));
        object.insert("resultName".into(), json!(result_name));
        object.insert("iterationId".into(), json!(iteration_id));
        object.insert("iterationName".into(), json!(iteration_name));
        object.insert("project".into(), json!(project));
        object.insert(
            "url".into(),
            json!(format!("/{org}/{project}/_apis/test/runs/{run_id}/results/{result_id}/iterations/{iteration_id}/parameterresults/{id}")),
        );
    }
    item
}

fn action_attachments(
    state: &AppState,
    method: &Method,
    org: &str,
    project: &str,
    run_id: &str,
    rest: &[String],
    body: &[u8],
) -> Response {
    let file_tail = if rest.len() > 10 { &rest[10..] } else { &[] };
    if file_tail.len() > 1 {
        return if method == Method::DELETE {
            fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée")
        } else {
            fail(StatusCode::NOT_FOUND, "pièce introuvable")
        };
    }
    let Some(run) = get_item(state, org, project, "test/runs", run_id) else {
        return fail(StatusCode::NOT_FOUND, "exécution introuvable");
    };
    let stored_run = value_key(&run["id"]);
    let result_key = pct_decode(rest.get(4).map(String::as_str).unwrap_or(""));
    let results_key = format!("test/runs/{stored_run}/results");
    let Some(result) = get_item(state, org, project, &results_key, result_key.trim()) else {
        return fail(StatusCode::NOT_FOUND, "résultat introuvable");
    };
    let stored_result = value_key(&result["id"]);
    let result_name = text_of(&result, "testCaseTitle")
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| stored_result.clone());
    let iteration_key = pct_decode(rest.get(6).map(String::as_str).unwrap_or(""));
    let iterations_key = format!("test/runs/{stored_run}/results/{stored_result}/iterations");
    let Some(iteration) = get_item(state, org, project, &iterations_key, iteration_key.trim()) else {
        return fail(StatusCode::NOT_FOUND, "itération introuvable");
    };
    let stored_iteration = value_key(&iteration["id"]);
    let iteration_name = text_of(&iteration, "name")
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| stored_iteration.clone());
    let action_key = pct_decode(rest.get(8).map(String::as_str).unwrap_or(""));
    let actions_key = format!("test/runs/{stored_run}/results/{stored_result}/iterations/{stored_iteration}/actionresults");
    let Some(action) = get_item(state, org, project, &actions_key, action_key.trim()) else {
        return fail(StatusCode::NOT_FOUND, "action introuvable");
    };
    let stored_action = value_key(&action["id"]);
    let action_name = text_of(&action, "name")
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| stored_action.clone());
    let bucket = format!("{actions_key}/{stored_action}/attachments");
    if file_tail.is_empty() {
        if method == Method::GET {
            let mut rows = list(state, org, project, &bucket);
            rows.sort_by(|left, right| value_key(&left["id"]).cmp(&value_key(&right["id"])));
            let value = rows
                .into_iter()
                .map(|item| {
                    dress_action_attachment(
                        org,
                        project,
                        &stored_run,
                        &stored_result,
                        &result_name,
                        &stored_iteration,
                        &iteration_name,
                        &stored_action,
                        &action_name,
                        item,
                    )
                })
                .collect();
            return Json(page(value)).into_response();
        }
        if method == Method::POST {
            let parsed = match run_attachment_payload(body) {
                Ok(parsed) => parsed,
                Err(message) => return fail(StatusCode::BAD_REQUEST, message),
            };
            return match action_attachment_create(
                state,
                org,
                project,
                &stored_run,
                &stored_result,
                &result_name,
                &stored_iteration,
                &iteration_name,
                &stored_action,
                &action_name,
                &bucket,
                parsed,
            ) {
                Ok(item) => Json(item).into_response(),
                Err(message) => fail(StatusCode::BAD_REQUEST, message),
            };
        }
        return fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée");
    }
    let key = pct_decode(&file_tail[0]);
    if method == Method::GET {
        return match locate_run_attachment(state, org, project, &bucket, key.trim()) {
            Some(found) => Json(dress_action_attachment(
                org,
                project,
                &stored_run,
                &stored_result,
                &result_name,
                &stored_iteration,
                &iteration_name,
                &stored_action,
                &action_name,
                found,
            ))
            .into_response(),
            None => fail(StatusCode::NOT_FOUND, "pièce introuvable"),
        };
    }
    if method == Method::DELETE {
        let Some(found) = locate_run_attachment(state, org, project, &bucket, key.trim()) else {
            return fail(StatusCode::NOT_FOUND, "pièce introuvable");
        };
        let stored_id = value_key(&found["id"]);
        let removed = delete_item(state, org, project, &bucket, &stored_id);
        return if removed {
            StatusCode::NO_CONTENT.into_response()
        } else {
            fail(StatusCode::NOT_FOUND, "pièce introuvable")
        };
    }
    fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée")
}

fn next_action_attachment_id(state: &AppState, org: &str) -> i64 {
    state
        .read(|db| {
            let Some(organization) = db.orgs.get(org) else {
                return Vec::new();
            };
            let mut rows = Vec::new();
            for project in organization.projects.values() {
                for (key, bucket) in &project.buckets {
                    if key.starts_with("test/runs/") && key.contains("/actionresults/") && key.ends_with("/attachments") {
                        rows.extend(bucket.items.values().cloned());
                    }
                }
            }
            rows
        })
        .iter()
        .filter_map(|item| value_key(&item["id"]).strip_prefix("aa-")?.parse::<i64>().ok())
        .max()
        .unwrap_or(0)
        + 1
}

fn action_attachment_create(
    state: &AppState,
    org: &str,
    project: &str,
    run_id: &str,
    result_id: &str,
    result_name: &str,
    iteration_id: &str,
    iteration_name: &str,
    action_id: &str,
    action_name: &str,
    bucket: &str,
    parsed: (String, String, String, Option<String>),
) -> Result<Value, &'static str> {
    let (file_name, comment, attachment_type, stream) = parsed;
    if list(state, org, project, bucket)
        .iter()
        .any(|item| text_of(item, "fileName").is_some_and(|stored| stored.eq_ignore_ascii_case(&file_name)))
    {
        return Err("pièce existante");
    }
    let id = format!("aa-{}", next_action_attachment_id(state, org));
    let mut item = json!({
        "id": id,
        "fileName": file_name,
        "comment": comment,
        "attachmentType": attachment_type,
        "runId": run_id,
        "resultId": result_id,
        "iterationId": iteration_id,
        "actionId": action_id
    });
    if let Some(stream) = stream {
        item["stream"] = json!(stream);
    }
    put_item(state, org, project, bucket, item.clone());
    Ok(dress_action_attachment(
        org,
        project,
        run_id,
        result_id,
        result_name,
        iteration_id,
        iteration_name,
        action_id,
        action_name,
        item,
    ))
}

fn dress_action_attachment(
    org: &str,
    project: &str,
    run_id: &str,
    result_id: &str,
    result_name: &str,
    iteration_id: &str,
    iteration_name: &str,
    action_id: &str,
    action_name: &str,
    mut item: Value,
) -> Value {
    let id = value_key(&item["id"]);
    if let Some(object) = item.as_object_mut() {
        object.insert("runId".into(), json!(run_id));
        object.insert("resultId".into(), json!(result_id));
        object.insert("resultName".into(), json!(result_name));
        object.insert("iterationId".into(), json!(iteration_id));
        object.insert("iterationName".into(), json!(iteration_name));
        object.insert("actionId".into(), json!(action_id));
        object.insert("actionName".into(), json!(action_name));
        object.insert("project".into(), json!(project));
        object.insert(
            "url".into(),
            json!(format!(
                "/{org}/{project}/_apis/test/runs/{run_id}/results/{result_id}/iterations/{iteration_id}/actionresults/{action_id}/attachments/{id}"
            )),
        );
    }
    item
}

fn action_parameters(
    state: &AppState,
    method: &Method,
    org: &str,
    project: &str,
    run_id: &str,
    rest: &[String],
    body: &[u8],
) -> Response {
    let parameter_tail = if rest.len() > 10 { &rest[10..] } else { &[] };
    if parameter_tail.len() > 1 {
        return if method == Method::DELETE {
            fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée")
        } else {
            fail(StatusCode::NOT_FOUND, "paramètre introuvable")
        };
    }
    let Some(run) = get_item(state, org, project, "test/runs", run_id) else {
        return fail(StatusCode::NOT_FOUND, "exécution introuvable");
    };
    let stored_run = value_key(&run["id"]);
    let result_key = pct_decode(rest.get(4).map(String::as_str).unwrap_or(""));
    let results_key = format!("test/runs/{stored_run}/results");
    let Some(result) = get_item(state, org, project, &results_key, result_key.trim()) else {
        return fail(StatusCode::NOT_FOUND, "résultat introuvable");
    };
    let stored_result = value_key(&result["id"]);
    let result_name = text_of(&result, "testCaseTitle")
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| stored_result.clone());
    let iteration_key = pct_decode(rest.get(6).map(String::as_str).unwrap_or(""));
    let iterations_key = format!("test/runs/{stored_run}/results/{stored_result}/iterations");
    let Some(iteration) = get_item(state, org, project, &iterations_key, iteration_key.trim()) else {
        return fail(StatusCode::NOT_FOUND, "itération introuvable");
    };
    let stored_iteration = value_key(&iteration["id"]);
    let iteration_name = text_of(&iteration, "name")
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| stored_iteration.clone());
    let action_key = pct_decode(rest.get(8).map(String::as_str).unwrap_or(""));
    let actions_key = format!("test/runs/{stored_run}/results/{stored_result}/iterations/{stored_iteration}/actionresults");
    let Some(action) = get_item(state, org, project, &actions_key, action_key.trim()) else {
        return fail(StatusCode::NOT_FOUND, "action introuvable");
    };
    let stored_action = value_key(&action["id"]);
    let action_name = text_of(&action, "name")
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| stored_action.clone());
    let bucket = format!("{actions_key}/{stored_action}/parameters");
    if parameter_tail.is_empty() {
        if method == Method::GET {
            let mut rows = list(state, org, project, &bucket);
            rows.sort_by(|left, right| value_key(&left["id"]).cmp(&value_key(&right["id"])));
            let value = rows
                .into_iter()
                .map(|item| {
                    dress_action_parameter(
                        org,
                        project,
                        &stored_run,
                        &stored_result,
                        &result_name,
                        &stored_iteration,
                        &iteration_name,
                        &stored_action,
                        &action_name,
                        item,
                    )
                })
                .collect();
            return Json(page(value)).into_response();
        }
        if method == Method::POST {
            let parsed = match iteration_parameter_payload(body) {
                Ok(parsed) => parsed,
                Err(message) => return fail(StatusCode::BAD_REQUEST, message),
            };
            return match action_parameter_create(
                state,
                org,
                project,
                &stored_run,
                &stored_result,
                &result_name,
                &stored_iteration,
                &iteration_name,
                &stored_action,
                &action_name,
                &bucket,
                parsed,
            ) {
                Ok(item) => Json(item).into_response(),
                Err(message) => fail(StatusCode::BAD_REQUEST, message),
            };
        }
        return fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée");
    }
    let key = pct_decode(&parameter_tail[0]);
    if method == Method::GET {
        return match locate_iteration_parameter(state, org, project, &bucket, key.trim()) {
            Some(found) => Json(dress_action_parameter(
                org,
                project,
                &stored_run,
                &stored_result,
                &result_name,
                &stored_iteration,
                &iteration_name,
                &stored_action,
                &action_name,
                found,
            ))
            .into_response(),
            None => fail(StatusCode::NOT_FOUND, "paramètre introuvable"),
        };
    }
    if method == Method::DELETE {
        let Some(found) = locate_iteration_parameter(state, org, project, &bucket, key.trim()) else {
            return fail(StatusCode::NOT_FOUND, "paramètre introuvable");
        };
        let stored_id = value_key(&found["id"]);
        let removed = delete_item(state, org, project, &bucket, &stored_id);
        return if removed {
            StatusCode::NO_CONTENT.into_response()
        } else {
            fail(StatusCode::NOT_FOUND, "paramètre introuvable")
        };
    }
    fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée")
}

fn next_action_parameter_id(state: &AppState, org: &str) -> i64 {
    state
        .read(|db| {
            let Some(organization) = db.orgs.get(org) else {
                return Vec::new();
            };
            let mut rows = Vec::new();
            for project in organization.projects.values() {
                for (key, bucket) in &project.buckets {
                    if key.starts_with("test/runs/") && key.contains("/actionresults/") && key.ends_with("/parameters") {
                        rows.extend(bucket.items.values().cloned());
                    }
                }
            }
            rows
        })
        .iter()
        .filter_map(|item| value_key(&item["id"]).strip_prefix("ap-")?.parse::<i64>().ok())
        .max()
        .unwrap_or(0)
        + 1
}

fn action_parameter_create(
    state: &AppState,
    org: &str,
    project: &str,
    run_id: &str,
    result_id: &str,
    result_name: &str,
    iteration_id: &str,
    iteration_name: &str,
    action_id: &str,
    action_name: &str,
    bucket: &str,
    parsed: (String, String),
) -> Result<Value, &'static str> {
    let (name, value) = parsed;
    if list(state, org, project, bucket)
        .iter()
        .any(|item| text_of(item, "name").is_some_and(|stored| stored.eq_ignore_ascii_case(&name)))
    {
        return Err("paramètre existant");
    }
    let id = format!("ap-{}", next_action_parameter_id(state, org));
    let item = json!({
        "id": id,
        "name": name,
        "value": value,
        "runId": run_id,
        "resultId": result_id,
        "iterationId": iteration_id,
        "actionId": action_id
    });
    put_item(state, org, project, bucket, item.clone());
    Ok(dress_action_parameter(
        org,
        project,
        run_id,
        result_id,
        result_name,
        iteration_id,
        iteration_name,
        action_id,
        action_name,
        item,
    ))
}

fn dress_action_parameter(
    org: &str,
    project: &str,
    run_id: &str,
    result_id: &str,
    result_name: &str,
    iteration_id: &str,
    iteration_name: &str,
    action_id: &str,
    action_name: &str,
    mut item: Value,
) -> Value {
    let id = value_key(&item["id"]);
    if let Some(object) = item.as_object_mut() {
        object.insert("runId".into(), json!(run_id));
        object.insert("resultId".into(), json!(result_id));
        object.insert("resultName".into(), json!(result_name));
        object.insert("iterationId".into(), json!(iteration_id));
        object.insert("iterationName".into(), json!(iteration_name));
        object.insert("actionId".into(), json!(action_id));
        object.insert("actionName".into(), json!(action_name));
        object.insert("project".into(), json!(project));
        object.insert(
            "url".into(),
            json!(format!(
                "/{org}/{project}/_apis/test/runs/{run_id}/results/{result_id}/iterations/{iteration_id}/actionresults/{action_id}/parameters/{id}"
            )),
        );
    }
    item
}

fn result_logs(
    state: &AppState,
    method: &Method,
    org: &str,
    project: &str,
    run_id: &str,
    rest: &[String],
    body: &[u8],
) -> Response {
    let log_tail = if rest.len() > 6 { &rest[6..] } else { &[] };
    if log_tail.len() > 1 {
        return if method == Method::DELETE {
            fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée")
        } else {
            fail(StatusCode::NOT_FOUND, "journal introuvable")
        };
    }
    let Some(run) = get_item(state, org, project, "test/runs", run_id) else {
        return fail(StatusCode::NOT_FOUND, "exécution introuvable");
    };
    let stored_run = value_key(&run["id"]);
    let result_key = pct_decode(rest.get(4).map(String::as_str).unwrap_or(""));
    let results_key = format!("test/runs/{stored_run}/results");
    let Some(result) = get_item(state, org, project, &results_key, result_key.trim()) else {
        return fail(StatusCode::NOT_FOUND, "résultat introuvable");
    };
    let stored_result = value_key(&result["id"]);
    let result_name = text_of(&result, "testCaseTitle")
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| stored_result.clone());
    let bucket = format!("test/runs/{stored_run}/results/{stored_result}/logs");
    if log_tail.is_empty() {
        if method == Method::GET {
            let mut rows = list(state, org, project, &bucket);
            rows.sort_by(|left, right| value_key(&left["id"]).cmp(&value_key(&right["id"])));
            let value = rows
                .into_iter()
                .map(|item| dress_result_log(org, project, &stored_run, &stored_result, &result_name, item))
                .collect();
            return Json(page(value)).into_response();
        }
        if method == Method::POST {
            let parsed = match result_log_payload(body) {
                Ok(parsed) => parsed,
                Err(message) => return fail(StatusCode::BAD_REQUEST, message),
            };
            return match result_log_create(state, org, project, &stored_run, &stored_result, &result_name, &bucket, parsed) {
                Ok(item) => Json(item).into_response(),
                Err(message) => fail(StatusCode::BAD_REQUEST, message),
            };
        }
        return fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée");
    }
    let key = pct_decode(&log_tail[0]);
    if method == Method::GET {
        return match locate_result_log(state, org, project, &bucket, key.trim()) {
            Some(found) => Json(dress_result_log(org, project, &stored_run, &stored_result, &result_name, found)).into_response(),
            None => fail(StatusCode::NOT_FOUND, "journal introuvable"),
        };
    }
    if method == Method::DELETE {
        let Some(found) = locate_result_log(state, org, project, &bucket, key.trim()) else {
            return fail(StatusCode::NOT_FOUND, "journal introuvable");
        };
        let stored_id = value_key(&found["id"]);
        let removed = delete_item(state, org, project, &bucket, &stored_id);
        return if removed {
            StatusCode::NO_CONTENT.into_response()
        } else {
            fail(StatusCode::NOT_FOUND, "journal introuvable")
        };
    }
    fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée")
}

fn locate_result_log(state: &AppState, org: &str, project: &str, bucket: &str, key: &str) -> Option<Value> {
    list(state, org, project, bucket).into_iter().find(|item| {
        value_key(&item["id"]) == key || text_of(item, "name").is_some_and(|stored| stored.eq_ignore_ascii_case(key))
    })
}

fn next_result_log_id(state: &AppState, org: &str) -> i64 {
    state
        .read(|db| {
            let Some(organization) = db.orgs.get(org) else {
                return Vec::new();
            };
            let mut rows = Vec::new();
            for project in organization.projects.values() {
                for (key, bucket) in &project.buckets {
                    if key.starts_with("test/runs/") && key.contains("/results/") && key.ends_with("/logs") {
                        rows.extend(bucket.items.values().cloned());
                    }
                }
            }
            rows
        })
        .iter()
        .filter_map(|item| value_key(&item["id"]).strip_prefix("rl-")?.parse::<i64>().ok())
        .max()
        .unwrap_or(0)
        + 1
}

fn result_log_payload(body: &[u8]) -> Result<(String, String), &'static str> {
    let Some(payload) = parse_json(body) else {
        return Err("JSON illisible");
    };
    let Some(object) = payload.as_object() else {
        return Err("journal illisible");
    };
    let name = match object.get("name") {
        Some(Value::String(value)) => {
            let value = value.trim();
            if value.is_empty() {
                return Err("nom absent");
            }
            value.to_string()
        }
        Some(_) => return Err("nom absent"),
        None => return Err("nom absent"),
    };
    let message = match object.get("message") {
        None => String::new(),
        Some(Value::String(value)) => value.trim().to_string(),
        Some(_) => return Err("message absent"),
    };
    Ok((name, message))
}

fn result_log_create(
    state: &AppState,
    org: &str,
    project: &str,
    run_id: &str,
    result_id: &str,
    result_name: &str,
    bucket: &str,
    parsed: (String, String),
) -> Result<Value, &'static str> {
    let (name, message) = parsed;
    if list(state, org, project, bucket)
        .iter()
        .any(|item| text_of(item, "name").is_some_and(|stored| stored.eq_ignore_ascii_case(&name)))
    {
        return Err("journal existant");
    }
    let id = format!("rl-{}", next_result_log_id(state, org));
    let item = json!({
        "id": id,
        "name": name,
        "message": message,
        "runId": run_id,
        "resultId": result_id
    });
    put_item(state, org, project, bucket, item.clone());
    Ok(dress_result_log(org, project, run_id, result_id, result_name, item))
}

fn dress_result_log(org: &str, project: &str, run_id: &str, result_id: &str, result_name: &str, mut item: Value) -> Value {
    let id = value_key(&item["id"]);
    if let Some(object) = item.as_object_mut() {
        object.insert("runId".into(), json!(run_id));
        object.insert("resultId".into(), json!(result_id));
        object.insert("resultName".into(), json!(result_name));
        object.insert("project".into(), json!(project));
        object.insert(
            "url".into(),
            json!(format!("/{org}/{project}/_apis/test/runs/{run_id}/results/{result_id}/logs/{id}")),
        );
    }
    item
}

fn sub_results(
    state: &AppState,
    method: &Method,
    org: &str,
    project: &str,
    run_id: &str,
    rest: &[String],
    body: &[u8],
) -> Response {
    let sub_tail = if rest.len() > 6 { &rest[6..] } else { &[] };
    if sub_tail.len() > 1 {
        return if method == Method::DELETE {
            fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée")
        } else {
            fail(StatusCode::NOT_FOUND, "sous-résultat introuvable")
        };
    }
    let Some(run) = get_item(state, org, project, "test/runs", run_id) else {
        return fail(StatusCode::NOT_FOUND, "exécution introuvable");
    };
    let stored_run = value_key(&run["id"]);
    let result_key = pct_decode(rest.get(4).map(String::as_str).unwrap_or(""));
    let results_key = format!("test/runs/{stored_run}/results");
    let Some(result) = get_item(state, org, project, &results_key, result_key.trim()) else {
        return fail(StatusCode::NOT_FOUND, "résultat introuvable");
    };
    let stored_result = value_key(&result["id"]);
    let result_name = text_of(&result, "testCaseTitle")
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| stored_result.clone());
    let bucket = format!("test/runs/{stored_run}/results/{stored_result}/subresults");
    if sub_tail.is_empty() {
        if method == Method::GET {
            let mut rows = list(state, org, project, &bucket);
            rows.sort_by(|left, right| value_key(&left["id"]).cmp(&value_key(&right["id"])));
            let value = rows
                .into_iter()
                .map(|item| dress_sub_result(org, project, &stored_run, &stored_result, &result_name, item))
                .collect();
            return Json(page(value)).into_response();
        }
        if method == Method::POST {
            let parsed = match sub_result_payload(body) {
                Ok(parsed) => parsed,
                Err(message) => return fail(StatusCode::BAD_REQUEST, message),
            };
            return match sub_result_create(state, org, project, &stored_run, &stored_result, &result_name, &bucket, parsed) {
                Ok(item) => Json(item).into_response(),
                Err(message) => fail(StatusCode::BAD_REQUEST, message),
            };
        }
        return fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée");
    }
    let key = pct_decode(&sub_tail[0]);
    if method == Method::GET {
        return match locate_sub_result(state, org, project, &bucket, key.trim()) {
            Some(found) => Json(dress_sub_result(org, project, &stored_run, &stored_result, &result_name, found)).into_response(),
            None => fail(StatusCode::NOT_FOUND, "sous-résultat introuvable"),
        };
    }
    if method == Method::DELETE {
        let Some(found) = locate_sub_result(state, org, project, &bucket, key.trim()) else {
            return fail(StatusCode::NOT_FOUND, "sous-résultat introuvable");
        };
        let stored_id = value_key(&found["id"]);
        let removed = delete_item(state, org, project, &bucket, &stored_id);
        if removed {
            let child_prefix = format!("test/runs/{stored_run}/results/{stored_result}/subresults/{stored_id}/");
            state.write(|db| {
                if let Some(stored) = db.project_mut(org, project) {
                    stored.buckets.retain(|key, _| !key.starts_with(&child_prefix));
                }
            });
            return StatusCode::NO_CONTENT.into_response();
        }
        return fail(StatusCode::NOT_FOUND, "sous-résultat introuvable");
    }
    fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée")
}

fn locate_sub_result(state: &AppState, org: &str, project: &str, bucket: &str, key: &str) -> Option<Value> {
    list(state, org, project, bucket).into_iter().find(|item| {
        value_key(&item["id"]) == key || text_of(item, "name").is_some_and(|stored| stored.eq_ignore_ascii_case(key))
    })
}

fn next_sub_result_id(state: &AppState, org: &str) -> i64 {
    state
        .read(|db| {
            let Some(organization) = db.orgs.get(org) else {
                return Vec::new();
            };
            let mut rows = Vec::new();
            for project in organization.projects.values() {
                for (key, bucket) in &project.buckets {
                    if key.starts_with("test/runs/") && key.contains("/results/") && key.ends_with("/subresults") {
                        rows.extend(bucket.items.values().cloned());
                    }
                }
            }
            rows
        })
        .iter()
        .filter_map(|item| value_key(&item["id"]).strip_prefix("sr-")?.parse::<i64>().ok())
        .max()
        .unwrap_or(0)
        + 1
}

fn sub_result_payload(body: &[u8]) -> Result<(String, String), &'static str> {
    let Some(payload) = parse_json(body) else {
        return Err("JSON illisible");
    };
    let Some(object) = payload.as_object() else {
        return Err("sous-résultat illisible");
    };
    let name = match object.get("name") {
        Some(Value::String(value)) => {
            let value = value.trim();
            if value.is_empty() {
                return Err("nom absent");
            }
            value.to_string()
        }
        Some(_) => return Err("nom absent"),
        None => return Err("nom absent"),
    };
    let comment = match object.get("comment") {
        None => String::new(),
        Some(Value::String(value)) => value.trim().to_string(),
        Some(_) => return Err("commentaire absent"),
    };
    Ok((name, comment))
}

fn sub_result_create(
    state: &AppState,
    org: &str,
    project: &str,
    run_id: &str,
    result_id: &str,
    result_name: &str,
    bucket: &str,
    parsed: (String, String),
) -> Result<Value, &'static str> {
    let (name, comment) = parsed;
    if list(state, org, project, bucket)
        .iter()
        .any(|item| text_of(item, "name").is_some_and(|stored| stored.eq_ignore_ascii_case(&name)))
    {
        return Err("sous-résultat existant");
    }
    let id = format!("sr-{}", next_sub_result_id(state, org));
    let item = json!({
        "id": id,
        "name": name,
        "comment": comment,
        "runId": run_id,
        "resultId": result_id
    });
    put_item(state, org, project, bucket, item.clone());
    Ok(dress_sub_result(org, project, run_id, result_id, result_name, item))
}

fn dress_sub_result(org: &str, project: &str, run_id: &str, result_id: &str, result_name: &str, mut item: Value) -> Value {
    let id = value_key(&item["id"]);
    if let Some(object) = item.as_object_mut() {
        object.insert("runId".into(), json!(run_id));
        object.insert("resultId".into(), json!(result_id));
        object.insert("resultName".into(), json!(result_name));
        object.insert("project".into(), json!(project));
        object.insert(
            "url".into(),
            json!(format!("/{org}/{project}/_apis/test/runs/{run_id}/results/{result_id}/subresults/{id}")),
        );
    }
    item
}

fn sub_result_attachments(
    state: &AppState,
    method: &Method,
    org: &str,
    project: &str,
    run_id: &str,
    rest: &[String],
    body: &[u8],
) -> Response {
    let file_tail = if rest.len() > 8 { &rest[8..] } else { &[] };
    if file_tail.len() > 1 {
        return if method == Method::DELETE {
            fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée")
        } else {
            fail(StatusCode::NOT_FOUND, "pièce introuvable")
        };
    }
    let Some(run) = get_item(state, org, project, "test/runs", run_id) else {
        return fail(StatusCode::NOT_FOUND, "exécution introuvable");
    };
    let stored_run = value_key(&run["id"]);
    let result_key = pct_decode(rest.get(4).map(String::as_str).unwrap_or(""));
    let results_key = format!("test/runs/{stored_run}/results");
    let Some(result) = get_item(state, org, project, &results_key, result_key.trim()) else {
        return fail(StatusCode::NOT_FOUND, "résultat introuvable");
    };
    let stored_result = value_key(&result["id"]);
    let result_name = text_of(&result, "testCaseTitle")
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| stored_result.clone());
    let sub_key = pct_decode(rest.get(6).map(String::as_str).unwrap_or(""));
    let subs_key = format!("test/runs/{stored_run}/results/{stored_result}/subresults");
    let Some(sub) = get_item(state, org, project, &subs_key, sub_key.trim()) else {
        return fail(StatusCode::NOT_FOUND, "sous-résultat introuvable");
    };
    let stored_sub = value_key(&sub["id"]);
    let sub_name = text_of(&sub, "name")
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| stored_sub.clone());
    let bucket = format!("{subs_key}/{stored_sub}/attachments");
    if file_tail.is_empty() {
        if method == Method::GET {
            let mut rows = list(state, org, project, &bucket);
            rows.sort_by(|left, right| value_key(&left["id"]).cmp(&value_key(&right["id"])));
            let value = rows
                .into_iter()
                .map(|item| dress_sub_result_attachment(org, project, &stored_run, &stored_result, &result_name, &stored_sub, &sub_name, item))
                .collect();
            return Json(page(value)).into_response();
        }
        if method == Method::POST {
            let parsed = match run_attachment_payload(body) {
                Ok(parsed) => parsed,
                Err(message) => return fail(StatusCode::BAD_REQUEST, message),
            };
            return match sub_result_attachment_create(
                state,
                org,
                project,
                &stored_run,
                &stored_result,
                &result_name,
                &stored_sub,
                &sub_name,
                &bucket,
                parsed,
            ) {
                Ok(item) => Json(item).into_response(),
                Err(message) => fail(StatusCode::BAD_REQUEST, message),
            };
        }
        return fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée");
    }
    let key = pct_decode(&file_tail[0]);
    if method == Method::GET {
        return match locate_run_attachment(state, org, project, &bucket, key.trim()) {
            Some(found) => {
                Json(dress_sub_result_attachment(org, project, &stored_run, &stored_result, &result_name, &stored_sub, &sub_name, found)).into_response()
            }
            None => fail(StatusCode::NOT_FOUND, "pièce introuvable"),
        };
    }
    if method == Method::DELETE {
        let Some(found) = locate_run_attachment(state, org, project, &bucket, key.trim()) else {
            return fail(StatusCode::NOT_FOUND, "pièce introuvable");
        };
        let stored_id = value_key(&found["id"]);
        let removed = delete_item(state, org, project, &bucket, &stored_id);
        return if removed {
            StatusCode::NO_CONTENT.into_response()
        } else {
            fail(StatusCode::NOT_FOUND, "pièce introuvable")
        };
    }
    fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée")
}

fn next_sub_result_attachment_id(state: &AppState, org: &str) -> i64 {
    state
        .read(|db| {
            let Some(organization) = db.orgs.get(org) else {
                return Vec::new();
            };
            let mut rows = Vec::new();
            for project in organization.projects.values() {
                for (key, bucket) in &project.buckets {
                    if key.starts_with("test/runs/") && key.contains("/subresults/") && key.ends_with("/attachments") {
                        rows.extend(bucket.items.values().cloned());
                    }
                }
            }
            rows
        })
        .iter()
        .filter_map(|item| value_key(&item["id"]).strip_prefix("sa-")?.parse::<i64>().ok())
        .max()
        .unwrap_or(0)
        + 1
}

fn sub_result_attachment_create(
    state: &AppState,
    org: &str,
    project: &str,
    run_id: &str,
    result_id: &str,
    result_name: &str,
    sub_id: &str,
    sub_name: &str,
    bucket: &str,
    parsed: (String, String, String, Option<String>),
) -> Result<Value, &'static str> {
    let (file_name, comment, attachment_type, stream) = parsed;
    if list(state, org, project, bucket)
        .iter()
        .any(|item| text_of(item, "fileName").is_some_and(|stored| stored.eq_ignore_ascii_case(&file_name)))
    {
        return Err("pièce existante");
    }
    let id = format!("sa-{}", next_sub_result_attachment_id(state, org));
    let mut item = json!({
        "id": id,
        "fileName": file_name,
        "comment": comment,
        "attachmentType": attachment_type,
        "runId": run_id,
        "resultId": result_id,
        "subResultId": sub_id
    });
    if let Some(stream) = stream {
        item["stream"] = json!(stream);
    }
    put_item(state, org, project, bucket, item.clone());
    Ok(dress_sub_result_attachment(org, project, run_id, result_id, result_name, sub_id, sub_name, item))
}

fn dress_sub_result_attachment(
    org: &str,
    project: &str,
    run_id: &str,
    result_id: &str,
    result_name: &str,
    sub_id: &str,
    sub_name: &str,
    mut item: Value,
) -> Value {
    let id = value_key(&item["id"]);
    if let Some(object) = item.as_object_mut() {
        object.insert("runId".into(), json!(run_id));
        object.insert("resultId".into(), json!(result_id));
        object.insert("resultName".into(), json!(result_name));
        object.insert("subResultId".into(), json!(sub_id));
        object.insert("subResultName".into(), json!(sub_name));
        object.insert("project".into(), json!(project));
        object.insert(
            "url".into(),
            json!(format!(
                "/{org}/{project}/_apis/test/runs/{run_id}/results/{result_id}/subresults/{sub_id}/attachments/{id}"
            )),
        );
    }
    item
}

const RESULT_METADATA: &str = "testresults/resultmetadata";

fn test_result_metadata(state: &AppState, method: &Method, org: &str, project: &str, rest: &[String], body: &[u8]) -> Response {
    if rest.get(2).is_none_or(|part| !part.eq_ignore_ascii_case("resultmetadata")) {
        return fail(StatusCode::NOT_FOUND, "métadonnée introuvable");
    }
    let tail = if rest.len() > 3 { &rest[3..] } else { &[] };
    if tail.len() > 1 {
        return if method == Method::DELETE {
            fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée")
        } else {
            fail(StatusCode::NOT_FOUND, "métadonnée introuvable")
        };
    }
    if tail.is_empty() {
        if method == Method::GET {
            let mut rows = list(state, org, project, RESULT_METADATA);
            rows.sort_by(|left, right| value_key(&left["id"]).cmp(&value_key(&right["id"])));
            let value = rows.into_iter().map(|item| dress_result_metadata(org, project, item)).collect();
            return Json(page(value)).into_response();
        }
        if method == Method::POST {
            let rows = match metadata_payload(body) {
                Ok(rows) => rows,
                Err(message) => return fail(StatusCode::BAD_REQUEST, message),
            };
            let mut created = Vec::new();
            for row in rows {
                match metadata_create(state, org, project, row) {
                    Ok(item) => created.push(item),
                    Err(message) => return fail(StatusCode::BAD_REQUEST, message),
                }
            }
            return if created.len() == 1 {
                Json(created.remove(0)).into_response()
            } else {
                Json(created).into_response()
            };
        }
        return fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée");
    }
    let key = pct_decode(&tail[0]);
    let key = key.trim();
    let Some(found) = locate_result_metadata(state, org, project, key) else {
        return fail(StatusCode::NOT_FOUND, "métadonnée introuvable");
    };
    if method == Method::GET {
        return Json(dress_result_metadata(org, project, found)).into_response();
    }
    if method == Method::DELETE {
        let stored_id = value_key(&found["id"]);
        return if delete_item(state, org, project, RESULT_METADATA, &stored_id) {
            StatusCode::NO_CONTENT.into_response()
        } else {
            fail(StatusCode::NOT_FOUND, "métadonnée introuvable")
        };
    }
    fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée")
}

struct MetadataRow {
    title: String,
    branch: String,
    flaky: bool,
}

fn metadata_payload(body: &[u8]) -> Result<Vec<MetadataRow>, &'static str> {
    let Some(payload) = parse_json(body) else {
        return Err("JSON illisible");
    };
    let Some(object) = payload.as_object() else {
        return Err("métadonnée illisible");
    };
    if object.contains_key("flakyIdentifiers") || object.contains_key("branchName") {
        let title = required_metadata_text(object.get("testCaseTitle").or_else(|| object.get("name")), "nom absent")?;
        let Some(identifiers) = object.get("flakyIdentifiers").and_then(Value::as_array) else {
            return Err("identifiant illisible");
        };
        if identifiers.is_empty() {
            return Err("identifiant absent");
        }
        let mut rows = Vec::new();
        for identifier in identifiers {
            let Some(identifier) = identifier.as_object() else {
                return Err("identifiant illisible");
            };
            let branch = required_metadata_text(identifier.get("branchName"), "branche absente")?;
            let flaky = match identifier.get("isFlaky") {
                None => true,
                Some(Value::Bool(value)) => *value,
                Some(_) => return Err("flaky absent"),
            };
            if rows.iter().any(|row: &MetadataRow| row.title.eq_ignore_ascii_case(&title) && row.branch.eq_ignore_ascii_case(&branch)) {
                return Err("métadonnée existante");
            }
            rows.push(MetadataRow { title: title.clone(), branch, flaky });
        }
        return Ok(rows);
    }
    let title = required_metadata_text(object.get("name").or_else(|| object.get("testCaseTitle")), "nom absent")?;
    Ok(vec![MetadataRow { title, branch: "refs/heads/main".into(), flaky: true }])
}

fn required_metadata_text(value: Option<&Value>, missing: &'static str) -> Result<String, &'static str> {
    let Some(value) = value else {
        return Err(missing);
    };
    let Some(text) = value.as_str() else {
        return Err(missing);
    };
    let text = text.trim();
    if text.is_empty() {
        return Err(missing);
    }
    Ok(text.to_string())
}

fn metadata_create(state: &AppState, org: &str, project: &str, row: MetadataRow) -> Result<Value, &'static str> {
    if list(state, org, project, RESULT_METADATA).iter().any(|item| {
        text_of(item, "testCaseTitle").is_some_and(|stored| stored.eq_ignore_ascii_case(&row.title))
            && text_of(item, "branchName").is_some_and(|stored| stored.eq_ignore_ascii_case(&row.branch))
    }) {
        return Err("métadonnée existante");
    }
    let number = next_metadata_id(state, org);
    let item = json!({
        "id": format!("md-{number}"),
        "testCaseReferenceId": number,
        "name": row.title,
        "testCaseTitle": row.title,
        "branchName": row.branch,
        "isFlaky": row.flaky,
        "priority": 0
    });
    put_item(state, org, project, RESULT_METADATA, item.clone());
    Ok(dress_result_metadata(org, project, item))
}

fn next_metadata_id(state: &AppState, org: &str) -> i64 {
    state
        .read(|db| {
            let Some(organization) = db.orgs.get(org) else {
                return Vec::new();
            };
            let mut rows = Vec::new();
            for project in organization.projects.values() {
                if let Some(bucket) = project.buckets.get(RESULT_METADATA) {
                    rows.extend(bucket.items.values().cloned());
                }
            }
            rows
        })
        .iter()
        .filter_map(|item| value_key(&item["id"]).strip_prefix("md-")?.parse::<i64>().ok())
        .max()
        .unwrap_or(0)
        + 1
}

fn locate_result_metadata(state: &AppState, org: &str, project: &str, key: &str) -> Option<Value> {
    list(state, org, project, RESULT_METADATA).into_iter().find(|item| {
        value_key(&item["id"]) == key
            || value_key(&item["testCaseReferenceId"]) == key
            || text_of(item, "name").is_some_and(|stored| stored.eq_ignore_ascii_case(key))
    })
}

fn dress_result_metadata(org: &str, project: &str, mut item: Value) -> Value {
    let id = value_key(&item["id"]);
    let branch = text_of(&item, "branchName").unwrap_or_default();
    let flaky = item.get("isFlaky").and_then(Value::as_bool).unwrap_or(false);
    if let Some(object) = item.as_object_mut() {
        object.insert("project".into(), json!(project));
        object.insert("flakyIdentifiers".into(), json!([{ "branchName": branch, "isFlaky": flaky }]));
        object.insert("url".into(), json!(format!("/{org}/{project}/_apis/testresults/results/resultmetadata/{id}")));
    }
    item
}

const FAILURE_TYPES: &str = "testresults/testfailuretype";

fn test_failure_types(state: &AppState, method: &Method, org: &str, project: &str, rest: &[String], body: &[u8]) -> Response {
    let tail = if rest.len() > 2 { &rest[2..] } else { &[] };
    if tail.len() > 1 {
        return if method == Method::DELETE {
            fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée")
        } else {
            fail(StatusCode::NOT_FOUND, "échec introuvable")
        };
    }
    if tail.is_empty() {
        if method == Method::GET {
            let mut rows = list(state, org, project, FAILURE_TYPES);
            rows.sort_by(|left, right| value_key(&left["id"]).cmp(&value_key(&right["id"])));
            let value = rows.into_iter().map(|item| dress_failure_type(org, project, item)).collect();
            return Json(page(value)).into_response();
        }
        if method == Method::POST {
            let name = match failure_type_name(body) {
                Ok(name) => name,
                Err(message) => return fail(StatusCode::BAD_REQUEST, message),
            };
            return match failure_type_create(state, org, project, name) {
                Ok(item) => Json(item).into_response(),
                Err(message) => fail(StatusCode::BAD_REQUEST, message),
            };
        }
        return fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée");
    }
    let key = pct_decode(&tail[0]);
    let key = key.trim();
    let Some(found) = locate_failure_type(state, org, project, key) else {
        return fail(StatusCode::NOT_FOUND, "échec introuvable");
    };
    if method == Method::GET {
        return Json(dress_failure_type(org, project, found)).into_response();
    }
    if method == Method::DELETE {
        let stored_id = value_key(&found["id"]);
        return if delete_item(state, org, project, FAILURE_TYPES, &stored_id) {
            StatusCode::NO_CONTENT.into_response()
        } else {
            fail(StatusCode::NOT_FOUND, "échec introuvable")
        };
    }
    fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée")
}

fn failure_type_name(body: &[u8]) -> Result<String, &'static str> {
    let Some(payload) = parse_json(body) else {
        return Err("JSON illisible");
    };
    let Some(object) = payload.as_object() else {
        return Err("échec illisible");
    };
    let Some(name) = object.get("name").and_then(Value::as_str) else {
        return Err("nom absent");
    };
    let name = name.trim();
    if name.is_empty() {
        return Err("nom absent");
    }
    Ok(name.to_string())
}

fn failure_type_create(state: &AppState, org: &str, project: &str, name: String) -> Result<Value, &'static str> {
    if list(state, org, project, FAILURE_TYPES)
        .iter()
        .any(|item| text_of(item, "name").is_some_and(|stored| stored.eq_ignore_ascii_case(&name)))
    {
        return Err("échec existant");
    }
    let number = next_failure_type_id(state, org);
    let item = json!({
        "id": format!("ft-{number}"),
        "failureTypeId": number,
        "name": name
    });
    put_item(state, org, project, FAILURE_TYPES, item.clone());
    Ok(dress_failure_type(org, project, item))
}

fn next_failure_type_id(state: &AppState, org: &str) -> i64 {
    state
        .read(|db| {
            let Some(organization) = db.orgs.get(org) else {
                return Vec::new();
            };
            let mut rows = Vec::new();
            for project in organization.projects.values() {
                if let Some(bucket) = project.buckets.get(FAILURE_TYPES) {
                    rows.extend(bucket.items.values().cloned());
                }
            }
            rows
        })
        .iter()
        .filter_map(|item| value_key(&item["id"]).strip_prefix("ft-")?.parse::<i64>().ok())
        .max()
        .unwrap_or(0)
        + 1
}

fn locate_failure_type(state: &AppState, org: &str, project: &str, key: &str) -> Option<Value> {
    list(state, org, project, FAILURE_TYPES).into_iter().find(|item| {
        value_key(&item["id"]) == key
            || value_key(&item["failureTypeId"]) == key
            || text_of(item, "name").is_some_and(|stored| stored.eq_ignore_ascii_case(key))
    })
}

fn dress_failure_type(org: &str, project: &str, mut item: Value) -> Value {
    let id = value_key(&item["id"]);
    if let Some(object) = item.as_object_mut() {
        object.insert("project".into(), json!(project));
        object.insert("url".into(), json!(format!("/{org}/{project}/_apis/testresults/testfailuretype/{id}")));
    }
    item
}

const CODE_COVERAGE: &str = "testresults/codecoverage";

fn test_code_coverage(
    state: &AppState,
    method: &Method,
    org: &str,
    project: &str,
    rest: &[String],
    query: &str,
    body: &[u8],
) -> Response {
    let tail = if rest.len() > 2 { &rest[2..] } else { &[] };
    if tail.len() > 1 {
        return if method == Method::DELETE {
            fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée")
        } else {
            fail(StatusCode::NOT_FOUND, "couverture introuvable")
        };
    }
    if tail.is_empty() {
        if method == Method::GET {
            if let Some(build_id) = query_text(query, "buildId") {
                let Some(build) = get_item(state, org, project, "build/builds", build_id.trim()) else {
                    return fail(StatusCode::NOT_FOUND, "build introuvable");
                };
                let stored_build = value_key(&build["id"]);
                let build_name = text_of(&build, "buildNumber")
                    .filter(|value| !value.trim().is_empty())
                    .or_else(|| text_of(&build, "name"))
                    .unwrap_or_else(|| stored_build.clone());
                return Json(coverage_summary(state, org, project, &stored_build, &build_name)).into_response();
            }
            let mut rows = alive_coverage(state, org, project);
            rows.sort_by(|left, right| value_key(&left["id"]).cmp(&value_key(&right["id"])));
            let value = rows
                .into_iter()
                .map(|item| dress_coverage(state, org, project, item))
                .collect();
            return Json(page(value)).into_response();
        }
        if method == Method::POST {
            let Some(build_id) = query_text(query, "buildId") else {
                return fail(StatusCode::BAD_REQUEST, "build absent");
            };
            let Some(build) = get_item(state, org, project, "build/builds", build_id.trim()) else {
                return fail(StatusCode::NOT_FOUND, "build introuvable");
            };
            let stored_build = value_key(&build["id"]);
            let parsed = match coverage_payload(body) {
                Ok(parsed) => parsed,
                Err(message) => return fail(StatusCode::BAD_REQUEST, message),
            };
            let mut created = Vec::new();
            for row in parsed {
                match coverage_create(state, org, project, &stored_build, row) {
                    Ok(item) => created.push(item),
                    Err(message) => return fail(StatusCode::BAD_REQUEST, message),
                }
            }
            return if created.len() == 1 {
                Json(created.remove(0)).into_response()
            } else {
                Json(created).into_response()
            };
        }
        return fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée");
    }
    let key = pct_decode(&tail[0]);
    let key = key.trim();
    let Some(found) = locate_coverage(state, org, project, key) else {
        return fail(StatusCode::NOT_FOUND, "couverture introuvable");
    };
    let build_id = value_key(&found["buildId"]);
    if get_item(state, org, project, "build/builds", &build_id).is_none() {
        return fail(StatusCode::NOT_FOUND, "build introuvable");
    }
    if method == Method::GET {
        return Json(dress_coverage(state, org, project, found)).into_response();
    }
    if method == Method::DELETE {
        let stored_id = value_key(&found["id"]);
        return if delete_item(state, org, project, CODE_COVERAGE, &stored_id) {
            StatusCode::NO_CONTENT.into_response()
        } else {
            fail(StatusCode::NOT_FOUND, "couverture introuvable")
        };
    }
    fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée")
}

struct CoverageRow {
    flavor: String,
    platform: String,
    label: String,
    covered: i64,
    total: i64,
}

fn coverage_payload(body: &[u8]) -> Result<Vec<CoverageRow>, &'static str> {
    let Some(payload) = parse_json(body) else {
        return Err("JSON illisible");
    };
    let Some(object) = payload.as_object() else {
        return Err("couverture illisible");
    };
    if object.contains_key("coverageStats") || object.contains_key("buildFlavor") || object.contains_key("buildPlatform") {
        let flavor = required_coverage_text(object.get("buildFlavor"), "variante absente")?;
        let platform = required_coverage_text(object.get("buildPlatform"), "plateforme absente")?;
        let Some(stats) = object.get("coverageStats").and_then(Value::as_array) else {
            return Err("statistique illisible");
        };
        if stats.is_empty() {
            return Err("statistique absente");
        }
        let mut rows = Vec::new();
        for stat in stats {
            let Some(stat) = stat.as_object() else {
                return Err("statistique illisible");
            };
            let label = required_coverage_text(stat.get("label"), "nom absent")?;
            let covered = coverage_number(stat.get("covered"), "couvert absent")?;
            let total = coverage_number(stat.get("total"), "total absent")?;
            if rows.iter().any(|row: &CoverageRow| {
                row.flavor.eq_ignore_ascii_case(&flavor)
                    && row.platform.eq_ignore_ascii_case(&platform)
                    && row.label.eq_ignore_ascii_case(&label)
            }) {
                return Err("couverture existante");
            }
            rows.push(CoverageRow { flavor: flavor.clone(), platform: platform.clone(), label, covered, total });
        }
        return Ok(rows);
    }
    let label = required_coverage_text(object.get("name").or_else(|| object.get("label")), "nom absent")?;
    Ok(vec![CoverageRow {
        flavor: "Debug".into(),
        platform: "AnyCPU".into(),
        label,
        covered: 1,
        total: 1,
    }])
}

fn required_coverage_text(value: Option<&Value>, missing: &'static str) -> Result<String, &'static str> {
    let Some(value) = value else {
        return Err(missing);
    };
    let Some(text) = value.as_str() else {
        return Err(missing);
    };
    let text = text.trim();
    if text.is_empty() {
        return Err(missing);
    }
    Ok(text.to_string())
}

fn coverage_number(value: Option<&Value>, missing: &'static str) -> Result<i64, &'static str> {
    match value {
        None => Ok(0),
        Some(Value::Number(number)) => number.as_i64().ok_or(missing),
        Some(_) => Err(missing),
    }
}

fn coverage_create(state: &AppState, org: &str, project: &str, build_id: &str, row: CoverageRow) -> Result<Value, &'static str> {
    if alive_coverage(state, org, project).iter().any(|item| {
        value_key(&item["buildId"]) == build_id
            && text_of(item, "buildFlavor").is_some_and(|stored| stored.eq_ignore_ascii_case(&row.flavor))
            && text_of(item, "buildPlatform").is_some_and(|stored| stored.eq_ignore_ascii_case(&row.platform))
            && text_of(item, "label").is_some_and(|stored| stored.eq_ignore_ascii_case(&row.label))
    }) {
        return Err("couverture existante");
    }
    let id = format!("cv-{}", next_coverage_id(state, org));
    let item = json!({
        "id": id,
        "buildId": build_id,
        "buildFlavor": row.flavor,
        "buildPlatform": row.platform,
        "label": row.label.clone(),
        "name": row.label,
        "covered": row.covered,
        "total": row.total
    });
    put_item(state, org, project, CODE_COVERAGE, item.clone());
    Ok(dress_coverage(state, org, project, item))
}

fn next_coverage_id(state: &AppState, org: &str) -> i64 {
    state
        .read(|db| {
            let Some(organization) = db.orgs.get(org) else {
                return Vec::new();
            };
            let mut rows = Vec::new();
            for project in organization.projects.values() {
                if let Some(bucket) = project.buckets.get(CODE_COVERAGE) {
                    rows.extend(bucket.items.values().cloned());
                }
            }
            rows
        })
        .iter()
        .filter_map(|item| value_key(&item["id"]).strip_prefix("cv-")?.parse::<i64>().ok())
        .max()
        .unwrap_or(0)
        + 1
}

fn alive_coverage(state: &AppState, org: &str, project: &str) -> Vec<Value> {
    list(state, org, project, CODE_COVERAGE)
        .into_iter()
        .filter(|item| get_item(state, org, project, "build/builds", &value_key(&item["buildId"])).is_some())
        .collect()
}

fn locate_coverage(state: &AppState, org: &str, project: &str, key: &str) -> Option<Value> {
    list(state, org, project, CODE_COVERAGE).into_iter().find(|item| {
        value_key(&item["id"]) == key || text_of(item, "name").is_some_and(|stored| stored.eq_ignore_ascii_case(key))
    })
}

fn dress_coverage(state: &AppState, org: &str, project: &str, mut item: Value) -> Value {
    let id = value_key(&item["id"]);
    let build_id = value_key(&item["buildId"]);
    let build_name = get_item(state, org, project, "build/builds", &build_id)
        .and_then(|build| text_of(&build, "buildNumber").filter(|value| !value.trim().is_empty()).or_else(|| text_of(&build, "name")))
        .unwrap_or_else(|| build_id.clone());
    if let Some(object) = item.as_object_mut() {
        object.insert("buildId".into(), json!(build_id));
        object.insert("buildName".into(), json!(build_name));
        object.insert("project".into(), json!(project));
        object.insert("url".into(), json!(format!("/{org}/{project}/_apis/testresults/codecoverage/{id}")));
    }
    item
}

fn coverage_summary(state: &AppState, org: &str, project: &str, build_id: &str, build_name: &str) -> Value {
    let mut groups: Vec<(String, String, Vec<Value>)> = Vec::new();
    for item in alive_coverage(state, org, project) {
        if value_key(&item["buildId"]) != build_id {
            continue;
        }
        let flavor = text_of(&item, "buildFlavor").unwrap_or_default();
        let platform = text_of(&item, "buildPlatform").unwrap_or_default();
        let stat = json!({
            "label": text_of(&item, "label").unwrap_or_default(),
            "covered": item.get("covered").and_then(Value::as_i64).unwrap_or(0),
            "total": item.get("total").and_then(Value::as_i64).unwrap_or(0),
            "position": 0
        });
        if let Some(group) = groups.iter_mut().find(|group| group.0.eq_ignore_ascii_case(&flavor) && group.1.eq_ignore_ascii_case(&platform)) {
            group.2.push(stat);
        } else {
            groups.push((flavor, platform, vec![stat]));
        }
    }
    let coverage_data: Vec<Value> = groups
        .into_iter()
        .map(|(flavor, platform, stats)| json!({ "buildFlavor": flavor, "buildPlatform": platform, "coverageStats": stats }))
        .collect();
    json!({
        "build": { "id": build_id, "name": build_name },
        "coverageData": coverage_data,
        "status": "completed",
        "project": project
    })
}

fn test_run_custom_fields(state: &AppState, method: &Method, org: &str, project: &str, rest: &[String], body: &[u8]) -> Response {
    let tail = if rest.len() > 4 { &rest[4..] } else { &[] };
    if tail.len() > 1 {
        return if method == Method::DELETE {
            fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée")
        } else {
            fail(StatusCode::NOT_FOUND, "champ introuvable")
        };
    }
    let run_key = pct_decode(rest.get(2).map(String::as_str).unwrap_or(""));
    let Some(run) = get_item(state, org, project, "test/runs", run_key.trim()) else {
        return fail(StatusCode::NOT_FOUND, "exécution introuvable");
    };
    let stored_run = value_key(&run["id"]);
    let run_name = text_of(&run, "name")
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| stored_run.clone());
    let bucket = format!("testresults/runs/{stored_run}/customfields");
    if tail.is_empty() {
        if method == Method::GET {
            let mut rows = list(state, org, project, &bucket);
            rows.sort_by(|left, right| value_key(&left["id"]).cmp(&value_key(&right["id"])));
            let value = rows
                .into_iter()
                .map(|item| dress_custom_field(org, project, &stored_run, &run_name, item))
                .collect();
            return Json(page(value)).into_response();
        }
        if method == Method::POST {
            let (name, value) = match custom_field_payload(body) {
                Ok(parsed) => parsed,
                Err(message) => return fail(StatusCode::BAD_REQUEST, message),
            };
            return match custom_field_create(state, org, project, &stored_run, &run_name, &bucket, name, value) {
                Ok(item) => Json(item).into_response(),
                Err(message) => fail(StatusCode::BAD_REQUEST, message),
            };
        }
        return fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée");
    }
    let key = pct_decode(&tail[0]);
    let key = key.trim();
    let Some(found) = locate_custom_field(state, org, project, &bucket, key) else {
        return fail(StatusCode::NOT_FOUND, "champ introuvable");
    };
    if method == Method::GET {
        return Json(dress_custom_field(org, project, &stored_run, &run_name, found)).into_response();
    }
    if method == Method::DELETE {
        let stored_id = value_key(&found["id"]);
        return if delete_item(state, org, project, &bucket, &stored_id) {
            StatusCode::NO_CONTENT.into_response()
        } else {
            fail(StatusCode::NOT_FOUND, "champ introuvable")
        };
    }
    fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée")
}

fn custom_field_payload(body: &[u8]) -> Result<(String, Value), &'static str> {
    let Some(payload) = parse_json(body) else {
        return Err("JSON illisible");
    };
    let Some(object) = payload.as_object() else {
        return Err("champ illisible");
    };
    let name = required_metadata_text(object.get("fieldName").or_else(|| object.get("name")), "nom absent")?;
    let value = match object.get("value") {
        None => json!(name),
        Some(Value::String(text)) => {
            let text = text.trim();
            if text.is_empty() {
                return Err("valeur absente");
            }
            json!(text)
        }
        Some(Value::Number(number)) => Value::Number(number.clone()),
        Some(Value::Bool(flag)) => json!(flag),
        Some(_) => return Err("valeur absente"),
    };
    Ok((name, value))
}

fn custom_field_create(
    state: &AppState,
    org: &str,
    project: &str,
    run_id: &str,
    run_name: &str,
    bucket: &str,
    name: String,
    value: Value,
) -> Result<Value, &'static str> {
    if list(state, org, project, bucket)
        .iter()
        .any(|item| text_of(item, "fieldName").is_some_and(|stored| stored.eq_ignore_ascii_case(&name)))
    {
        return Err("champ existant");
    }
    let number = next_custom_field_id(state, org);
    let item = json!({
        "id": format!("cf-{number}"),
        "fieldId": number,
        "fieldName": name,
        "name": name,
        "value": value
    });
    put_item(state, org, project, bucket, item.clone());
    Ok(dress_custom_field(org, project, run_id, run_name, item))
}

fn next_custom_field_id(state: &AppState, org: &str) -> i64 {
    state
        .read(|db| {
            let Some(organization) = db.orgs.get(org) else {
                return Vec::new();
            };
            let mut rows = Vec::new();
            for project in organization.projects.values() {
                for (key, bucket) in &project.buckets {
                    if key.ends_with("/customfields") {
                        rows.extend(bucket.items.values().cloned());
                    }
                }
            }
            rows
        })
        .iter()
        .filter_map(|item| value_key(&item["id"]).strip_prefix("cf-")?.parse::<i64>().ok())
        .max()
        .unwrap_or(0)
        + 1
}

fn locate_custom_field(state: &AppState, org: &str, project: &str, bucket: &str, key: &str) -> Option<Value> {
    list(state, org, project, bucket).into_iter().find(|item| {
        value_key(&item["id"]) == key
            || value_key(&item["fieldId"]) == key
            || text_of(item, "fieldName").is_some_and(|stored| stored.eq_ignore_ascii_case(key))
    })
}

fn dress_custom_field(org: &str, project: &str, run_id: &str, run_name: &str, mut item: Value) -> Value {
    let id = value_key(&item["id"]);
    if let Some(object) = item.as_object_mut() {
        object.insert("runId".into(), json!(run_id));
        object.insert("runName".into(), json!(run_name));
        object.insert("project".into(), json!(project));
        object.insert(
            "url".into(),
            json!(format!("/{org}/{project}/_apis/testresults/runs/{run_id}/customfields/{id}")),
        );
    }
    item
}

fn test_run_tags(state: &AppState, method: &Method, org: &str, project: &str, rest: &[String], body: &[u8]) -> Response {
    let tail = if rest.len() > 4 { &rest[4..] } else { &[] };
    if tail.len() > 1 {
        return if method == Method::DELETE {
            fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée")
        } else {
            fail(StatusCode::NOT_FOUND, "étiquette introuvable")
        };
    }
    let run_key = pct_decode(rest.get(2).map(String::as_str).unwrap_or(""));
    let Some(run) = get_item(state, org, project, "test/runs", run_key.trim()) else {
        return fail(StatusCode::NOT_FOUND, "exécution introuvable");
    };
    let stored_run = value_key(&run["id"]);
    let run_name = text_of(&run, "name")
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| stored_run.clone());
    let bucket = format!("testresults/runs/{stored_run}/tags");
    if tail.is_empty() {
        if method == Method::GET {
            let mut rows = list(state, org, project, &bucket);
            rows.sort_by(|left, right| value_key(&left["id"]).cmp(&value_key(&right["id"])));
            let value = rows
                .into_iter()
                .map(|item| dress_test_run_tag(org, project, &stored_run, &run_name, item))
                .collect();
            return Json(page(value)).into_response();
        }
        if method == Method::POST {
            let name = match test_run_tag_payload(body) {
                Ok(name) => name,
                Err(message) => return fail(StatusCode::BAD_REQUEST, message),
            };
            return match test_run_tag_create(state, org, project, &stored_run, &run_name, &bucket, &name) {
                Ok(item) => Json(item).into_response(),
                Err(message) => fail(StatusCode::BAD_REQUEST, message),
            };
        }
        if method == Method::PATCH {
            let names = match test_run_tags_update(body) {
                Ok(names) => names,
                Err(message) => return fail(StatusCode::BAD_REQUEST, message),
            };
            state.write(|db| {
                if let Some(stored) = db.project_mut(org, project) {
                    stored.buckets.remove(&bucket);
                }
            });
            let mut rows = Vec::new();
            for name in names {
                match test_run_tag_create(state, org, project, &stored_run, &run_name, &bucket, &name) {
                    Ok(item) => rows.push(item),
                    Err(message) => return fail(StatusCode::BAD_REQUEST, message),
                }
            }
            return Json(rows).into_response();
        }
        return fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée");
    }
    let key = pct_decode(&tail[0]);
    let key = key.trim();
    if method == Method::GET {
        return match locate_test_run_tag(state, org, project, &bucket, key) {
            Some(found) => Json(dress_test_run_tag(org, project, &stored_run, &run_name, found)).into_response(),
            None => fail(StatusCode::NOT_FOUND, "étiquette introuvable"),
        };
    }
    if method == Method::DELETE {
        let Some(found) = locate_test_run_tag(state, org, project, &bucket, key) else {
            return fail(StatusCode::NOT_FOUND, "étiquette introuvable");
        };
        let stored_id = value_key(&found["id"]);
        return if delete_item(state, org, project, &bucket, &stored_id) {
            StatusCode::NO_CONTENT.into_response()
        } else {
            fail(StatusCode::NOT_FOUND, "étiquette introuvable")
        };
    }
    fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée")
}

fn test_run_tag_payload(body: &[u8]) -> Result<String, &'static str> {
    let Some(payload) = parse_json(body) else {
        return Err("JSON illisible");
    };
    let Some(object) = payload.as_object() else {
        return Err("étiquette illisible");
    };
    let Some(name) = object.get("name") else {
        return Err("nom absent");
    };
    test_tag_name(name)
}

fn test_run_tags_update(body: &[u8]) -> Result<Vec<String>, &'static str> {
    let Some(payload) = parse_json(body) else {
        return Err("JSON illisible");
    };
    let Some(object) = payload.as_object() else {
        return Err("étiquette illisible");
    };
    let Some(tags) = object.get("tags").and_then(Value::as_array) else {
        return Err("étiquette illisible");
    };
    let mut names = Vec::new();
    for item in tags {
        let Some(object) = item.as_object() else {
            return Err("étiquette illisible");
        };
        let Some(name) = object.get("name") else {
            return Err("nom absent");
        };
        let name = test_tag_name(name)?;
        if names.iter().any(|stored: &String| stored.eq_ignore_ascii_case(&name)) {
            return Err("étiquette existante");
        }
        names.push(name);
    }
    Ok(names)
}

fn test_tag_name(value: &Value) -> Result<String, &'static str> {
    let Some(text) = value.as_str() else {
        return Err("nom absent");
    };
    let text = text.trim();
    if text.is_empty() {
        return Err("nom absent");
    }
    if text.len() >= 30 || !text.chars().all(|char| char.is_ascii_alphanumeric()) {
        return Err("nom invalide");
    }
    Ok(text.to_string())
}

fn locate_test_run_tag(state: &AppState, org: &str, project: &str, bucket: &str, key: &str) -> Option<Value> {
    list(state, org, project, bucket)
        .into_iter()
        .find(|item| value_key(&item["id"]) == key || text_of(item, "name").is_some_and(|stored| stored.eq_ignore_ascii_case(key)))
}

fn test_run_tag_create(
    state: &AppState,
    org: &str,
    project: &str,
    run_id: &str,
    run_name: &str,
    bucket: &str,
    name: &str,
) -> Result<Value, &'static str> {
    if list(state, org, project, bucket)
        .iter()
        .any(|item| text_of(item, "name").is_some_and(|stored| stored.eq_ignore_ascii_case(name)))
    {
        return Err("étiquette existante");
    }
    let id = format!("tg-{}", next_test_run_tag_id(state, org));
    let item = json!({ "id": id, "name": name, "runId": run_id });
    put_item(state, org, project, bucket, item.clone());
    Ok(dress_test_run_tag(org, project, run_id, run_name, item))
}

fn next_test_run_tag_id(state: &AppState, org: &str) -> i64 {
    state
        .read(|db| {
            let Some(organization) = db.orgs.get(org) else {
                return Vec::new();
            };
            let mut rows = Vec::new();
            for project in organization.projects.values() {
                for (key, bucket) in &project.buckets {
                    if key.starts_with("testresults/runs/") && key.ends_with("/tags") {
                        rows.extend(bucket.items.values().cloned());
                    }
                }
            }
            rows
        })
        .iter()
        .filter_map(|item| value_key(&item["id"]).strip_prefix("tg-")?.parse::<i64>().ok())
        .max()
        .unwrap_or(0)
        + 1
}

fn dress_test_run_tag(org: &str, project: &str, run_id: &str, run_name: &str, mut item: Value) -> Value {
    let id = value_key(&item["id"]);
    if let Some(object) = item.as_object_mut() {
        object.insert("runId".into(), json!(run_id));
        object.insert("runName".into(), json!(run_name));
        object.insert("project".into(), json!(project));
        object.insert(
            "url".into(),
            json!(format!("/{org}/{project}/_apis/testresults/runs/{run_id}/tags/{id}")),
        );
    }
    item
}

fn test_log_store(
    state: &AppState,
    method: &Method,
    org: &str,
    project: &str,
    rest: &[String],
    query: &str,
    body: &[u8],
) -> Response {
    if rest.get(3).is_none_or(|part| !part.eq_ignore_ascii_case("testattachments")) {
        return if method == Method::DELETE {
            fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée")
        } else {
            fail(StatusCode::NOT_FOUND, "pièce introuvable")
        };
    }
    let tail = if rest.len() > 4 { &rest[4..] } else { &[] };
    if tail.len() > 1 {
        return if method == Method::DELETE {
            fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée")
        } else {
            fail(StatusCode::NOT_FOUND, "pièce introuvable")
        };
    }
    let run_key = pct_decode(rest.get(2).map(String::as_str).unwrap_or(""));
    let Some(run) = get_item(state, org, project, "test/runs", run_key.trim()) else {
        return fail(StatusCode::NOT_FOUND, "exécution introuvable");
    };
    let stored_run = value_key(&run["id"]);
    let run_name = text_of(&run, "name")
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| stored_run.clone());
    let bucket = format!("testresults/runs/{stored_run}/testattachments");
    if tail.is_empty() {
        if method == Method::GET {
            if let Some(filename) = query_text(query, "filename") {
                return match locate_run_attachment(state, org, project, &bucket, &filename) {
                    Some(found) => Json(dress_log_store_file(org, project, &stored_run, &run_name, found)).into_response(),
                    None => fail(StatusCode::NOT_FOUND, "pièce introuvable"),
                };
            }
            let mut rows = list(state, org, project, &bucket);
            rows.sort_by(|left, right| value_key(&left["id"]).cmp(&value_key(&right["id"])));
            let value = rows
                .into_iter()
                .map(|item| dress_log_store_file(org, project, &stored_run, &run_name, item))
                .collect();
            return Json(page(value)).into_response();
        }
        if method == Method::POST {
            let parsed = match run_attachment_payload(body) {
                Ok(parsed) => parsed,
                Err(message) => return fail(StatusCode::BAD_REQUEST, message),
            };
            return match log_store_file_create(state, org, project, &stored_run, &run_name, &bucket, parsed) {
                Ok(item) => Json(item).into_response(),
                Err(message) => fail(StatusCode::BAD_REQUEST, message),
            };
        }
        if method == Method::DELETE {
            let Some(filename) = query_text(query, "filename") else {
                return fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée");
            };
            let Some(found) = locate_run_attachment(state, org, project, &bucket, &filename) else {
                return fail(StatusCode::NOT_FOUND, "pièce introuvable");
            };
            let stored_id = value_key(&found["id"]);
            return if delete_item(state, org, project, &bucket, &stored_id) {
                StatusCode::NO_CONTENT.into_response()
            } else {
                fail(StatusCode::NOT_FOUND, "pièce introuvable")
            };
        }
        return fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée");
    }
    let key = pct_decode(&tail[0]);
    let key = key.trim();
    if method == Method::GET {
        return match locate_run_attachment(state, org, project, &bucket, key) {
            Some(found) => Json(dress_log_store_file(org, project, &stored_run, &run_name, found)).into_response(),
            None => fail(StatusCode::NOT_FOUND, "pièce introuvable"),
        };
    }
    if method == Method::DELETE {
        let Some(found) = locate_run_attachment(state, org, project, &bucket, key) else {
            return fail(StatusCode::NOT_FOUND, "pièce introuvable");
        };
        let stored_id = value_key(&found["id"]);
        return if delete_item(state, org, project, &bucket, &stored_id) {
            StatusCode::NO_CONTENT.into_response()
        } else {
            fail(StatusCode::NOT_FOUND, "pièce introuvable")
        };
    }
    fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée")
}

fn log_store_file_create(
    state: &AppState,
    org: &str,
    project: &str,
    run_id: &str,
    run_name: &str,
    bucket: &str,
    parsed: (String, String, String, Option<String>),
) -> Result<Value, &'static str> {
    let (file_name, comment, attachment_type, stream) = parsed;
    if list(state, org, project, bucket)
        .iter()
        .any(|item| text_of(item, "fileName").is_some_and(|stored| stored.eq_ignore_ascii_case(&file_name)))
    {
        return Err("pièce existante");
    }
    let id = format!("ls-{}", next_log_store_file_id(state, org));
    let mut item = json!({
        "id": id,
        "fileName": file_name,
        "comment": comment,
        "attachmentType": attachment_type,
        "runId": run_id
    });
    if let Some(stream) = stream {
        item["stream"] = json!(stream);
    }
    put_item(state, org, project, bucket, item.clone());
    Ok(dress_log_store_file(org, project, run_id, run_name, item))
}

fn next_log_store_file_id(state: &AppState, org: &str) -> i64 {
    state
        .read(|db| {
            let Some(organization) = db.orgs.get(org) else {
                return Vec::new();
            };
            let mut rows = Vec::new();
            for project in organization.projects.values() {
                for (key, bucket) in &project.buckets {
                    if key.starts_with("testresults/runs/") && key.ends_with("/testattachments") {
                        rows.extend(bucket.items.values().cloned());
                    }
                }
            }
            rows
        })
        .iter()
        .filter_map(|item| value_key(&item["id"]).strip_prefix("ls-")?.parse::<i64>().ok())
        .max()
        .unwrap_or(0)
        + 1
}

fn dress_log_store_file(org: &str, project: &str, run_id: &str, run_name: &str, mut item: Value) -> Value {
    let id = value_key(&item["id"]);
    if let Some(object) = item.as_object_mut() {
        object.insert("runId".into(), json!(run_id));
        object.insert("runName".into(), json!(run_name));
        object.insert("project".into(), json!(project));
        object.insert(
            "url".into(),
            json!(format!("/{org}/{project}/_apis/testresults/runs/{run_id}/testattachments/{id}")),
        );
    }
    item
}

const TEST_METHOD_LINKS: &str = "testresults/testmethods/workitems";

fn test_method_links(
    state: &AppState,
    method: &Method,
    org: &str,
    project: &str,
    rest: &[String],
    query: &str,
    body: &[u8],
) -> Response {
    if rest.get(2).is_none_or(|part| !part.eq_ignore_ascii_case("workitems")) {
        return if method == Method::DELETE {
            fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée")
        } else {
            fail(StatusCode::NOT_FOUND, "lien introuvable")
        };
    }
    let tail = if rest.len() > 3 { &rest[3..] } else { &[] };
    if tail.len() > 1 {
        return if method == Method::DELETE {
            fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée")
        } else {
            fail(StatusCode::NOT_FOUND, "lien introuvable")
        };
    }
    if tail.is_empty() {
        if method == Method::GET {
            let value = alive_test_method_links(state, org, project)
                .into_iter()
                .map(|item| dress_test_method_link(org, project, item))
                .collect();
            return Json(page(value)).into_response();
        }
        if method == Method::POST {
            if let Some(test_name) = query_text(query, "testName") {
                return query_test_method_links(state, org, project, &test_name);
            }
            let draft = match test_method_link_payload(body) {
                Ok(draft) => draft,
                Err(message) => return fail(StatusCode::BAD_REQUEST, message),
            };
            return match test_method_link_create(state, org, project, draft) {
                Ok(item) => Json(item).into_response(),
                Err("élément introuvable") => fail(StatusCode::NOT_FOUND, "élément introuvable"),
                Err(message) => fail(StatusCode::BAD_REQUEST, message),
            };
        }
        if method == Method::DELETE {
            let Some(test_name) = query_text(query, "testName") else {
                return fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée");
            };
            let Some(work_item_id) = query_text(query, "workItemId") else {
                return fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée");
            };
            return delete_test_method_link_query(state, org, project, &test_name, &work_item_id);
        }
        return fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée");
    }
    let key = pct_decode(&tail[0]);
    let key = key.trim();
    if method == Method::GET {
        return match locate_test_method_link(state, org, project, key) {
            LinkHit::Alive(found) => Json(dress_test_method_link(org, project, found)).into_response(),
            LinkHit::Orphan => fail(StatusCode::NOT_FOUND, "élément introuvable"),
            LinkHit::Missing => fail(StatusCode::NOT_FOUND, "lien introuvable"),
        };
    }
    if method == Method::DELETE {
        return match locate_test_method_link(state, org, project, key) {
            LinkHit::Alive(found) => {
                let stored_id = value_key(&found["id"]);
                if delete_item(state, org, project, TEST_METHOD_LINKS, &stored_id) {
                    StatusCode::NO_CONTENT.into_response()
                } else {
                    fail(StatusCode::NOT_FOUND, "lien introuvable")
                }
            }
            LinkHit::Orphan => fail(StatusCode::NOT_FOUND, "élément introuvable"),
            LinkHit::Missing => fail(StatusCode::NOT_FOUND, "lien introuvable"),
        };
    }
    fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée")
}

enum LinkHit {
    Alive(Value),
    Orphan,
    Missing,
}

fn query_text(query: &str, key: &str) -> Option<String> {
    query_param(query, key)
        .map(pct_decode)
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn locate_test_method_link(state: &AppState, org: &str, project: &str, key: &str) -> LinkHit {
    let Some(found) = list(state, org, project, TEST_METHOD_LINKS).into_iter().find(|item| {
        value_key(&item["id"]) == key || text_of(item, "name").is_some_and(|stored| stored.eq_ignore_ascii_case(key))
    }) else {
        return LinkHit::Missing;
    };
    let work_item_id = text_of(&found, "workItemId").and_then(|value| value.parse::<i64>().ok());
    match work_item_id {
        Some(id) if work_item_alive(state, org, project, id) => LinkHit::Alive(found),
        Some(_) => LinkHit::Orphan,
        None => LinkHit::Missing,
    }
}

fn alive_test_method_links(state: &AppState, org: &str, project: &str) -> Vec<Value> {
    let mut rows = list(state, org, project, TEST_METHOD_LINKS);
    rows.retain(|item| {
        text_of(item, "workItemId")
            .and_then(|value| value.parse::<i64>().ok())
            .is_some_and(|id| work_item_alive(state, org, project, id))
    });
    rows.sort_by(|left, right| value_key(&left["id"]).cmp(&value_key(&right["id"])));
    rows
}

fn work_item_alive(state: &AppState, org: &str, project: &str, id: i64) -> bool {
    state.read(|db| {
        db.project(org, project)
            .is_some_and(|stored| stored.work_items.contains_key(&id))
    })
}

fn work_item_record(state: &AppState, org: &str, project: &str, id: i64) -> Option<Value> {
    state.read(|db| {
        db.project(org, project)
            .and_then(|stored| stored.work_items.get(&id).cloned())
    })
}

fn query_test_method_links(state: &AppState, org: &str, project: &str, test_name: &str) -> Response {
    let matches: Vec<Value> = alive_test_method_links(state, org, project)
        .into_iter()
        .filter(|item| text_of(item, "name").is_some_and(|stored| stored.eq_ignore_ascii_case(test_name)))
        .collect();
    if matches.is_empty() {
        return match locate_test_method_link(state, org, project, test_name) {
            LinkHit::Orphan => fail(StatusCode::NOT_FOUND, "élément introuvable"),
            _ => fail(StatusCode::NOT_FOUND, "lien introuvable"),
        };
    }
    let container = text_of(&matches[0], "container").unwrap_or_default();
    let work_items: Vec<Value> = matches
        .iter()
        .map(|item| dress_test_method_link(org, project, item.clone())["workItem"].clone())
        .collect();
    Json(json!({
        "test": { "container": container, "name": text_of(&matches[0], "name").unwrap_or_else(|| test_name.to_string()) },
        "workItems": work_items
    }))
    .into_response()
}

fn delete_test_method_link_query(state: &AppState, org: &str, project: &str, test_name: &str, work_item_id: &str) -> Response {
    let Some(found) = list(state, org, project, TEST_METHOD_LINKS).into_iter().find(|item| {
        text_of(item, "name").is_some_and(|stored| stored.eq_ignore_ascii_case(test_name))
            && text_of(item, "workItemId").is_some_and(|stored| same_work_item(&stored, work_item_id))
    }) else {
        return fail(StatusCode::NOT_FOUND, "lien introuvable");
    };
    let Some(id) = text_of(&found, "workItemId").and_then(|value| value.parse::<i64>().ok()) else {
        return fail(StatusCode::NOT_FOUND, "lien introuvable");
    };
    if !work_item_alive(state, org, project, id) {
        return fail(StatusCode::NOT_FOUND, "élément introuvable");
    }
    let stored_id = value_key(&found["id"]);
    if delete_item(state, org, project, TEST_METHOD_LINKS, &stored_id) {
        StatusCode::NO_CONTENT.into_response()
    } else {
        fail(StatusCode::NOT_FOUND, "lien introuvable")
    }
}

fn same_work_item(stored: &str, query: &str) -> bool {
    if stored == query {
        return true;
    }
    matches!((stored.parse::<i64>(), query.parse::<i64>()), (Ok(left), Ok(right)) if left == right)
}

struct TestMethodLinkDraft {
    name: String,
    container: String,
    executed_in: String,
    work_item_id: Option<i64>,
    work_item_name: Option<String>,
    work_item_type: Option<String>,
}

fn test_method_link_payload(body: &[u8]) -> Result<TestMethodLinkDraft, &'static str> {
    let Some(payload) = parse_json(body) else {
        return Err("JSON illisible");
    };
    let Some(object) = payload.as_object() else {
        return Err("lien illisible");
    };
    let (name, container) = match object.get("tests") {
        None => (link_text(object.get("name"), "nom absent")?, String::new()),
        Some(Value::Array(rows)) => {
            if rows.is_empty() {
                (link_text(object.get("name"), "nom absent")?, String::new())
            } else {
                let Some(test) = rows[0].as_object() else {
                    return Err("nom absent");
                };
                let name = link_text(test.get("name"), "nom absent")?;
                let container = match test.get("container") {
                    None => String::new(),
                    Some(Value::String(value)) => value.trim().to_string(),
                    Some(_) => return Err("conteneur absent"),
                };
                (name, container)
            }
        }
        Some(_) => return Err("lien illisible"),
    };
    let executed_in = match object.get("executedIn") {
        None => "tcm".to_string(),
        Some(Value::String(value)) => {
            let value = value.trim();
            if value.is_empty() {
                return Err("contexte absent");
            }
            value.to_string()
        }
        Some(_) => return Err("contexte absent"),
    };
    let (work_item_id, work_item_name, work_item_type) = match object.get("workItem") {
        None => (None, None, None),
        Some(Value::Object(work_item)) => {
            let work_item_id = match work_item.get("id") {
                None => None,
                Some(value) => Some(positive_id(value)?),
            };
            let work_item_name = match work_item.get("name") {
                None => None,
                Some(value) => Some(link_text(Some(value), "élément absent")?),
            };
            let work_item_type = match work_item.get("type") {
                None => None,
                Some(Value::String(value)) => {
                    let value = value.trim();
                    if value.is_empty() {
                        return Err("type absent");
                    }
                    Some(value.to_string())
                }
                Some(_) => return Err("type absent"),
            };
            (work_item_id, work_item_name, work_item_type)
        }
        Some(_) => return Err("élément absent"),
    };
    Ok(TestMethodLinkDraft {
        name,
        container,
        executed_in,
        work_item_id,
        work_item_name,
        work_item_type,
    })
}

fn link_text(value: Option<&Value>, message: &'static str) -> Result<String, &'static str> {
    match value {
        Some(Value::String(text)) => {
            let text = text.trim();
            if text.is_empty() {
                Err(message)
            } else {
                Ok(text.to_string())
            }
        }
        _ => Err(message),
    }
}

fn positive_id(value: &Value) -> Result<i64, &'static str> {
    let id = match value {
        Value::Number(_) => json_i64(value),
        Value::String(text) => text.trim().parse::<i64>().ok(),
        _ => None,
    };
    id.filter(|id| *id > 0).ok_or("élément absent")
}

fn test_method_link_create(state: &AppState, org: &str, project: &str, draft: TestMethodLinkDraft) -> Result<Value, &'static str> {
    if alive_test_method_links(state, org, project)
        .iter()
        .any(|item| text_of(item, "name").is_some_and(|stored| stored.eq_ignore_ascii_case(&draft.name)))
    {
        return Err("lien existant");
    }
    let (work_item_id, work_item_name, work_item_type) = match draft.work_item_id {
        Some(id) => {
            let Some(item) = work_item_record(state, org, project, id) else {
                return Err("élément introuvable");
            };
            let fields = item.get("fields");
            let name = draft.work_item_name.unwrap_or_else(|| {
                fields
                    .and_then(|fields| text_of(fields, "System.Title"))
                    .unwrap_or_else(|| draft.name.clone())
            });
            let kind = draft.work_item_type.unwrap_or_else(|| {
                fields
                    .and_then(|fields| text_of(fields, "System.WorkItemType"))
                    .unwrap_or_else(|| "Bug".into())
            });
            (id, name, kind)
        }
        None => {
            let name = draft.work_item_name.unwrap_or_else(|| draft.name.clone());
            let kind = draft.work_item_type.unwrap_or_else(|| "Bug".into());
            let created = state.write(|db| {
                db.ensure_project(org, project);
                let stored = db.project_mut(org, project)?;
                let mut fields = Map::new();
                fields.insert("System.Title".into(), json!(name.clone()));
                Some(stored.create_work_item(project, &kind, fields))
            });
            let Some(item) = created else {
                return Err("élément introuvable");
            };
            deliver_work_item(state, org, project, "workitem.created", &item);
            let id = item.get("id").and_then(json_i64).ok_or("élément introuvable")?;
            (id, name, kind)
        }
    };
    let id = format!("tw-{}", next_test_method_link_id(state, org));
    let item = json!({
        "id": id,
        "name": draft.name,
        "container": draft.container,
        "executedIn": draft.executed_in,
        "workItemId": work_item_id.to_string(),
        "workItemName": work_item_name,
        "workItemType": work_item_type
    });
    put_item(state, org, project, TEST_METHOD_LINKS, item.clone());
    Ok(dress_test_method_link(org, project, item))
}

fn next_test_method_link_id(state: &AppState, org: &str) -> i64 {
    state
        .read(|db| {
            let Some(organization) = db.orgs.get(org) else {
                return Vec::new();
            };
            let mut rows = Vec::new();
            for project in organization.projects.values() {
                if let Some(bucket) = project.buckets.get(TEST_METHOD_LINKS) {
                    rows.extend(bucket.items.values().cloned());
                }
            }
            rows
        })
        .iter()
        .filter_map(|item| value_key(&item["id"]).strip_prefix("tw-")?.parse::<i64>().ok())
        .max()
        .unwrap_or(0)
        + 1
}

fn dress_test_method_link(org: &str, project: &str, mut item: Value) -> Value {
    let id = value_key(&item["id"]);
    let name = text_of(&item, "name").unwrap_or_default();
    let container = text_of(&item, "container").unwrap_or_default();
    let work_item_id = text_of(&item, "workItemId").unwrap_or_default();
    let work_item_name = text_of(&item, "workItemName").unwrap_or_else(|| name.clone());
    let work_item_type = text_of(&item, "workItemType").unwrap_or_else(|| "Bug".into());
    if let Some(object) = item.as_object_mut() {
        object.insert("project".into(), json!(project));
        object.insert("testName".into(), json!(name));
        object.insert(
            "tests".into(),
            json!([{ "container": container, "name": name }]),
        );
        object.insert(
            "workItem".into(),
            json!({
                "id": work_item_id,
                "name": work_item_name,
                "type": work_item_type,
                "url": format!("/{org}/{project}/_apis/wit/workitems/{work_item_id}"),
                "webUrl": format!("/{org}/{project}/_workitems?id={work_item_id}")
            }),
        );
        object.insert(
            "url".into(),
            json!(format!("/{org}/{project}/_apis/testresults/testmethods/workitems/{id}")),
        );
    }
    item
}

fn page_path_of(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    let mut path = if trimmed.starts_with('/') {
        trimmed.to_string()
    } else {
        format!("/{trimmed}")
    };
    while path.len() > 1 && path.ends_with('/') {
        path.pop();
    }
    path
}

fn page_under(parent: &str, path: &str) -> bool {
    path.starts_with(parent) && path.len() > parent.len() && path.as_bytes().get(parent.len()) == Some(&b'/')
}

fn page_depth(parent: &str, path: &str) -> Option<usize> {
    if !page_under(parent, path) {
        return None;
    }
    Some(path[parent.len() + 1..].split('/').filter(|part| !part.is_empty()).count())
}

fn relocate_page(path: &str, from: &str, to: &str) -> String {
    if path == from {
        to.to_string()
    } else {
        format!("{to}{}", &path[from.len()..])
    }
}

fn wiki_pages_key(wiki_key: &str) -> String {
    format!("wiki/wikis/{wiki_key}/pages")
}

fn wiki_comments_key(wiki_key: &str, page_path: &str) -> String {
    format!("wiki/wikis/{wiki_key}/pages/{page_path}/comments")
}

fn pct_encode_segment(raw: &str) -> String {
    let mut out = String::new();
    for byte in raw.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(byte as char),
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

fn wiki_page(org: &str, project: &str, wiki_key: &str, path: &str, content: &str) -> Value {
    json!({
        "id": path,
        "path": path,
        "content": content,
        "gitItemPath": path,
        "url": format!("/{org}/{project}/_apis/wiki/wikis/{wiki_key}/pages?path={path}")
    })
}

fn recursion_mode(raw: &str) -> Option<&'static str> {
    if raw.is_empty() || raw.eq_ignore_ascii_case("none") {
        Some("none")
    } else if raw.eq_ignore_ascii_case("oneLevel") {
        Some("one")
    } else if raw.eq_ignore_ascii_case("oneLevelPlusNested") || raw.eq_ignore_ascii_case("full") {
        Some("full")
    } else {
        None
    }
}

fn delete_wiki_page(state: &AppState, org: &str, project: &str, wiki_key: &str, query: &str) -> Response {
    let pages_key = wiki_pages_key(wiki_key);
    let Some(raw) = query_param(query, "path") else {
        return fail(StatusCode::BAD_REQUEST, "chemin de page absent");
    };
    let path = page_path_of(&pct_decode(raw));
    if path.is_empty() {
        return fail(StatusCode::BAD_REQUEST, "chemin de page absent");
    }
    let Some(page) = get_item(state, org, project, &pages_key, &path) else {
        return fail(StatusCode::NOT_FOUND, "page introuvable");
    };
    let requested = query_param(query, "recursionLevel").map(pct_decode).unwrap_or_default();
    let Some(mode) = recursion_mode(requested.trim()) else {
        return fail(StatusCode::BAD_REQUEST, "profondeur inconnue");
    };
    let stored = list(state, org, project, &pages_key);
    let mut doomed = vec![path.clone()];
    for item in &stored {
        let child = text_of(item, "path").unwrap_or_default();
        let Some(depth) = page_depth(&path, &child) else {
            continue;
        };
        if mode == "none" || (mode == "one" && depth > 1) {
            return fail(StatusCode::BAD_REQUEST, "sous-pages présentes");
        }
        doomed.push(child);
    }
    let removed = state.write(|db| {
        let bucket = db.project_mut(org, project)?.buckets.get_mut(&pages_key)?;
        let mut gone = true;
        for id in &doomed {
            gone = bucket.remove(id) && gone;
        }
        Some(gone)
    });
    if removed != Some(true) {
        return fail(StatusCode::NOT_FOUND, "page introuvable");
    }
    deliver_work_item(state, org, project, "wiki.page.deleted", &page);
    Json(page).into_response()
}

fn move_wiki_page(state: &AppState, method: &Method, org: &str, project: &str, wiki_id: &str, body: &[u8]) -> Response {
    let Some(wiki) = resolve_wiki(state, org, project, wiki_id) else {
        return fail(StatusCode::NOT_FOUND, "wiki introuvable");
    };
    if method != Method::POST {
        return fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée");
    }
    let Some(payload) = parse_json(body) else {
        return fail(StatusCode::BAD_REQUEST, "déplacement illisible");
    };
    let Some(object) = payload.as_object() else {
        return fail(StatusCode::BAD_REQUEST, "déplacement illisible");
    };
    let (Some(path_raw), Some(new_raw)) = (
        object.get("path").and_then(|value| value.as_str()),
        object.get("newPath").and_then(|value| value.as_str()),
    ) else {
        return fail(StatusCode::BAD_REQUEST, "déplacement illisible");
    };
    let path = page_path_of(path_raw);
    let new_path = page_path_of(new_raw);
    if path.is_empty() || new_path.is_empty() {
        return fail(StatusCode::BAD_REQUEST, "chemin de page absent");
    }
    if let Some(order) = object.get("newOrder") {
        if !order.is_number() {
            return fail(StatusCode::BAD_REQUEST, "ordre illisible");
        }
    }
    let wiki_key = value_key(&wiki["id"]);
    let pages_key = wiki_pages_key(&wiki_key);
    let Some(page) = get_item(state, org, project, &pages_key, &path) else {
        return fail(StatusCode::NOT_FOUND, "page introuvable");
    };
    if path == new_path {
        return Json(page).into_response();
    }
    if page_under(&path, &new_path) {
        return fail(StatusCode::BAD_REQUEST, "déplacement refusé");
    }
    let stored = list(state, org, project, &pages_key);
    let mut moving = Vec::new();
    let mut staying = Vec::new();
    for item in stored {
        let current = text_of(&item, "path").unwrap_or_default();
        if current == path || page_under(&path, &current) {
            moving.push(item);
        } else {
            staying.push(current);
        }
    }
    let destinations: Vec<String> = moving
        .iter()
        .map(|item| relocate_page(&text_of(item, "path").unwrap_or_default(), &path, &new_path))
        .collect();
    if destinations.iter().any(|destination| staying.iter().any(|kept| kept == destination)) {
        return fail(StatusCode::BAD_REQUEST, "page déjà présente");
    }
    let order = object.get("newOrder").cloned();
    let written = state.write(|db| {
        let bucket = db.project_mut(org, project)?.buckets.get_mut(&pages_key)?;
        let mut moved = None;
        for item in &moving {
            let current = text_of(item, "path").unwrap_or_default();
            let destination = relocate_page(&current, &path, &new_path);
            let content = text_of(item, "content").unwrap_or_default();
            let mut next = wiki_page(org, project, &wiki_key, &destination, &content);
            if current == path {
                if let (Some(fields), Some(order)) = (next.as_object_mut(), order.clone()) {
                    fields.insert("newOrder".into(), order);
                }
                moved = Some(next.clone());
            }
            bucket.remove(&current);
            bucket.put(next);
        }
        moved
    });
    let Some(moved) = written else {
        return fail(StatusCode::NOT_FOUND, "page introuvable");
    };
    deliver_work_item(state, org, project, "wiki.page.moved", &moved);
    Json(moved).into_response()
}

fn wiki_comment_text(body: &[u8]) -> Result<String, &'static str> {
    let Some(payload) = parse_json(body) else {
        return Err("JSON illisible");
    };
    let Some(text) = payload.as_object().and_then(|object| object.get("text")).and_then(|value| value.as_str()) else {
        return Err("texte absent");
    };
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Err("texte absent");
    }
    Ok(trimmed.to_string())
}

fn dress_wiki_comment(org: &str, project: &str, wiki_key: &str, page_path: &str, mut item: Value) -> Value {
    let id = value_key(&item["id"]);
    if let Some(object) = item.as_object_mut() {
        object.insert("pageId".into(), json!(page_path));
        object.insert("wikiId".into(), json!(wiki_key));
        object.insert(
            "url".into(),
            json!(format!(
                "/{org}/{project}/_apis/wiki/wikis/{wiki_key}/pages/{}/comments/{id}",
                pct_encode_segment(page_path)
            )),
        );
    }
    item
}

fn wiki_page_comments(
    state: &AppState,
    method: &Method,
    org: &str,
    project: &str,
    wiki_id: &str,
    rest: &[String],
    body: &[u8],
) -> Response {
    let Some(wiki) = resolve_wiki(state, org, project, wiki_id) else {
        return fail(StatusCode::NOT_FOUND, "wiki introuvable");
    };
    let wiki_key = value_key(&wiki["id"]);
    let page_path = page_path_of(rest.get(4).map(String::as_str).unwrap_or(""));
    if page_path.is_empty() || get_item(state, org, project, &wiki_pages_key(&wiki_key), &page_path).is_none() {
        return fail(StatusCode::NOT_FOUND, "page introuvable");
    }
    let key = wiki_comments_key(&wiki_key, &page_path);
    if rest.len() == 6 {
        if method == Method::GET {
            let mut rows = list(state, org, project, &key);
            rows.sort_by_key(|item| json_i64(&item["id"]).unwrap_or(0));
            let value = rows
                .into_iter()
                .map(|item| dress_wiki_comment(org, project, &wiki_key, &page_path, item))
                .collect();
            return Json(page(value)).into_response();
        }
        if method == Method::POST {
            let text = match wiki_comment_text(body) {
                Ok(text) => text,
                Err(message) => return fail(StatusCode::BAD_REQUEST, message),
            };
            let next = list(state, org, project, &key)
                .into_iter()
                .filter_map(|item| json_i64(&item["id"]))
                .max()
                .unwrap_or(0)
                + 1;
            let item = json!({"id": next, "text": text});
            put_item(state, org, project, &key, item.clone());
            return Json(dress_wiki_comment(org, project, &wiki_key, &page_path, item)).into_response();
        }
        return fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée");
    }
    if rest.len() == 7 {
        let Ok(comment_id) = rest[6].parse::<i64>() else {
            return fail(StatusCode::NOT_FOUND, "commentaire introuvable");
        };
        let id = comment_id.to_string();
        let Some(item) = get_item(state, org, project, &key, &id) else {
            return fail(StatusCode::NOT_FOUND, "commentaire introuvable");
        };
        if method == Method::GET {
            return Json(dress_wiki_comment(org, project, &wiki_key, &page_path, item)).into_response();
        }
        if method == Method::DELETE {
            return if delete_item(state, org, project, &key, &id) {
                StatusCode::NO_CONTENT.into_response()
            } else {
                fail(StatusCode::NOT_FOUND, "commentaire introuvable")
            };
        }
        return fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée");
    }
    if method == Method::DELETE {
        return fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée");
    }
    fail(StatusCode::NOT_FOUND, "commentaire introuvable")
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
        let next = list(state, org, project, key)
            .into_iter()
            .filter_map(|item| text_of(&item, "id"))
            .filter_map(|id| id.rsplit('-').next().and_then(|hex| i64::from_str_radix(hex, 16).ok()))
            .max()
            .unwrap_or(0)
            + 1;
        let id = format!("00000000-0000-0000-0000-{next:012x}");
        let item = dress_wiki(org, project, json!({"id": id, "name": name, "type": "projectWiki"}));
        put_item(state, org, project, key, item.clone());
        return Json(item).into_response();
    }
    let wiki_id = rest.get(2).map(String::as_str).unwrap_or("");
    if rest.len() >= 6
        && rest.get(3).is_some_and(|value| value.eq_ignore_ascii_case("pages"))
        && rest.get(5).is_some_and(|value| value.eq_ignore_ascii_case("comments"))
    {
        return wiki_page_comments(state, method, org, project, wiki_id, rest, body);
    }
    if rest.get(3).is_some_and(|value| value.eq_ignore_ascii_case("pagemoves")) {
        return move_wiki_page(state, method, org, project, wiki_id, body);
    }
    if rest.len() >= 4 && rest[3].eq_ignore_ascii_case("pages") && (method == Method::GET || method == Method::PUT || method == Method::POST || method == Method::DELETE) {
        let resolved = resolve_wiki(state, org, project, wiki_id);
        let Some(wiki) = resolved else {
            return fail(StatusCode::NOT_FOUND, "wiki introuvable");
        };
        let wiki_key = value_key(&wiki["id"]);
        let pages = wiki_pages_key(&wiki_key);
        if method == Method::DELETE {
            return delete_wiki_page(state, org, project, &wiki_key, query);
        }
        let path = query_param(query, "path")
            .map(|value| page_path_of(&pct_decode(value)))
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
        let item = wiki_page(org, project, &wiki_key, &path, &content);
        put_item(state, org, project, &pages, item.clone());
        return Json(item).into_response();
    }
    if method == Method::DELETE {
        if rest.len() != 3 {
            return fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée");
        }
        let Some(found) = resolve_wiki(state, org, project, wiki_id) else {
            return fail(StatusCode::NOT_FOUND, "wiki introuvable");
        };
        let stored_id = value_key(&found["id"]);
        state.write(|db| {
            if let Some(stored) = db.project_mut(org, project) {
                if let Some(bucket) = stored.buckets.get_mut(key) {
                    bucket.remove(&stored_id);
                }
            }
        });
        return StatusCode::NO_CONTENT.into_response();
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
        let next = list(state, org, project, key)
            .into_iter()
            .filter_map(|item| text_of(&item, "id"))
            .filter_map(|id| id.strip_prefix("feed-").and_then(|suffix| suffix.parse::<i64>().ok()))
            .max()
            .unwrap_or(0)
            + 1;
        let id = format!("feed-{next:08}");
        let item = dress_feed(org, project, json!({"id": id, "name": name}));
        put_item(state, org, project, key, item.clone());
        return Json(item).into_response();
    }
    if rest.len() == 2 && method == Method::DELETE {
        return fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée");
    }
    let feed_id = rest.get(2).map(String::as_str).unwrap_or("");
    if rest.len() >= 4 && rest[3].eq_ignore_ascii_case("packages") {
        if method == Method::DELETE && rest.len() != 5 {
            return fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée");
        }
        let Some(feed) = get_item(state, org, project, key, feed_id) else {
            return fail(StatusCode::NOT_FOUND, "flux introuvable");
        };
        let stored_feed = text_of(&feed, "id").unwrap_or_default();
        let packages = format!("packaging/feeds/{stored_feed}/packages");
        if rest.len() == 4 && method == Method::GET {
            return Json(page(list(state, org, project, &packages))).into_response();
        }
        if rest.len() == 4 && method == Method::POST {
            let payload = parse_json(body).unwrap_or(json!({}));
            let name = text_of(&payload, "name").unwrap_or_else(|| "package".into());
            let version = text_of(&payload, "version").unwrap_or_else(|| "1.0.0".into());
            let recycle = format!("packaging/feeds/{stored_feed}/recyclebin");
            let next = list(state, org, project, &packages)
                .into_iter()
                .chain(list(state, org, project, &recycle))
                .filter_map(|item| text_of(&item, "id"))
                .filter_map(|id| id.strip_prefix("pkg-").and_then(|suffix| suffix.parse::<i64>().ok()))
                .max()
                .unwrap_or(0)
                + 1;
            let item = json!({"id": format!("pkg-{next}"), "name": name, "versions": [{"version": version}]});
            put_item(state, org, project, &packages, item.clone());
            return Json(item).into_response();
        }
        if rest.len() == 5 && (method == Method::GET || method == Method::DELETE) {
            let package_id = rest[4].as_str();
            let Some(found) = get_item(state, org, project, &packages, package_id) else {
                return fail(StatusCode::NOT_FOUND, "paquet introuvable");
            };
            if method == Method::GET {
                return Json(found).into_response();
            }
            let stored_id = text_of(&found, "id").unwrap_or_default();
            let recycle = format!("packaging/feeds/{stored_feed}/recyclebin");
            put_item(state, org, project, &recycle, found);
            delete_item(state, org, project, &packages, &stored_id);
            return StatusCode::NO_CONTENT.into_response();
        }
        return fail(StatusCode::NOT_FOUND, "paquet introuvable");
    }
    if rest.len() >= 4 && rest[3].eq_ignore_ascii_case("recyclebin") {
        return feed_recycle(state, method, org, project, feed_id, rest, body);
    }
    if rest.len() >= 4 && rest[3].eq_ignore_ascii_case("retentionpolicies") {
        return feed_retention(state, method, org, project, feed_id, rest, body);
    }
    if rest.len() >= 4 && rest[3].eq_ignore_ascii_case("permissions") {
        return feed_permissions(state, method, org, project, feed_id, rest, body);
    }
    if rest.len() >= 4 && rest[3].eq_ignore_ascii_case("upstreamsources") {
        return feed_upstreams(state, method, org, project, feed_id, rest, body);
    }
    if rest.len() >= 4 && rest[3].eq_ignore_ascii_case("views") {
        return feed_views(state, method, org, project, feed_id, rest, body);
    }
    if method == Method::GET {
        return match get_item(state, org, project, key, feed_id) {
            Some(item) => Json(dress_feed(org, project, item)).into_response(),
            None => fail(StatusCode::NOT_FOUND, "flux introuvable"),
        };
    }
    if method == Method::DELETE {
        if rest.len() != 3 {
            return fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée");
        }
        let Some(found) = get_item(state, org, project, key, feed_id) else {
            return fail(StatusCode::NOT_FOUND, "flux introuvable");
        };
        let stored_id = text_of(&found, "id").unwrap_or_default();
        state.write(|db| {
            let Some(stored) = db.project_mut(org, project) else {
                return;
            };
            if let Some(bucket) = stored.buckets.get_mut(key) {
                bucket.remove(&stored_id);
            }
            let prefix = format!("packaging/feeds/{stored_id}/");
            let drop_keys: Vec<String> = stored.buckets.keys().filter(|name| name.starts_with(&prefix)).cloned().collect();
            for drop_key in drop_keys {
                stored.buckets.remove(&drop_key);
            }
        });
        return StatusCode::NO_CONTENT.into_response();
    }
    fail(StatusCode::NOT_FOUND, "flux introuvable")
}

fn feed_recycle(state: &AppState, method: &Method, org: &str, project: &str, feed_id: &str, rest: &[String], body: &[u8]) -> Response {
    let Some(feed) = get_item(state, org, project, "packaging/feeds", feed_id) else {
        return fail(StatusCode::NOT_FOUND, "flux introuvable");
    };
    let stored_feed = text_of(&feed, "id").unwrap_or_default();
    let key = format!("packaging/feeds/{stored_feed}/recyclebin");
    let packages = format!("packaging/feeds/{stored_feed}/packages");
    let on_packages = rest.get(4).is_some_and(|part| part.eq_ignore_ascii_case("packages"));
    if !on_packages {
        if method == Method::DELETE {
            return fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée");
        }
        return fail(StatusCode::NOT_FOUND, "paquet introuvable");
    }
    if rest.len() == 5 {
        if method == Method::GET {
            let mut rows = list(state, org, project, &key);
            rows.sort_by(|left, right| value_key(&left["id"]).cmp(&value_key(&right["id"])));
            let value = rows
                .into_iter()
                .map(|item| dress_recycled_package(org, project, &stored_feed, item))
                .collect();
            return Json(page(value)).into_response();
        }
        return fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée");
    }
    if rest.len() != 6 {
        if method == Method::DELETE {
            return fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée");
        }
        return fail(StatusCode::NOT_FOUND, "paquet introuvable");
    }
    let package_id = rest[5].as_str();
    if method == Method::GET {
        return match get_item(state, org, project, &key, package_id) {
            Some(item) => Json(dress_recycled_package(org, project, &stored_feed, item)).into_response(),
            None => fail(StatusCode::NOT_FOUND, "paquet introuvable"),
        };
    }
    if method == Method::DELETE {
        let Some(found) = get_item(state, org, project, &key, package_id) else {
            return fail(StatusCode::NOT_FOUND, "paquet introuvable");
        };
        let stored_id = text_of(&found, "id").unwrap_or_default();
        return if delete_item(state, org, project, &key, &stored_id) {
            StatusCode::NO_CONTENT.into_response()
        } else {
            fail(StatusCode::NOT_FOUND, "paquet introuvable")
        };
    }
    if method == Method::PATCH {
        let Some(found) = get_item(state, org, project, &key, package_id) else {
            return fail(StatusCode::NOT_FOUND, "paquet introuvable");
        };
        if let Err(message) = recycle_restore(body) {
            return fail(StatusCode::BAD_REQUEST, message);
        }
        let stored_id = text_of(&found, "id").unwrap_or_default();
        put_item(state, org, project, &packages, found.clone());
        delete_item(state, org, project, &key, &stored_id);
        return Json(found).into_response();
    }
    fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée")
}

fn recycle_restore(body: &[u8]) -> Result<(), &'static str> {
    let Some(payload) = parse_json(body) else {
        return Err("JSON illisible");
    };
    let Some(object) = payload.as_object() else {
        return Err("restauration absente");
    };
    match object.get("deleted") {
        Some(Value::Bool(false)) => Ok(()),
        _ => Err("restauration absente"),
    }
}

fn dress_recycled_package(org: &str, project: &str, feed_id: &str, mut item: Value) -> Value {
    if let Some(object) = item.as_object_mut() {
        let id = value_key(&object["id"]);
        object.insert("isDeleted".into(), json!(true));
        object.insert(
            "url".into(),
            json!(format!("/{org}/{project}/_apis/packaging/feeds/{feed_id}/recyclebin/packages/{id}")),
        );
    }
    item
}

fn feed_retention(state: &AppState, method: &Method, org: &str, project: &str, feed_id: &str, rest: &[String], body: &[u8]) -> Response {
    let Some(feed) = get_item(state, org, project, "packaging/feeds", feed_id) else {
        return fail(StatusCode::NOT_FOUND, "flux introuvable");
    };
    let stored_feed = text_of(&feed, "id").unwrap_or_default();
    let key = format!("packaging/feeds/{stored_feed}/retentionpolicies");
    if rest.len() != 4 {
        if method == Method::DELETE {
            return fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée");
        }
        return fail(StatusCode::NOT_FOUND, "politique introuvable");
    }
    if method == Method::GET {
        return match get_item(state, org, project, &key, "policy") {
            Some(item) => Json(dress_feed_retention(org, project, &stored_feed, item)).into_response(),
            None => fail(StatusCode::NOT_FOUND, "politique introuvable"),
        };
    }
    if method == Method::PUT {
        let item = match retention_policy(body) {
            Ok(item) => item,
            Err(message) => return fail(StatusCode::BAD_REQUEST, message),
        };
        put_item(state, org, project, &key, item.clone());
        return Json(dress_feed_retention(org, project, &stored_feed, item)).into_response();
    }
    if method == Method::DELETE {
        return if delete_item(state, org, project, &key, "policy") {
            StatusCode::NO_CONTENT.into_response()
        } else {
            fail(StatusCode::NOT_FOUND, "politique introuvable")
        };
    }
    fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée")
}

fn retention_policy(body: &[u8]) -> Result<Value, &'static str> {
    let Some(payload) = parse_json(body) else {
        return Err("JSON illisible");
    };
    let Some(object) = payload.as_object() else {
        return Err("politique absente");
    };
    if let Some(age) = object.get("ageLimitInDays") {
        retention_limit(age, "limite d'âge absente")?;
    }
    let count = match object.get("countLimit") {
        None => None,
        Some(value) => Some(retention_limit(value, "limite absente")?),
    };
    let days = match object.get("daysToKeepRecentlyDownloadedPackages") {
        None => None,
        Some(value) => Some(retention_limit(value, "durée absente")?),
    };
    if count.is_none() && days.is_none() {
        return Err("politique absente");
    }
    let mut item = Map::new();
    item.insert("id".into(), json!("policy"));
    if let Some(count) = count {
        item.insert("countLimit".into(), json!(count));
    }
    if let Some(days) = days {
        item.insert("daysToKeepRecentlyDownloadedPackages".into(), json!(days));
    }
    Ok(Value::Object(item))
}

fn retention_limit(value: &Value, missing: &'static str) -> Result<i64, &'static str> {
    let Some(number) = json_i64(value) else {
        return Err(missing);
    };
    if number < 0 || number > i64::from(i32::MAX) {
        return Err(missing);
    }
    Ok(number)
}

fn dress_feed_retention(org: &str, project: &str, feed_id: &str, mut item: Value) -> Value {
    if let Some(object) = item.as_object_mut() {
        object.remove("id");
        object.insert(
            "url".into(),
            json!(format!("/{org}/{project}/_apis/packaging/feeds/{feed_id}/retentionpolicies")),
        );
    }
    item
}

fn feed_permissions(state: &AppState, method: &Method, org: &str, project: &str, feed_id: &str, rest: &[String], body: &[u8]) -> Response {
    let Some(feed) = get_item(state, org, project, "packaging/feeds", feed_id) else {
        return fail(StatusCode::NOT_FOUND, "flux introuvable");
    };
    let stored_feed = text_of(&feed, "id").unwrap_or_default();
    let key = format!("packaging/feeds/{stored_feed}/permissions");
    if rest.len() == 4 {
        if method == Method::GET {
            let mut rows = list(state, org, project, &key);
            rows.sort_by(|left, right| {
                value_key(&left["id"])
                    .to_ascii_lowercase()
                    .cmp(&value_key(&right["id"]).to_ascii_lowercase())
            });
            let value = rows
                .into_iter()
                .map(|item| dress_feed_permission(org, project, &stored_feed, item))
                .collect();
            return Json(page(value)).into_response();
        }
        if method == Method::POST || method == Method::PATCH {
            let entries = match permission_entries(body) {
                Ok(entries) => entries,
                Err(message) => return fail(StatusCode::BAD_REQUEST, message),
            };
            let stored: Vec<Value> = entries
                .into_iter()
                .map(|(identity, role)| store_feed_permission(state, org, project, &key, &stored_feed, identity, role))
                .collect();
            if stored.len() == 1 && permission_body_is_object(body) {
                return Json(stored[0].clone()).into_response();
            }
            return Json(page(stored)).into_response();
        }
        return fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée");
    }
    if rest.len() != 5 {
        if method == Method::DELETE {
            return fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée");
        }
        return fail(StatusCode::NOT_FOUND, "permission introuvable");
    }
    let wanted = rest[4].as_str();
    let Some(found) = list(state, org, project, &key)
        .into_iter()
        .find(|item| value_key(&item["id"]).eq_ignore_ascii_case(wanted))
    else {
        return fail(StatusCode::NOT_FOUND, "permission introuvable");
    };
    let id = value_key(&found["id"]);
    if method == Method::GET {
        return Json(dress_feed_permission(org, project, &stored_feed, found)).into_response();
    }
    if method == Method::DELETE {
        return if delete_item(state, org, project, &key, &id) {
            StatusCode::NO_CONTENT.into_response()
        } else {
            fail(StatusCode::NOT_FOUND, "permission introuvable")
        };
    }
    if method == Method::PATCH {
        let role = match permission_role_only(body) {
            Ok(role) => role,
            Err(message) => return fail(StatusCode::BAD_REQUEST, message),
        };
        let item = json!({ "id": id, "role": role });
        put_item(state, org, project, &key, item.clone());
        return Json(dress_feed_permission(org, project, &stored_feed, item)).into_response();
    }
    fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée")
}

fn permission_body_is_object(body: &[u8]) -> bool {
    parse_json(body).and_then(|value| value.as_object().map(|_| ())).is_some()
}

fn store_feed_permission(
    state: &AppState,
    org: &str,
    project: &str,
    key: &str,
    feed_id: &str,
    identity: String,
    role: &'static str,
) -> Value {
    let id = list(state, org, project, key)
        .into_iter()
        .find(|item| value_key(&item["id"]).eq_ignore_ascii_case(&identity))
        .map(|item| value_key(&item["id"]))
        .unwrap_or(identity);
    let item = json!({ "id": id, "role": role });
    put_item(state, org, project, key, item.clone());
    dress_feed_permission(org, project, feed_id, item)
}

fn permission_entries(body: &[u8]) -> Result<Vec<(String, &'static str)>, &'static str> {
    let Some(payload) = parse_json(body) else {
        return Err("JSON illisible");
    };
    match payload {
        Value::Array(rows) => {
            if rows.is_empty() {
                return Err("identité absente");
            }
            rows.iter().map(permission_entry).collect()
        }
        Value::Object(_) => permission_entry(&payload).map(|entry| vec![entry]),
        _ => Err("identité absente"),
    }
}

fn permission_entry(value: &Value) -> Result<(String, &'static str), &'static str> {
    let Some(object) = value.as_object() else {
        return Err("identité absente");
    };
    let identity = permission_identity(object)?;
    let role = permission_role(object)?;
    Ok((identity, role))
}

fn permission_identity(object: &Map<String, Value>) -> Result<String, &'static str> {
    if object.contains_key("identity") {
        return required_text(object.get("identity"));
    }
    if object.contains_key("displayName") {
        return required_text(object.get("displayName"));
    }
    match object.get("identityDescriptor") {
        Some(Value::String(text)) => nonempty_identity(text),
        Some(Value::Object(descriptor)) => required_text(descriptor.get("identifier")),
        Some(_) => Err("identité absente"),
        None => Err("identité absente"),
    }
}

fn required_text(value: Option<&Value>) -> Result<String, &'static str> {
    match value {
        Some(Value::String(text)) => nonempty_identity(text),
        _ => Err("identité absente"),
    }
}

fn nonempty_identity(text: &str) -> Result<String, &'static str> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        Err("identité absente")
    } else {
        Ok(trimmed.to_string())
    }
}

fn permission_role(object: &Map<String, Value>) -> Result<&'static str, &'static str> {
    match object.get("role") {
        Some(Value::String(text)) => canonical_feed_role(text),
        _ => Err("rôle absent"),
    }
}

fn permission_role_only(body: &[u8]) -> Result<&'static str, &'static str> {
    let Some(payload) = parse_json(body) else {
        return Err("JSON illisible");
    };
    let Some(object) = payload.as_object() else {
        return Err("rôle absent");
    };
    permission_role(object)
}

fn canonical_feed_role(text: &str) -> Result<&'static str, &'static str> {
    match text.trim().to_ascii_lowercase().as_str() {
        "" => Err("rôle absent"),
        "reader" => Ok("reader"),
        "contributor" => Ok("contributor"),
        "administrator" => Ok("administrator"),
        "collaborator" => Ok("collaborator"),
        _ => Err("rôle inconnu"),
    }
}

fn dress_feed_permission(org: &str, project: &str, feed_id: &str, mut item: Value) -> Value {
    let id = value_key(&item["id"]);
    if let Some(object) = item.as_object_mut() {
        object.insert("identity".into(), json!(id));
        object.insert("displayName".into(), json!(id));
        object.insert(
            "identityDescriptor".into(),
            json!({ "identifier": id, "identityType": "Microsoft.TeamFoundation.Identity" }),
        );
        object.insert("identityId".into(), json!(object_id(&id)));
        object.insert("isInheritedRole".into(), json!(false));
        object.insert(
            "url".into(),
            json!(format!("/{org}/{project}/_apis/packaging/feeds/{feed_id}/permissions/{id}")),
        );
    }
    item
}

fn feed_upstreams(state: &AppState, method: &Method, org: &str, project: &str, feed_id: &str, rest: &[String], body: &[u8]) -> Response {
    let Some(feed) = get_item(state, org, project, "packaging/feeds", feed_id) else {
        return fail(StatusCode::NOT_FOUND, "flux introuvable");
    };
    let stored_feed = text_of(&feed, "id").unwrap_or_default();
    let key = format!("packaging/feeds/{stored_feed}/upstreams");
    if rest.len() == 4 {
        if method == Method::GET {
            let mut rows = list(state, org, project, &key);
            rows.sort_by_key(|item| json_i64(&item["id"]).unwrap_or(0));
            let value = rows
                .into_iter()
                .map(|item| dress_feed_upstream(org, project, &stored_feed, item))
                .collect();
            return Json(page(value)).into_response();
        }
        if method == Method::POST {
            let (name, location, protocol) = match upstream_fields(body) {
                Ok(fields) => fields,
                Err(message) => return fail(StatusCode::BAD_REQUEST, message),
            };
            if let Some(existing) = list(state, org, project, &key)
                .into_iter()
                .find(|item| text_of(item, "name").is_some_and(|current| current.eq_ignore_ascii_case(&name)))
            {
                return Json(dress_feed_upstream(org, project, &stored_feed, existing)).into_response();
            }
            let next = list(state, org, project, &key)
                .into_iter()
                .filter_map(|item| json_i64(&item["id"]))
                .max()
                .unwrap_or(0)
                + 1;
            let item = json!({ "id": next, "name": name, "location": location, "protocol": protocol });
            put_item(state, org, project, &key, item.clone());
            return Json(dress_feed_upstream(org, project, &stored_feed, item)).into_response();
        }
        return fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée");
    }
    if rest.len() != 5 {
        if method == Method::DELETE {
            return fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée");
        }
        return fail(StatusCode::NOT_FOUND, "source introuvable");
    }
    let Ok(source_id) = rest[4].parse::<i64>() else {
        return fail(StatusCode::NOT_FOUND, "source introuvable");
    };
    let id = source_id.to_string();
    let Some(item) = get_item(state, org, project, &key, &id) else {
        return fail(StatusCode::NOT_FOUND, "source introuvable");
    };
    if json_i64(&item["id"]) != Some(source_id) {
        return fail(StatusCode::NOT_FOUND, "source introuvable");
    }
    if method == Method::GET {
        return Json(dress_feed_upstream(org, project, &stored_feed, item)).into_response();
    }
    if method == Method::DELETE {
        return if delete_item(state, org, project, &key, &id) {
            StatusCode::NO_CONTENT.into_response()
        } else {
            fail(StatusCode::NOT_FOUND, "source introuvable")
        };
    }
    fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée")
}

fn upstream_fields(body: &[u8]) -> Result<(String, String, &'static str), &'static str> {
    let Some(payload) = parse_json(body) else {
        return Err("JSON illisible");
    };
    let Some(object) = payload.as_object() else {
        return Err("nom absent");
    };
    let Some(name) = object.get("name").and_then(|value| value.as_str()).map(str::trim) else {
        return Err("nom absent");
    };
    if name.is_empty() {
        return Err("nom absent");
    }
    let Some(location) = object.get("location").and_then(|value| value.as_str()).map(str::trim) else {
        return Err("emplacement absent");
    };
    if location.is_empty() {
        return Err("emplacement absent");
    }
    let Some(protocol) = object.get("protocol").and_then(|value| value.as_str()).map(str::trim) else {
        return Err("protocole absent");
    };
    if protocol.is_empty() {
        return Err("protocole absent");
    }
    let protocol = match protocol.to_ascii_lowercase().as_str() {
        "nuget" => "nuget",
        "npm" => "npm",
        "maven" => "maven",
        "pypi" => "pypi",
        "upack" => "upack",
        _ => return Err("protocole inconnu"),
    };
    Ok((name.to_string(), location.to_string(), protocol))
}

fn dress_feed_upstream(org: &str, project: &str, feed_id: &str, mut item: Value) -> Value {
    let id = value_key(&item["id"]);
    let location = text_of(&item, "location").unwrap_or_default();
    if let Some(object) = item.as_object_mut() {
        object.insert("displayLocation".into(), json!(location));
        object.insert("upstreamSourceType".into(), json!("public"));
        object.insert("status".into(), json!("ok"));
        object.insert(
            "url".into(),
            json!(format!("/{org}/{project}/_apis/packaging/feeds/{feed_id}/upstreamsources/{id}")),
        );
    }
    item
}

fn feed_views(state: &AppState, method: &Method, org: &str, project: &str, feed_id: &str, rest: &[String], body: &[u8]) -> Response {
    let Some(feed) = get_item(state, org, project, "packaging/feeds", feed_id) else {
        return fail(StatusCode::NOT_FOUND, "flux introuvable");
    };
    let stored_feed = text_of(&feed, "id").unwrap_or_default();
    let feed_name = text_of(&feed, "name").unwrap_or_default();
    let key = format!("packaging/feeds/{stored_feed}/views");
    if rest.len() == 4 {
        if method == &Method::GET {
            let mut rows = default_feed_views();
            rows.extend(list(state, org, project, &key));
            rows.sort_by(|left, right| text_of(left, "name").unwrap_or_default().cmp(&text_of(right, "name").unwrap_or_default()));
            let value = rows
                .into_iter()
                .map(|item| dress_feed_view(org, project, &stored_feed, &feed_name, item))
                .collect();
            return Json(page(value)).into_response();
        }
        if method == &Method::POST {
            return create_feed_view(state, org, project, &stored_feed, &feed_name, body);
        }
        return fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée");
    }
    if rest.len() != 5 {
        if method == &Method::DELETE {
            return fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée");
        }
        return fail(StatusCode::NOT_FOUND, "vue introuvable");
    }
    let wanted = pct_decode(&rest[4]).trim().to_string();
    if wanted.is_empty() {
        return fail(StatusCode::NOT_FOUND, "vue introuvable");
    }
    if reserved_feed_view(&wanted) {
        if method == &Method::GET {
            let item = default_feed_views()
                .into_iter()
                .find(|item| text_of(item, "name").is_some_and(|name| name.eq_ignore_ascii_case(&wanted)))
                .unwrap_or_else(|| json!({"id": wanted, "name": wanted, "visibility": "private"}));
            return Json(dress_feed_view(org, project, &stored_feed, &feed_name, item)).into_response();
        }
        if method == &Method::DELETE {
            return fail(StatusCode::BAD_REQUEST, "vue réservée");
        }
        return fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée");
    }
    let Some(item) = find_feed_view(&list(state, org, project, &key), &wanted) else {
        return fail(StatusCode::NOT_FOUND, "vue introuvable");
    };
    let id = value_key(&item["id"]);
    if method == &Method::GET {
        return Json(dress_feed_view(org, project, &stored_feed, &feed_name, item)).into_response();
    }
    if method == &Method::DELETE {
        return if delete_item(state, org, project, &key, &id) {
            StatusCode::NO_CONTENT.into_response()
        } else {
            fail(StatusCode::NOT_FOUND, "vue introuvable")
        };
    }
    fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée")
}

fn create_feed_view(state: &AppState, org: &str, project: &str, feed_id: &str, feed_name: &str, body: &[u8]) -> Response {
    let Some(payload) = parse_json(body) else {
        return fail(StatusCode::BAD_REQUEST, "JSON illisible");
    };
    let Some(object) = payload.as_object() else {
        return fail(StatusCode::BAD_REQUEST, "nom absent");
    };
    let name = object.get("name").and_then(|value| value.as_str()).map(str::trim).unwrap_or("");
    if name.is_empty() {
        return fail(StatusCode::BAD_REQUEST, "nom absent");
    }
    if reserved_feed_view(name) {
        return fail(StatusCode::BAD_REQUEST, "vue réservée");
    }
    let visibility = match object.get("visibility") {
        None => "private".to_string(),
        Some(value) => match value.as_str().map(str::trim) {
            Some(text) if text.eq_ignore_ascii_case("private") || text.eq_ignore_ascii_case("organization") || text.eq_ignore_ascii_case("collection") => {
                text.to_ascii_lowercase()
            }
            _ => return fail(StatusCode::BAD_REQUEST, "visibilité absente"),
        },
    };
    let key = format!("packaging/feeds/{feed_id}/views");
    if find_feed_view(&list(state, org, project, &key), name).is_some() {
        return fail(StatusCode::BAD_REQUEST, "vue déjà présente");
    }
    let item = json!({"id": name, "name": name, "visibility": visibility});
    put_item(state, org, project, &key, item.clone());
    Json(dress_feed_view(org, project, feed_id, feed_name, item)).into_response()
}

fn default_feed_views() -> Vec<Value> {
    vec![
        json!({"id": "@Local", "name": "@Local", "visibility": "private"}),
        json!({"id": "@Prerelease", "name": "@Prerelease", "visibility": "private"}),
        json!({"id": "@Release", "name": "@Release", "visibility": "private"}),
    ]
}

fn reserved_feed_view(name: &str) -> bool {
    let trimmed = name.trim();
    trimmed.eq_ignore_ascii_case("@Local") || trimmed.eq_ignore_ascii_case("@Prerelease") || trimmed.eq_ignore_ascii_case("@Release")
}

fn find_feed_view(rows: &[Value], name: &str) -> Option<Value> {
    rows.iter()
        .find(|item| {
            text_of(item, "id").is_some_and(|id| id.eq_ignore_ascii_case(name))
                || text_of(item, "name").is_some_and(|current| current.eq_ignore_ascii_case(name))
        })
        .cloned()
}

fn dress_feed_view(org: &str, project: &str, feed_id: &str, feed_name: &str, mut item: Value) -> Value {
    let id = value_key(&item["id"]);
    if let Some(object) = item.as_object_mut() {
        object.entry("visibility").or_insert(json!("private"));
        object.insert("feed".into(), json!({"id": feed_id, "name": feed_name}));
        object.insert(
            "url".into(),
            json!(format!("/{org}/{project}/_apis/packaging/feeds/{feed_id}/views/{id}")),
        );
    }
    item
}

fn release_folders(state: &AppState, method: &Method, org: &str, project: &str, rest: &[String], body: &[u8]) -> Response {
    let key = "release/folders";
    if rest.len() == 2 {
        if method == &Method::GET {
            let mut rows = list(state, org, project, key);
            rows.push(json!({"id": "\\", "path": "\\", "description": ""}));
            rows.sort_by(|left, right| text_of(left, "path").unwrap_or_default().cmp(&text_of(right, "path").unwrap_or_default()));
            let value = rows.into_iter().map(|item| dress_release_folder(org, project, item)).collect();
            return Json(page(value)).into_response();
        }
        if method == &Method::POST {
            return create_release_folder(state, org, project, body);
        }
        return fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée");
    }
    let raw = rest[2..].iter().map(|part| pct_decode(part)).collect::<Vec<_>>().join("/");
    let path = match canonical_folder_path(&raw) {
        Ok(path) => path,
        Err("dossier racine") => {
            if method == &Method::GET {
                return Json(dress_release_folder(org, project, json!({"id": "\\", "path": "\\", "description": ""}))).into_response();
            }
            if method == &Method::DELETE {
                return fail(StatusCode::BAD_REQUEST, "dossier racine");
            }
            return fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée");
        }
        Err(message) => return fail(StatusCode::NOT_FOUND, message),
    };
    if method == &Method::GET {
        return match find_folder(&list(state, org, project, key), &path) {
            Some(item) => Json(dress_release_folder(org, project, item)).into_response(),
            None => fail(StatusCode::NOT_FOUND, "dossier introuvable"),
        };
    }
    if method == &Method::DELETE {
        let rows = list(state, org, project, key);
        let Some(item) = find_folder(&rows, &path) else {
            return fail(StatusCode::NOT_FOUND, "dossier introuvable");
        };
        let id = value_key(&item["id"]);
        let prefix = format!("{path}\\");
        for child in &rows {
            let child_path = text_of(child, "path").unwrap_or_default();
            if child_path.eq_ignore_ascii_case(&path) {
                continue;
            }
            if child_path.len() > prefix.len() && child_path[..prefix.len()].eq_ignore_ascii_case(&prefix) {
                delete_item(state, org, project, key, &value_key(&child["id"]));
            }
        }
        return if delete_item(state, org, project, key, &id) {
            StatusCode::NO_CONTENT.into_response()
        } else {
            fail(StatusCode::NOT_FOUND, "dossier introuvable")
        };
    }
    fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée")
}

fn create_release_folder(state: &AppState, org: &str, project: &str, body: &[u8]) -> Response {
    let Some(payload) = parse_json(body) else {
        return fail(StatusCode::BAD_REQUEST, "JSON illisible");
    };
    let Some(object) = payload.as_object() else {
        return fail(StatusCode::BAD_REQUEST, "chemin absent");
    };
    let raw = object.get("path").and_then(|value| value.as_str()).unwrap_or("");
    if raw.trim().is_empty() {
        return fail(StatusCode::BAD_REQUEST, "chemin absent");
    }
    let path = match canonical_folder_path(raw) {
        Ok(path) => path,
        Err(message) => return fail(StatusCode::BAD_REQUEST, message),
    };
    let description = match object.get("description") {
        None => String::new(),
        Some(value) => match value.as_str() {
            Some(text) => text.trim().to_string(),
            None => return fail(StatusCode::BAD_REQUEST, "description absente"),
        },
    };
    let existing = list(state, org, project, "release/folders");
    if find_folder(&existing, &path).is_some() {
        return fail(StatusCode::BAD_REQUEST, "dossier déjà présent");
    }
    let item = json!({"id": path, "path": path, "description": description});
    put_item(state, org, project, "release/folders", item.clone());
    Json(dress_release_folder(org, project, item)).into_response()
}

fn dress_release_folder(org: &str, project: &str, mut item: Value) -> Value {
    let path = text_of(&item, "path").unwrap_or_else(|| "\\".into());
    if let Some(object) = item.as_object_mut() {
        object.entry("description").or_insert(json!(""));
        object.insert("project".into(), json!({"name": project}));
        object.insert(
            "url".into(),
            json!(format!("/{org}/{project}/_apis/release/folders/{}", path.replace('\\', "%5C"))),
        );
    }
    item
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
    if rest.len() == 2 && method == Method::DELETE {
        return fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée");
    }
    let id = rest.get(2).map(String::as_str).unwrap_or("");
    if method == Method::GET {
        return match get_item(state, org, project, key, id) {
            Some(item) => Json(item).into_response(),
            None => fail(StatusCode::NOT_FOUND, "définition introuvable"),
        };
    }
    if method == Method::DELETE {
        if rest.len() != 3 {
            return fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée");
        }
        let Some(found) = get_item(state, org, project, key, id) else {
            return fail(StatusCode::NOT_FOUND, "définition introuvable");
        };
        let stored_id = value_key(&found["id"]);
        state.write(|db| {
            if let Some(stored) = db.project_mut(org, project) {
                if let Some(bucket) = stored.buckets.get_mut(key) {
                    bucket.remove(&stored_id);
                }
            }
        });
        return StatusCode::NO_CONTENT.into_response();
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
    if method == Method::PATCH || method == Method::PUT {
        return update_release(state, org, project, id, body);
    }
    fail(StatusCode::NOT_FOUND, "version introuvable")
}

fn update_release(state: &AppState, org: &str, project: &str, id: &str, body: &[u8]) -> Response {
    let key = "release/releases";
    let Some(previous) = get_item(state, org, project, key, id) else {
        return fail(StatusCode::NOT_FOUND, "version introuvable");
    };
    let Some(payload) = parse_json(body).filter(|value| value.is_object()) else {
        return fail(StatusCode::BAD_REQUEST, "version illisible");
    };
    if payload.get("description").is_some_and(|value| !value.is_string()) {
        return fail(StatusCode::BAD_REQUEST, "description illisible");
    }
    let Some(status_value) = payload.get("status") else {
        return fail(StatusCode::BAD_REQUEST, "statut de version absent");
    };
    let Some(status_raw) = status_value.as_str() else {
        return fail(StatusCode::BAD_REQUEST, "version illisible");
    };
    let trimmed = status_raw.trim();
    if trimmed.is_empty() {
        return fail(StatusCode::BAD_REQUEST, "statut de version absent");
    }
    let Some(canonical) = canonical_release_status(trimmed) else {
        return fail(StatusCode::BAD_REQUEST, "statut de version inconnu");
    };
    let previous_status = text_of(&previous, "status").unwrap_or_else(|| "active".into());
    if canonical == "active" && previous_status.eq_ignore_ascii_case("abandoned") {
        return fail(StatusCode::BAD_REQUEST, "version abandonnée");
    }
    let notify = canonical == "abandoned" && !previous_status.eq_ignore_ascii_case("abandoned");
    let description = payload.get("description").and_then(|value| value.as_str()).map(str::to_string);
    let updated = state.write(|db| {
        let stored = db.project_mut(org, project)?;
        let mut release = stored.get_bucket(key, id)?;
        if let Some(object) = release.as_object_mut() {
            object.insert("status".into(), json!(canonical));
            if let Some(description) = description.as_deref() {
                object.insert("description".into(), json!(description));
            }
        }
        if canonical == "abandoned" {
            cancel_open_environments(&mut release);
        }
        let saved = stored.put_bucket(key, release);
        if canonical == "abandoned" {
            let approvals = stored.list_bucket("release/approvals");
            for mut approval in approvals {
                let same_release = approval
                    .get("release")
                    .and_then(|release| release.get("id"))
                    .map(|value| value_key(value) == id)
                    .unwrap_or(false);
                let pending = text_of(&approval, "status").is_some_and(|status| status.eq_ignore_ascii_case("pending"));
                if same_release && pending {
                    if let Some(object) = approval.as_object_mut() {
                        object.insert("status".into(), json!("canceled"));
                    }
                    stored.put_bucket("release/approvals", approval);
                }
            }
        }
        Some(saved)
    });
    match updated {
        Some(item) => {
            if notify {
                deliver_work_item(state, org, project, "release.abandoned", &item);
            }
            Json(item).into_response()
        }
        None => fail(StatusCode::NOT_FOUND, "version introuvable"),
    }
}

fn canonical_release_status(status: &str) -> Option<&'static str> {
    match status.to_ascii_lowercase().as_str() {
        "active" => Some("active"),
        "abandoned" => Some("abandoned"),
        _ => None,
    }
}

fn cancel_open_environments(release: &mut Value) {
    let Some(environments) = release.get_mut("environments").and_then(|value| value.as_array_mut()) else {
        return;
    };
    for environment in environments {
        let status = text_of(environment, "status").unwrap_or_default();
        if matches!(status.as_str(), "succeeded" | "rejected" | "canceled" | "partiallySucceeded") {
            continue;
        }
        if let Some(object) = environment.as_object_mut() {
            object.insert("status".into(), json!("canceled"));
        }
    }
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

fn user_entitlements(state: &AppState, method: &Method, org: &str, rest: &[String], query: &str, body: &[u8]) -> Response {
    let key = "memberentitlementmanagement/userentitlements";
    if rest.len() == 2 {
        if method == Method::GET {
            let mut rows = org_list(state, org, key);
            rows.retain(|item| entitlement_listed(item, query));
            rows.sort_by(|left, right| value_key(&left["id"]).cmp(&value_key(&right["id"])));
            let value = rows.into_iter().map(|item| dress_entitlement(org, item)).collect();
            return Json(page(value)).into_response();
        }
        if method == Method::POST {
            let mut item = match entitlement_create(body) {
                Ok(item) => item,
                Err(message) => return fail(StatusCode::BAD_REQUEST, message),
            };
            let next = org_list(state, org, key)
                .into_iter()
                .filter_map(|item| text_of(&item, "id"))
                .filter_map(|id| id.strip_prefix("ent-").and_then(|number| number.parse::<i64>().ok()))
                .max()
                .unwrap_or(0)
                + 1;
            if let Some(object) = item.as_object_mut() {
                let id = format!("ent-{next}");
                object.insert("id".into(), json!(id.clone()));
                if let Some(user) = object.get_mut("user").and_then(|value| value.as_object_mut()) {
                    user.insert("originId".into(), json!(id));
                }
                object.entry("projectEntitlements").or_insert(json!([{
                    "projectRef": {"id": org, "name": org},
                    "group": {"groupType": "projectContributor", "displayName": "Contributeurs"}
                }]));
            }
            org_put(state, org, key, item.clone());
            return Json(dress_entitlement(org, item)).into_response();
        }
        return fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée");
    }
    if rest.len() != 3 {
        if method == Method::DELETE {
            return fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée");
        }
        return fail(StatusCode::NOT_FOUND, "droit introuvable");
    }
    let id = rest[2].as_str();
    if method == Method::GET {
        return match org_get(state, org, key, id) {
            Some(item) => Json(dress_entitlement(org, item)).into_response(),
            None => fail(StatusCode::NOT_FOUND, "droit introuvable"),
        };
    }
    if method == Method::DELETE {
        let Some(found) = org_get(state, org, key, id) else {
            return fail(StatusCode::NOT_FOUND, "droit introuvable");
        };
        let stored_id = value_key(&found["id"]);
        let removed = state.write(|db| -> Option<bool> {
            let bucket = db.orgs.get_mut(org)?.buckets.get_mut(key)?;
            let key_name = bucket
                .items
                .iter()
                .find_map(|(stored, item)| (stored == &stored_id || value_key(&item["id"]) == stored_id).then(|| stored.clone()))?;
            Some(bucket.items.remove(&key_name).is_some())
        });
        return if removed == Some(true) {
            StatusCode::NO_CONTENT.into_response()
        } else {
            fail(StatusCode::NOT_FOUND, "droit introuvable")
        };
    }
    fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée")
}

fn entitlement_listed(item: &Value, query: &str) -> bool {
    let Some(project) = query_param(query, "project").map(pct_decode).filter(|value| !value.is_empty()) else {
        return true;
    };
    item.get("projectEntitlements")
        .and_then(|value| value.as_array())
        .is_some_and(|rows| {
            rows.iter().any(|row| {
                let reference = row.get("projectRef");
                let id = reference.and_then(|value| value.get("id")).and_then(|value| value.as_str()).unwrap_or("");
                let name = reference.and_then(|value| value.get("name")).and_then(|value| value.as_str()).unwrap_or("");
                id.eq_ignore_ascii_case(&project) || name.eq_ignore_ascii_case(&project)
            })
        })
}

fn canonical_license(value: &str) -> Option<(&'static str, &'static str)> {
    match value.trim().to_ascii_lowercase().as_str() {
        "express" => Some(("express", "Basic")),
        "stakeholder" => Some(("stakeholder", "Stakeholder")),
        "advanced" => Some(("advanced", "Advanced")),
        "professional" => Some(("professional", "Professional")),
        _ => None,
    }
}

fn canonical_group(value: &str) -> Option<(&'static str, &'static str)> {
    match value.trim().to_ascii_lowercase().as_str() {
        "projectcontributor" => Some(("projectContributor", "Contributeurs")),
        "projectreader" => Some(("projectReader", "Lecteurs")),
        "projectadministrator" => Some(("projectAdministrator", "Administrateurs")),
        "projectstakeholder" => Some(("projectStakeholder", "Parties prenantes")),
        "custom" => Some(("custom", "Personnalisé")),
        _ => None,
    }
}

fn read_project_entitlements(value: &Value) -> Result<Vec<Value>, &'static str> {
    let Some(rows) = value.as_array() else {
        return Err("projet absent");
    };
    let mut entitlements = Vec::new();
    for row in rows {
        let Some(object) = row.as_object() else {
            return Err("projet absent");
        };
        let Some(project_ref) = object.get("projectRef").and_then(|item| item.as_object()) else {
            return Err("projet absent");
        };
        let Some(id) = project_ref
            .get("id")
            .and_then(|item| item.as_str())
            .map(str::trim)
            .filter(|item| !item.is_empty())
        else {
            return Err("projet absent");
        };
        let name = project_ref
            .get("name")
            .and_then(|item| item.as_str())
            .map(str::trim)
            .filter(|item| !item.is_empty())
            .unwrap_or(id);
        let (group_type, group_name) = match object.get("group") {
            None => ("projectContributor", "Contributeurs"),
            Some(group) => {
                let Some(group) = group.as_object() else {
                    return Err("groupe absent");
                };
                let Some(kind) = group
                    .get("groupType")
                    .and_then(|item| item.as_str())
                    .map(str::trim)
                    .filter(|item| !item.is_empty())
                else {
                    return Err("groupe absent");
                };
                let Some(pair) = canonical_group(kind) else {
                    return Err("groupe inconnu");
                };
                pair
            }
        };
        entitlements.push(json!({
            "projectRef": {"id": id, "name": name},
            "group": {"groupType": group_type, "displayName": group_name}
        }));
    }
    Ok(entitlements)
}

fn entitlement_create(body: &[u8]) -> Result<Value, &'static str> {
    let Some(payload) = parse_json(body) else {
        return Err("JSON illisible");
    };
    let Some(object) = payload.as_object() else {
        return Err("droit illisible");
    };
    let Some(user) = object.get("user").and_then(|value| value.as_object()) else {
        return Err("identité absente");
    };
    let Some(principal) = user
        .get("principalName")
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return Err("identité absente");
    };
    let display = match user.get("displayName") {
        None => principal.to_string(),
        Some(value) => {
            let Some(name) = value.as_str().map(str::trim).filter(|value| !value.is_empty()) else {
                return Err("nom absent");
            };
            name.to_string()
        }
    };
    let Some(access) = object.get("accessLevel").and_then(|value| value.as_object()) else {
        return Err("licence absente");
    };
    let Some(license_raw) = access
        .get("accountLicenseType")
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return Err("licence absente");
    };
    let Some((license, license_name)) = canonical_license(license_raw) else {
        return Err("licence inconnue");
    };
    let projects = match object.get("projectEntitlements") {
        None => None,
        Some(value) => Some(read_project_entitlements(value)?),
    };
    let mut item = Map::new();
    item.insert(
        "user".into(),
        json!({
            "displayName": display,
            "principalName": principal,
            "subjectKind": "user",
            "origin": "azurz"
        }),
    );
    item.insert(
        "accessLevel".into(),
        json!({
            "licensingSource": "account",
            "accountLicenseType": license,
            "licenseDisplayName": license_name,
            "status": "active",
            "assignmentSource": "unknown"
        }),
    );
    item.insert("dateCreated".into(), json!("2026-10-29T00:00:00Z"));
    item.insert("lastAccessedDate".into(), json!("2026-10-29T00:00:00Z"));
    if let Some(projects) = projects {
        item.insert("projectEntitlements".into(), json!(projects));
    }
    Ok(Value::Object(item))
}

fn dress_entitlement(org: &str, mut item: Value) -> Value {
    let id = text_of(&item, "id").unwrap_or_default();
    if let Some(object) = item.as_object_mut() {
        let href = format!("/{org}/_apis/memberentitlementmanagement/userentitlements/{id}");
        object.insert("url".into(), json!(href.clone()));
        object.insert("_links".into(), json!({"self": {"href": href}}));
    }
    item
}

fn favorites(state: &AppState, method: &Method, org: &str, rest: &[String], query: &str, body: &[u8]) -> Response {
    let key = "favorite/favorites";
    if rest.len() == 2 {
        if method == Method::GET {
            let mut rows = org_list(state, org, key);
            rows.retain(|item| favorite_listed(item, query));
            rows.sort_by(|left, right| value_key(&left["id"]).cmp(&value_key(&right["id"])));
            let value = rows.into_iter().map(|item| dress_favorite(org, item)).collect();
            return Json(page(value)).into_response();
        }
        if method == Method::POST {
            let mut item = match favorite_create(body) {
                Ok(item) => item,
                Err(message) => return fail(StatusCode::BAD_REQUEST, message),
            };
            let next = org_list(state, org, key)
                .into_iter()
                .filter_map(|item| text_of(&item, "id"))
                .filter_map(|id| id.strip_prefix("fav-").and_then(|number| number.parse::<i64>().ok()))
                .max()
                .unwrap_or(0)
                + 1;
            if let Some(object) = item.as_object_mut() {
                object.insert("id".into(), json!(format!("fav-{next}")));
                object.entry("artifactScope").or_insert(json!({"type": "Project", "id": org, "name": org}));
            }
            org_put(state, org, key, item.clone());
            return Json(dress_favorite(org, item)).into_response();
        }
        return fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée");
    }
    if rest.len() != 3 {
        if method == Method::DELETE {
            return fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée");
        }
        return fail(StatusCode::NOT_FOUND, "favori introuvable");
    }
    let id = rest[2].as_str();
    if method == Method::GET {
        return match org_get(state, org, key, id) {
            Some(item) => Json(dress_favorite(org, item)).into_response(),
            None => fail(StatusCode::NOT_FOUND, "favori introuvable"),
        };
    }
    if method == Method::DELETE {
        let Some(found) = org_get(state, org, key, id) else {
            return fail(StatusCode::NOT_FOUND, "favori introuvable");
        };
        let stored_id = value_key(&found["id"]);
        let removed = state.write(|db| -> Option<bool> {
            let bucket = db.orgs.get_mut(org)?.buckets.get_mut(key)?;
            let key_name = bucket
                .items
                .iter()
                .find_map(|(stored, item)| (stored == &stored_id || value_key(&item["id"]) == stored_id).then(|| stored.clone()))?;
            Some(bucket.items.remove(&key_name).is_some())
        });
        return if removed == Some(true) {
            StatusCode::NO_CONTENT.into_response()
        } else {
            fail(StatusCode::NOT_FOUND, "favori introuvable")
        };
    }
    fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée")
}

fn favorite_listed(item: &Value, query: &str) -> bool {
    if let Some(kind) = query_param(query, "artifactType").map(pct_decode).filter(|value| !value.is_empty()) {
        if !text_of(item, "artifactType").is_some_and(|value| value.eq_ignore_ascii_case(&kind)) {
            return false;
        }
    }
    let scope = item.get("artifactScope");
    if let Some(scope_type) = query_param(query, "artifactScopeType").map(pct_decode).filter(|value| !value.is_empty()) {
        let stored = scope.and_then(|value| value.get("type")).and_then(|value| value.as_str()).unwrap_or("");
        if !stored.eq_ignore_ascii_case(&scope_type) {
            return false;
        }
    }
    if let Some(scope_id) = query_param(query, "artifactScopeId").map(pct_decode).filter(|value| !value.is_empty()) {
        let id = scope.and_then(|value| value.get("id")).and_then(|value| value.as_str()).unwrap_or("");
        let name = scope.and_then(|value| value.get("name")).and_then(|value| value.as_str()).unwrap_or("");
        if !id.eq_ignore_ascii_case(&scope_id) && !name.eq_ignore_ascii_case(&scope_id) {
            return false;
        }
    }
    true
}

fn favorite_create(body: &[u8]) -> Result<Value, &'static str> {
    let Some(payload) = parse_json(body) else {
        return Err("JSON illisible");
    };
    let Some(object) = payload.as_object() else {
        return Err("favori illisible");
    };
    let Some(artifact_id) = object
        .get("artifactId")
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return Err("artefact absent");
    };
    let Some(artifact_type) = object
        .get("artifactType")
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return Err("type absent");
    };
    let artifact_name = match object.get("artifactName") {
        None => artifact_id.to_string(),
        Some(value) => {
            let Some(name) = value.as_str().map(str::trim).filter(|value| !value.is_empty()) else {
                return Err("nom absent");
            };
            name.to_string()
        }
    };
    let scope = match object.get("artifactScope") {
        None => None,
        Some(value) => {
            let Some(scope) = value.as_object() else {
                return Err("portée absente");
            };
            let Some(id) = scope.get("id").and_then(|value| value.as_str()).map(str::trim).filter(|value| !value.is_empty()) else {
                return Err("portée absente");
            };
            let kind = scope
                .get("type")
                .and_then(|value| value.as_str())
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .unwrap_or("Project");
            let name = scope
                .get("name")
                .and_then(|value| value.as_str())
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .unwrap_or(id);
            Some(json!({"type": kind, "id": id, "name": name}))
        }
    };
    let mut item = Map::new();
    item.insert("artifactId".into(), json!(artifact_id));
    item.insert("artifactType".into(), json!(artifact_type));
    item.insert("artifactName".into(), json!(artifact_name));
    item.insert("artifactIsDeleted".into(), json!(false));
    item.insert("creationDate".into(), json!("2026-10-29T00:00:00Z"));
    item.insert("owner".into(), json!({"id": "azurz", "name": "azurz"}));
    if let Some(scope) = scope {
        item.insert("artifactScope".into(), scope);
    }
    Ok(Value::Object(item))
}

fn dress_favorite(org: &str, mut item: Value) -> Value {
    let id = text_of(&item, "id").unwrap_or_default();
    if let Some(object) = item.as_object_mut() {
        object.insert("url".into(), json!(format!("/{org}/_apis/favorite/favorites/{id}")));
        object.insert(
            "_links".into(),
            json!({"self": {"href": format!("/{org}/_apis/favorite/favorites/{id}")}}),
        );
    }
    item
}

fn notification_subscriptions(state: &AppState, method: &Method, org: &str, rest: &[String], body: &[u8]) -> Response {
    let key = "notification/subscriptions";
    if rest.len() == 2 {
        if method == Method::GET {
            let mut rows = org_list(state, org, key);
            rows.sort_by(|left, right| value_key(&left["id"]).cmp(&value_key(&right["id"])));
            let value = rows.into_iter().map(|item| dress_notification(org, item)).collect();
            return Json(page(value)).into_response();
        }
        if method == Method::POST {
            let mut item = match notification_create(body) {
                Ok(item) => item,
                Err(message) => return fail(StatusCode::BAD_REQUEST, message),
            };
            let next = org_list(state, org, key)
                .into_iter()
                .filter_map(|item| text_of(&item, "id"))
                .filter_map(|id| id.strip_prefix("notif-").and_then(|number| number.parse::<i64>().ok()))
                .max()
                .unwrap_or(0)
                + 1;
            if let Some(object) = item.as_object_mut() {
                object.insert("id".into(), json!(format!("notif-{next}")));
                object.entry("scope").or_insert(json!({"type": "none", "id": org}));
            }
            org_put(state, org, key, item.clone());
            return Json(dress_notification(org, item)).into_response();
        }
        return fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée");
    }
    if rest.len() != 3 {
        if method == Method::DELETE {
            return fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée");
        }
        return fail(StatusCode::NOT_FOUND, "abonnement introuvable");
    }
    let id = rest[2].as_str();
    if method == Method::GET {
        return match org_get(state, org, key, id) {
            Some(item) => Json(dress_notification(org, item)).into_response(),
            None => fail(StatusCode::NOT_FOUND, "abonnement introuvable"),
        };
    }
    if method == Method::DELETE {
        let Some(found) = org_get(state, org, key, id) else {
            return fail(StatusCode::NOT_FOUND, "abonnement introuvable");
        };
        let stored_id = value_key(&found["id"]);
        let removed = state.write(|db| -> Option<bool> {
            let bucket = db.orgs.get_mut(org)?.buckets.get_mut(key)?;
            let key_name = bucket
                .items
                .iter()
                .find_map(|(stored, item)| (stored == &stored_id || value_key(&item["id"]) == stored_id).then(|| stored.clone()))?;
            Some(bucket.items.remove(&key_name).is_some())
        });
        return if removed == Some(true) {
            StatusCode::NO_CONTENT.into_response()
        } else {
            fail(StatusCode::NOT_FOUND, "abonnement introuvable")
        };
    }
    fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée")
}

fn notification_create(body: &[u8]) -> Result<Value, &'static str> {
    let Some(payload) = parse_json(body) else {
        return Err("JSON illisible");
    };
    let Some(object) = payload.as_object() else {
        return Err("abonnement illisible");
    };
    let Some(description) = object
        .get("description")
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return Err("description absente");
    };
    let Some(filter) = object.get("filter").and_then(|value| value.as_object()) else {
        return Err("filtre absent");
    };
    let filter_type = filter
        .get("type")
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("Expression");
    let canonical_type = match filter_type.to_ascii_lowercase().as_str() {
        "expression" => "Expression",
        "artifact" => "Artifact",
        _ => return Err("filtre inconnu"),
    };
    let mut stored_filter = Map::new();
    stored_filter.insert("type".into(), json!(canonical_type));
    if canonical_type == "Expression" {
        let Some(event) = filter
            .get("eventType")
            .and_then(|value| value.as_str())
            .map(str::trim)
            .filter(|value| !value.is_empty())
        else {
            return Err("événement absent");
        };
        stored_filter.insert("eventType".into(), json!(event));
        let criteria = match filter.get("criteria") {
            None => json!({"clauses": [], "groups": [], "maxGroupLevel": 0}),
            Some(value) if value.is_object() => value.clone(),
            Some(_) => return Err("filtre illisible"),
        };
        stored_filter.insert("criteria".into(), criteria);
    } else {
        let Some(artifact_type) = filter
            .get("artifactType")
            .and_then(|value| value.as_str())
            .map(str::trim)
            .filter(|value| !value.is_empty())
        else {
            return Err("artefact absent");
        };
        let Some(artifact_id) = filter
            .get("artifactId")
            .and_then(|value| value.as_str())
            .map(str::trim)
            .filter(|value| !value.is_empty())
        else {
            return Err("artefact absent");
        };
        stored_filter.insert("artifactType".into(), json!(artifact_type));
        stored_filter.insert("artifactId".into(), json!(artifact_id));
    }
    let channel = match object.get("channel") {
        None => json!({"type": "EmailHtml", "useCustomAddress": false}),
        Some(value) => {
            let Some(channel) = value.as_object() else {
                return Err("canal illisible");
            };
            let Some(kind) = channel
                .get("type")
                .and_then(|value| value.as_str())
                .map(str::trim)
                .filter(|value| !value.is_empty())
            else {
                return Err("canal illisible");
            };
            let mut stored = Map::new();
            stored.insert("type".into(), json!(kind));
            if let Some(address) = channel
                .get("address")
                .and_then(|value| value.as_str())
                .map(str::trim)
                .filter(|value| !value.is_empty())
            {
                stored.insert("address".into(), json!(address));
            }
            let custom = channel.get("useCustomAddress").and_then(|value| value.as_bool()).unwrap_or(false);
            stored.insert("useCustomAddress".into(), json!(custom));
            Value::Object(stored)
        }
    };
    let subscriber = match object.get("subscriber") {
        None => json!({"id": "azurz", "displayName": "azurz", "uniqueName": "azurz"}),
        Some(value) => {
            let Some(person) = value.as_object() else {
                return Err("abonné absent");
            };
            let Some(id) = person.get("id").and_then(|value| value.as_str()).map(str::trim).filter(|value| !value.is_empty()) else {
                return Err("abonné absent");
            };
            let display = person
                .get("displayName")
                .and_then(|value| value.as_str())
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .unwrap_or(id);
            let unique = person
                .get("uniqueName")
                .and_then(|value| value.as_str())
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .unwrap_or(id);
            json!({"id": id, "displayName": display, "uniqueName": unique})
        }
    };
    let scope = match object.get("scope") {
        None => None,
        Some(value) => {
            let Some(scope) = value.as_object() else {
                return Err("périmètre absent");
            };
            let Some(id) = scope.get("id").and_then(|value| value.as_str()).map(str::trim).filter(|value| !value.is_empty()) else {
                return Err("périmètre absent");
            };
            let kind = scope
                .get("type")
                .and_then(|value| value.as_str())
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .unwrap_or("project");
            let mut stored = Map::new();
            stored.insert("type".into(), json!(kind));
            stored.insert("id".into(), json!(id));
            if let Some(name) = scope.get("name").and_then(|value| value.as_str()).map(str::trim).filter(|value| !value.is_empty()) {
                stored.insert("name".into(), json!(name));
            }
            Some(Value::Object(stored))
        }
    };
    let mut item = Map::new();
    item.insert("description".into(), json!(description));
    item.insert("subscriber".into(), subscriber);
    item.insert("status".into(), json!("enabled"));
    item.insert("flags".into(), json!("none"));
    item.insert("filter".into(), Value::Object(stored_filter));
    item.insert("channel".into(), channel);
    item.insert("permissions".into(), json!("view, edit, delete"));
    if let Some(scope) = scope {
        item.insert("scope".into(), scope);
    }
    Ok(Value::Object(item))
}

fn dress_notification(org: &str, mut item: Value) -> Value {
    let id = text_of(&item, "id").unwrap_or_default();
    if let Some(object) = item.as_object_mut() {
        object.insert(
            "url".into(),
            json!(format!("/{org}/_apis/notification/subscriptions/{id}")),
        );
        object.insert(
            "_links".into(),
            json!({"self": {"href": format!("/{org}/_apis/notification/subscriptions/{id}")}}),
        );
    }
    item
}

fn subscriptions(state: &AppState, method: &Method, org: &str, rest: &[String], body: &[u8]) -> Response {
    let key = "hooks/subscriptions";
    if rest.len() == 2 && method == Method::GET {
        return Json(page(org_list(state, org, key))).into_response();
    }
    if rest.len() == 2 && method == Method::POST {
        let payload = parse_json(body).unwrap_or(json!({}));
        let next = org_list(state, org, key)
            .into_iter()
            .filter_map(|item| text_of(&item, "id"))
            .filter_map(|id| id.strip_prefix("sub-").and_then(|number| number.parse::<i64>().ok()))
            .max()
            .unwrap_or(0)
            + 1;
        let mut item = payload;
        if let Some(object) = item.as_object_mut() {
            let explicit = object
                .get("name")
                .and_then(|value| value.as_str())
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string);
            let event = object.get("eventType").and_then(|value| value.as_str()).unwrap_or("").trim().to_string();
            let name = explicit.unwrap_or(if event.is_empty() { format!("sub-{next}") } else { event });
            object.insert("id".into(), json!(format!("sub-{next}")));
            object.insert("name".into(), json!(name));
            object.entry("status").or_insert(json!("enabled"));
            object.entry("publisherId").or_insert(json!("tfs"));
            object.entry("url").or_insert(json!(format!("/{org}/_apis/hooks/subscriptions/sub-{next}")));
        }
        org_put(state, org, key, item.clone());
        return Json(item).into_response();
    }
    if method == Method::DELETE && rest.len() != 3 {
        return fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée");
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
        let Some(found) = org_get(state, org, key, &id) else {
            return fail(StatusCode::NOT_FOUND, "abonnement introuvable");
        };
        let stored_id = value_key(&found["id"]);
        state.write(|db| {
            if let Some(organization) = db.orgs.get_mut(org) {
                if let Some(bucket) = organization.buckets.get_mut(key) {
                    bucket.remove(&stored_id);
                }
            }
        });
        return StatusCode::NO_CONTENT.into_response();
    }
    if (method == Method::PATCH || method == Method::PUT) && rest.len() == 3 {
        return update_subscription_status(state, org, key, &id, body);
    }
    fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée")
}

fn update_subscription_status(state: &AppState, org: &str, key: &str, id: &str, body: &[u8]) -> Response {
    if org_get(state, org, key, id).is_none() {
        return fail(StatusCode::NOT_FOUND, "abonnement introuvable");
    }
    let Some(payload) = parse_json(body).filter(|value| value.is_object()) else {
        return fail(StatusCode::BAD_REQUEST, "abonnement illisible");
    };
    let Some(status_value) = payload.get("status") else {
        return fail(StatusCode::BAD_REQUEST, "statut absent");
    };
    let Some(status_raw) = status_value.as_str() else {
        return fail(StatusCode::BAD_REQUEST, "statut illisible");
    };
    let trimmed = status_raw.trim();
    if trimmed.is_empty() {
        return fail(StatusCode::BAD_REQUEST, "statut absent");
    }
    let Some(canonical) = canonical_subscription_status(trimmed) else {
        return fail(StatusCode::BAD_REQUEST, "statut inconnu");
    };
    let updated = state.write(|db| {
        let bucket = db.orgs.get_mut(org)?.buckets.get_mut(key)?;
        let mut item = bucket.get(id)?;
        if let Some(object) = item.as_object_mut() {
            object.insert("status".into(), json!(canonical));
        }
        Some(bucket.put(item))
    });
    match updated {
        Some(item) => Json(item).into_response(),
        None => fail(StatusCode::NOT_FOUND, "abonnement introuvable"),
    }
}

fn canonical_subscription_status(status: &str) -> Option<&'static str> {
    match status.to_ascii_lowercase().as_str() {
        "enabled" => Some("enabled"),
        "disabled" => Some("disabled"),
        _ => None,
    }
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

fn graph_groups(state: &AppState, method: &Method, org: &str, rest: &[String], query: &str, body: &[u8]) -> Response {
    if rest.get(3).is_some_and(|part| part.eq_ignore_ascii_case("memberships")) {
        return group_memberships(state, method, org, &rest[2], &rest[4..], body);
    }
    let key = "graph/groups";
    if rest.len() == 2 {
        if method == Method::GET {
            let mut rows = org_list(state, org, key);
            rows.retain(|item| group_listed(item, query));
            rows.sort_by(|left, right| value_key(&left["id"]).cmp(&value_key(&right["id"])));
            let value = rows.into_iter().map(|item| dress_graph_group(org, item)).collect();
            return Json(page(value)).into_response();
        }
        if method == Method::POST {
            let (display, description) = match group_create(body) {
                Ok(pair) => pair,
                Err(message) => return fail(StatusCode::BAD_REQUEST, message),
            };
            let scope = query_param(query, "scopeDescriptor")
                .map(pct_decode)
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
                .unwrap_or_else(|| org.to_string());
            let existing = org_list(state, org, key);
            if existing.iter().any(|item| {
                text_of(item, "scopeDescriptor").is_some_and(|stored| stored.eq_ignore_ascii_case(&scope))
                    && text_of(item, "displayName").is_some_and(|name| name.eq_ignore_ascii_case(&display))
            }) {
                return fail(StatusCode::BAD_REQUEST, "groupe existant");
            }
            let next = existing
                .iter()
                .filter_map(|item| text_of(item, "id"))
                .filter_map(|id| id.strip_prefix("grp-").and_then(|number| number.parse::<i64>().ok()))
                .max()
                .unwrap_or(0)
                + 1;
            let id = format!("grp-{next}");
            let item = json!({
                "id": id,
                "descriptor": id,
                "displayName": display,
                "description": description,
                "subjectKind": "group",
                "origin": "vsts",
                "originId": id,
                "principalName": format!("[{scope}]\\{display}"),
                "domain": format!("vstfs:///Classification/TeamProject/{scope}"),
                "scopeDescriptor": scope,
                "mailAddress": Value::Null,
            });
            org_put(state, org, key, item.clone());
            return Json(dress_graph_group(org, item)).into_response();
        }
        return fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée");
    }
    if rest.len() != 3 {
        if method == Method::DELETE {
            return fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée");
        }
        return fail(StatusCode::NOT_FOUND, "groupe introuvable");
    }
    let id = rest[2].as_str();
    if method == Method::GET {
        return match org_get(state, org, key, id) {
            Some(item) => Json(dress_graph_group(org, item)).into_response(),
            None => fail(StatusCode::NOT_FOUND, "groupe introuvable"),
        };
    }
    if method == Method::DELETE {
        let Some(found) = org_get(state, org, key, id) else {
            return fail(StatusCode::NOT_FOUND, "groupe introuvable");
        };
        let stored_id = value_key(&found["id"]);
        let removed = state.write(|db| -> Option<bool> {
            let bucket = db.orgs.get_mut(org)?.buckets.get_mut(key)?;
            let key_name = bucket
                .items
                .iter()
                .find_map(|(stored, item)| (stored == &stored_id || value_key(&item["id"]) == stored_id).then(|| stored.clone()))?;
            Some(bucket.items.remove(&key_name).is_some())
        });
        return if removed == Some(true) {
            StatusCode::NO_CONTENT.into_response()
        } else {
            fail(StatusCode::NOT_FOUND, "groupe introuvable")
        };
    }
    fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée")
}

fn group_listed(item: &Value, query: &str) -> bool {
    let Some(scope) = query_param(query, "scopeDescriptor")
        .map(pct_decode)
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
    else {
        return true;
    };
    text_of(item, "scopeDescriptor").is_some_and(|stored| stored.eq_ignore_ascii_case(&scope))
}

fn group_create(body: &[u8]) -> Result<(String, String), &'static str> {
    let Some(payload) = parse_json(body) else {
        return Err("JSON illisible");
    };
    let Some(object) = payload.as_object() else {
        return Err("groupe illisible");
    };
    let Some(display) = object
        .get("displayName")
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
    else {
        return Err("nom absent");
    };
    let description = match object.get("description") {
        None => String::new(),
        Some(value) => {
            let Some(text) = value.as_str() else {
                return Err("description absente");
            };
            text.trim().to_string()
        }
    };
    Ok((display, description))
}

fn dress_graph_group(org: &str, mut item: Value) -> Value {
    let id = text_of(&item, "id").unwrap_or_default();
    if let Some(object) = item.as_object_mut() {
        let href = format!("/{org}/_apis/graph/groups/{id}");
        object.insert("url".into(), json!(href.clone()));
        object.insert(
            "_links".into(),
            json!({
                "self": {"href": href},
                "memberships": {"href": format!("/{org}/_apis/graph/groups/{id}/memberships")}
            }),
        );
    }
    item
}

const MEMBERSHIPS: &str = "graph/memberships";

pub fn graph_memberships(state: &AppState, method: &Method, org: &str, rest: &[String], query: &str, body: &[u8]) -> Response {
    let Some(tail) = memberships_tail(rest) else {
        return if method == Method::DELETE {
            fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée")
        } else {
            fail(StatusCode::NOT_FOUND, "appartenance introuvable")
        };
    };
    if tail.len() > 2 {
        return if method == Method::DELETE {
            fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée")
        } else {
            fail(StatusCode::NOT_FOUND, "appartenance introuvable")
        };
    }
    let scope = membership_scope(query);
    if tail.is_empty() {
        if method == Method::GET {
            return Json(page(membership_rows(state, org, scope.as_deref()))).into_response();
        }
        if method == Method::POST {
            let project = scope.unwrap_or_else(|| state.names().1);
            return match membership_create(state, org, &project, body, None, None) {
                Ok(item) => Json(item).into_response(),
                Err(message) => fail(StatusCode::BAD_REQUEST, message),
            };
        }
        return fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée");
    }
    if tail.len() == 2 {
        let member = pct_decode(&tail[0]);
        let container = pct_decode(&tail[1]);
        if method == Method::POST {
            let project = scope.unwrap_or_else(|| state.names().1);
            return match membership_create(state, org, &project, body, Some(member.trim()), Some(container.trim())) {
                Ok(item) => Json(item).into_response(),
                Err(message) => fail(StatusCode::BAD_REQUEST, message),
            };
        }
        let Some(found) = locate_membership_pair(state, org, scope.as_deref(), member.trim(), container.trim()) else {
            return fail(StatusCode::NOT_FOUND, "appartenance introuvable");
        };
        return membership_item(state, method, org, found);
    }
    let id = pct_decode(&tail[0]);
    let Some(found) = locate_membership(state, org, scope.as_deref(), id.trim()) else {
        return fail(StatusCode::NOT_FOUND, "appartenance introuvable");
    };
    membership_item(state, method, org, found)
}

fn group_memberships(state: &AppState, method: &Method, org: &str, group_id: &str, tail: &[String], body: &[u8]) -> Response {
    let Some(group) = org_get(state, org, "graph/groups", group_id) else {
        return fail(StatusCode::NOT_FOUND, "groupe introuvable");
    };
    let scope = text_of(&group, "scopeDescriptor").unwrap_or_default();
    let display = text_of(&group, "displayName").unwrap_or_default();
    if tail.len() > 1 {
        return if method == Method::DELETE {
            fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée")
        } else {
            fail(StatusCode::NOT_FOUND, "appartenance introuvable")
        };
    }
    if tail.is_empty() {
        if method == Method::GET {
            let rows = membership_rows(state, org, Some(scope.as_str()))
                .into_iter()
                .filter(|item| membership_in_group(item, &display, group_id))
                .collect();
            return Json(page(rows)).into_response();
        }
        if method == Method::POST {
            return match membership_create(state, org, &scope, body, None, Some(display.as_str())) {
                Ok(item) => Json(item).into_response(),
                Err(message) => fail(StatusCode::BAD_REQUEST, message),
            };
        }
        return fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée");
    }
    let id = pct_decode(&tail[0]);
    let Some(found) = locate_membership(state, org, Some(scope.as_str()), id.trim()) else {
        return fail(StatusCode::NOT_FOUND, "appartenance introuvable");
    };
    if !membership_in_group(&found, &display, group_id) {
        return fail(StatusCode::NOT_FOUND, "appartenance introuvable");
    }
    membership_item(state, method, org, found)
}

fn membership_item(state: &AppState, method: &Method, org: &str, found: Value) -> Response {
    if method == Method::GET {
        return Json(dress_membership(org, found)).into_response();
    }
    if method == Method::DELETE {
        let stored_id = value_key(&found["id"]);
        let removed = state.write(|db| -> bool {
            db.orgs
                .get_mut(org)
                .and_then(|organization| organization.buckets.get_mut(MEMBERSHIPS))
                .is_some_and(|bucket| bucket.remove(&stored_id))
        });
        return if removed {
            StatusCode::NO_CONTENT.into_response()
        } else {
            fail(StatusCode::NOT_FOUND, "appartenance introuvable")
        };
    }
    fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée")
}

fn memberships_tail(rest: &[String]) -> Option<&[String]> {
    let index = rest.iter().position(|part| part.eq_ignore_ascii_case("memberships"))?;
    Some(&rest[index + 1..])
}

fn membership_scope(query: &str) -> Option<String> {
    query_param(query, "project")
        .or_else(|| query_param(query, "scopeDescriptor"))
        .map(pct_decode)
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn membership_rows(state: &AppState, org: &str, project: Option<&str>) -> Vec<Value> {
    let mut rows = org_list(state, org, MEMBERSHIPS);
    if let Some(project) = project {
        rows.retain(|item| text_of(item, "project").is_some_and(|stored| stored.eq_ignore_ascii_case(project)));
    }
    rows.sort_by(|left, right| value_key(&left["id"]).cmp(&value_key(&right["id"])));
    rows.into_iter().map(|item| dress_membership(org, item)).collect()
}

fn locate_membership(state: &AppState, org: &str, project: Option<&str>, id: &str) -> Option<Value> {
    org_list(state, org, MEMBERSHIPS).into_iter().find(|item| {
        project.is_none_or(|project| text_of(item, "project").is_some_and(|stored| stored.eq_ignore_ascii_case(project)))
            && (value_key(&item["id"]) == id
                || text_of(item, "memberDescriptor").is_some_and(|value| value.eq_ignore_ascii_case(id))
                || text_of(item, "name").is_some_and(|value| value.eq_ignore_ascii_case(id)))
    })
}

fn locate_membership_pair(state: &AppState, org: &str, project: Option<&str>, member: &str, container: &str) -> Option<Value> {
    org_list(state, org, MEMBERSHIPS).into_iter().find(|item| {
        project.is_none_or(|project| text_of(item, "project").is_some_and(|stored| stored.eq_ignore_ascii_case(project)))
            && membership_side(item, "member", "memberDescriptor", member)
            && membership_side(item, "container", "containerDescriptor", container)
    })
}

fn membership_side(item: &Value, name: &str, descriptor: &str, wanted: &str) -> bool {
    text_of(item, name).is_some_and(|value| value.eq_ignore_ascii_case(wanted))
        || text_of(item, descriptor).is_some_and(|value| value.eq_ignore_ascii_case(wanted))
}

fn membership_in_group(item: &Value, display: &str, group_id: &str) -> bool {
    text_of(item, "container").is_some_and(|value| value.eq_ignore_ascii_case(display) || value == group_id)
        || text_of(item, "containerDescriptor").is_some_and(|value| value.eq_ignore_ascii_case(group_id))
}

fn membership_create(
    state: &AppState,
    org: &str,
    project: &str,
    body: &[u8],
    path_member: Option<&str>,
    path_container: Option<&str>,
) -> Result<Value, &'static str> {
    let (member, container) = membership_names(body, path_member, path_container)?;
    let member_descriptor = membership_descriptor("usr", &member, "membre absent")?;
    let container_descriptor = membership_descriptor("grp", &container, "conteneur absent")?;
    let id = format!("mem-{}", next_membership_id(state, org));
    let item = json!({
        "id": id,
        "name": format!("{member}|{container}"),
        "member": member,
        "container": container,
        "memberDescriptor": member_descriptor,
        "containerDescriptor": container_descriptor,
        "project": project
    });
    state.write(|db| -> Result<(), &'static str> {
        db.ensure_org(org);
        let Some(organization) = db.orgs.get_mut(org) else {
            return Err("appartenance introuvable");
        };
        let existing: Vec<Value> = organization
            .buckets
            .get(MEMBERSHIPS)
            .map(|bucket| bucket.items.values().cloned().collect())
            .unwrap_or_default();
        if existing.iter().any(|stored| {
            text_of(stored, "project").is_some_and(|value| value.eq_ignore_ascii_case(project))
                && text_of(stored, "member").is_some_and(|value| value.eq_ignore_ascii_case(&member))
                && text_of(stored, "container").is_some_and(|value| value.eq_ignore_ascii_case(&container))
        }) {
            return Err("appartenance existante");
        }
        let bucket = organization.buckets.entry(MEMBERSHIPS.to_string()).or_insert_with(Bucket::empty);
        bucket.put(item.clone());
        Ok(())
    })?;
    Ok(dress_membership(org, item))
}

fn membership_names(body: &[u8], path_member: Option<&str>, path_container: Option<&str>) -> Result<(String, String), &'static str> {
    if let (Some(member), Some(container)) = (path_member, path_container) {
        if !body.is_empty() {
            let Some(payload) = parse_json(body) else {
                return Err("JSON illisible");
            };
            if payload.as_object().is_none_or(|object| !object.is_empty()) && !payload.is_null() {
                return Err("appartenance illisible");
            }
        }
        let member = member.trim();
        let container = container.trim();
        if member.is_empty() {
            return Err("membre absent");
        }
        if container.is_empty() {
            return Err("conteneur absent");
        }
        return Ok((member.to_string(), container.to_string()));
    }
    let Some(payload) = parse_json(body) else {
        return Err("JSON illisible");
    };
    let Some(object) = payload.as_object() else {
        return Err("appartenance illisible");
    };
    let Some(member) = object
        .get("member")
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
    else {
        return Err("membre absent");
    };
    let container = match path_container {
        Some(container) if object.get("container").is_none() => container.trim().to_string(),
        _ => match object.get("container") {
            Some(Value::String(value)) => {
                let value = value.trim();
                if value.is_empty() {
                    return Err("conteneur absent");
                }
                if let Some(expected) = path_container {
                    if !value.eq_ignore_ascii_case(expected.trim()) {
                        return Err("conteneur absent");
                    }
                }
                value.to_string()
            }
            Some(_) => return Err("conteneur absent"),
            None => return Err("conteneur absent"),
        },
    };
    if container.is_empty() {
        return Err("conteneur absent");
    }
    Ok((member, container))
}

fn membership_descriptor(prefix: &str, value: &str, missing: &'static str) -> Result<String, &'static str> {
    let tail: String = value.chars().filter(|ch| ch.is_ascii_alphanumeric()).collect();
    if tail.is_empty() {
        return Err(missing);
    }
    Ok(format!("{prefix}-{tail}"))
}

fn next_membership_id(state: &AppState, org: &str) -> i64 {
    org_list(state, org, MEMBERSHIPS)
        .iter()
        .filter_map(|item| value_key(&item["id"]).strip_prefix("mem-")?.parse::<i64>().ok())
        .max()
        .unwrap_or(0)
        + 1
}

fn dress_membership(org: &str, mut item: Value) -> Value {
    let id = value_key(&item["id"]);
    if let Some(object) = item.as_object_mut() {
        object.insert("url".into(), json!(format!("/{org}/_apis/graph/memberships/{id}")));
    }
    item
}

const AUDIT_STREAMS: &str = "audit/streams";

pub fn audit_streams(
    state: &AppState,
    method: &Method,
    org: &str,
    project: Option<&str>,
    rest: &[String],
    body: &[u8],
) -> Response {
    let Some(tail) = audit_tail(rest) else {
        return if method == Method::DELETE {
            fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée")
        } else {
            fail(StatusCode::NOT_FOUND, "flux d'audit introuvable")
        };
    };
    if tail.len() > 1 {
        return if method == Method::DELETE {
            fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée")
        } else {
            fail(StatusCode::NOT_FOUND, "flux d'audit introuvable")
        };
    }
    if tail.is_empty() {
        if method == Method::GET {
            let value = match project {
                Some(project) => project_streams(state, org, project),
                None => org_streams(state, org),
            };
            return Json(page(value)).into_response();
        }
        if method == Method::POST {
            let project = project.map(str::to_string).unwrap_or_else(|| state.names().1);
            return match stream_create(state, org, &project, body) {
                Ok(item) => Json(item).into_response(),
                Err(message) => fail(StatusCode::BAD_REQUEST, message),
            };
        }
        return fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée");
    }
    let id = pct_decode(&tail[0]);
    if method == Method::GET {
        return match locate_stream(state, org, project, id.trim()) {
            Some((owner, found)) => Json(dress_stream(org, &owner, found)).into_response(),
            None => fail(StatusCode::NOT_FOUND, "flux d'audit introuvable"),
        };
    }
    if method == Method::DELETE {
        let Some((owner, found)) = locate_stream(state, org, project, id.trim()) else {
            return fail(StatusCode::NOT_FOUND, "flux d'audit introuvable");
        };
        let stored_id = value_key(&found["id"]);
        let removed = state.write(|db| -> bool {
            db.project_mut(org, &owner)
                .and_then(|stored| stored.buckets.get_mut(AUDIT_STREAMS))
                .is_some_and(|bucket| bucket.remove(&stored_id))
        });
        return if removed {
            StatusCode::NO_CONTENT.into_response()
        } else {
            fail(StatusCode::NOT_FOUND, "flux d'audit introuvable")
        };
    }
    fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée")
}

fn audit_tail(rest: &[String]) -> Option<&[String]> {
    let index = rest.iter().position(|part| part.eq_ignore_ascii_case("streams"))?;
    Some(&rest[index + 1..])
}

fn project_streams(state: &AppState, org: &str, project: &str) -> Vec<Value> {
    let mut rows = list_streams(state, org, project);
    rows.sort_by(|left, right| value_key(&left["id"]).cmp(&value_key(&right["id"])));
    rows.into_iter().map(|item| dress_stream(org, project, item)).collect()
}

fn org_streams(state: &AppState, org: &str) -> Vec<Value> {
    let mut rows = all_streams(state, org);
    rows.sort_by(|left, right| value_key(&left.1["id"]).cmp(&value_key(&right.1["id"])));
    rows.into_iter().map(|(project, item)| dress_stream(org, &project, item)).collect()
}

fn list_streams(state: &AppState, org: &str, project: &str) -> Vec<Value> {
    state.read(|db| db.project(org, project).map(|stored| stored.list_bucket(AUDIT_STREAMS)).unwrap_or_default())
}

fn all_streams(state: &AppState, org: &str) -> Vec<(String, Value)> {
    state.read(|db| {
        let Some(organization) = db.orgs.get(org) else {
            return Vec::new();
        };
        let mut rows = Vec::new();
        for (name, project) in &organization.projects {
            if let Some(bucket) = project.buckets.get(AUDIT_STREAMS) {
                for item in bucket.items.values() {
                    rows.push((name.clone(), item.clone()));
                }
            }
        }
        rows
    })
}

fn locate_stream(state: &AppState, org: &str, project: Option<&str>, id: &str) -> Option<(String, Value)> {
    state.read(|db| {
        let organization = db.orgs.get(org)?;
        for (name, stored) in &organization.projects {
            if let Some(project) = project {
                if !name.eq_ignore_ascii_case(project) && stored.id != project {
                    continue;
                }
            }
            if let Some(item) = stored.buckets.get(AUDIT_STREAMS).and_then(|bucket| {
                bucket.items.values().find(|item| {
                    value_key(&item["id"]) == id
                        || item.get("displayName").and_then(|value| value.as_str()).is_some_and(|name| name.eq_ignore_ascii_case(id))
                })
            }) {
                return Some((name.clone(), item.clone()));
            }
        }
        None
    })
}

fn next_stream_id(state: &AppState, org: &str) -> i64 {
    all_streams(state, org)
        .iter()
        .filter_map(|(_, item)| value_key(&item["id"]).strip_prefix("stream-")?.parse::<i64>().ok())
        .max()
        .unwrap_or(0)
        + 1
}

fn stream_create(state: &AppState, org: &str, project: &str, body: &[u8]) -> Result<Value, &'static str> {
    let (name, consumer, status, days) = stream_payload(body)?;
    let id = format!("stream-{}", next_stream_id(state, org));
    let item = json!({
        "id": id,
        "displayName": name,
        "consumerType": consumer,
        "status": status,
        "daysToBackfill": days
    });
    state.write(|db| -> Result<(), &'static str> {
        db.ensure_project(org, project);
        let Some(stored) = db.project_mut(org, project) else {
            return Err("flux d'audit introuvable");
        };
        let existing: Vec<Value> = stored
            .buckets
            .get(AUDIT_STREAMS)
            .map(|bucket| bucket.items.values().cloned().collect())
            .unwrap_or_default();
        if existing.iter().any(|stored| {
            stored.get("displayName").and_then(|value| value.as_str()).is_some_and(|current| current.eq_ignore_ascii_case(&name))
        }) {
            return Err("flux d'audit existant");
        }
        let bucket = stored.buckets.entry(AUDIT_STREAMS.to_string()).or_insert_with(Bucket::empty);
        bucket.put(item.clone());
        Ok(())
    })?;
    Ok(dress_stream(org, project, item))
}

fn stream_payload(body: &[u8]) -> Result<(String, String, String, i64), &'static str> {
    let Some(payload) = parse_json(body) else {
        return Err("JSON illisible");
    };
    let Some(object) = payload.as_object() else {
        return Err("flux d'audit illisible");
    };
    let Some(name) = object
        .get("displayName")
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
    else {
        return Err("nom absent");
    };
    let consumer = match object.get("consumerType") {
        None => "AzureMonitorLogs".to_string(),
        Some(Value::String(value)) if matches!(value.as_str(), "AzureMonitorLogs" | "AzureEventHub" | "Splunk") => value.clone(),
        Some(_) => return Err("consommateur absent"),
    };
    let status = match object.get("status") {
        None => "enabled".to_string(),
        Some(Value::String(value)) if matches!(value.as_str(), "enabled" | "disabled" | "backfilling") => value.clone(),
        Some(_) => return Err("état absent"),
    };
    let days = match object.get("daysToBackfill") {
        None => 0,
        Some(Value::Number(value)) => {
            let Some(days) = value.as_i64() else {
                return Err("historique absent");
            };
            if !(0..=28).contains(&days) {
                return Err("historique absent");
            }
            days
        }
        Some(_) => return Err("historique absent"),
    };
    Ok((name, consumer, status, days))
}

fn dress_stream(org: &str, project: &str, mut item: Value) -> Value {
    let id = value_key(&item["id"]);
    if let Some(object) = item.as_object_mut() {
        object.insert("project".into(), json!(project));
        object.insert("url".into(), json!(format!("/{org}/{project}/_apis/audit/streams/{id}")));
    }
    item
}

const INSTALLED_EXTENSIONS: &str = "extensionmanagement/installed";

pub fn installed_extensions(
    state: &AppState,
    method: &Method,
    org: &str,
    project: Option<&str>,
    rest: &[String],
    body: &[u8],
) -> Response {
    let Some(tail) = extension_tail(rest) else {
        return if method == Method::DELETE {
            fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée")
        } else {
            fail(StatusCode::NOT_FOUND, "extension introuvable")
        };
    };
    if tail.len() > 2 {
        return if method == Method::DELETE {
            fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée")
        } else {
            fail(StatusCode::NOT_FOUND, "extension introuvable")
        };
    }
    if tail.is_empty() {
        if method == Method::GET {
            let value = match project {
                Some(project) => project_extensions(state, org, project),
                None => org_extensions(state, org),
            };
            return Json(page(value)).into_response();
        }
        if method == Method::POST {
            let project = project.map(str::to_string).unwrap_or_else(|| state.names().1);
            return match extension_create(state, org, &project, body) {
                Ok(item) => Json(item).into_response(),
                Err(message) => fail(StatusCode::BAD_REQUEST, message),
            };
        }
        return fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée");
    }
    let key: Vec<String> = tail.iter().map(|part| pct_decode(part)).collect();
    if method == Method::GET {
        return match locate_extension(state, org, project, &key) {
            Some((owner, found)) => Json(dress_extension(org, &owner, found)).into_response(),
            None => fail(StatusCode::NOT_FOUND, "extension introuvable"),
        };
    }
    if method == Method::DELETE {
        let Some((owner, found)) = locate_extension(state, org, project, &key) else {
            return fail(StatusCode::NOT_FOUND, "extension introuvable");
        };
        let stored_id = value_key(&found["id"]);
        let removed = state.write(|db| -> bool {
            db.project_mut(org, &owner)
                .and_then(|stored| stored.buckets.get_mut(INSTALLED_EXTENSIONS))
                .is_some_and(|bucket| bucket.remove(&stored_id))
        });
        return if removed {
            StatusCode::NO_CONTENT.into_response()
        } else {
            fail(StatusCode::NOT_FOUND, "extension introuvable")
        };
    }
    fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée")
}

fn extension_tail(rest: &[String]) -> Option<&[String]> {
    let index = rest.iter().position(|part| part.eq_ignore_ascii_case("installedextensions"))?;
    Some(&rest[index + 1..])
}

fn project_extensions(state: &AppState, org: &str, project: &str) -> Vec<Value> {
    let mut rows = list_extensions(state, org, project);
    rows.sort_by(|left, right| value_key(&left["id"]).cmp(&value_key(&right["id"])));
    rows.into_iter().map(|item| dress_extension(org, project, item)).collect()
}

fn org_extensions(state: &AppState, org: &str) -> Vec<Value> {
    let mut rows = all_extensions(state, org);
    rows.sort_by(|left, right| value_key(&left.1["id"]).cmp(&value_key(&right.1["id"])));
    rows.into_iter().map(|(project, item)| dress_extension(org, &project, item)).collect()
}

fn list_extensions(state: &AppState, org: &str, project: &str) -> Vec<Value> {
    state.read(|db| {
        db.project(org, project)
            .map(|stored| stored.list_bucket(INSTALLED_EXTENSIONS))
            .unwrap_or_default()
    })
}

fn all_extensions(state: &AppState, org: &str) -> Vec<(String, Value)> {
    state.read(|db| {
        let Some(organization) = db.orgs.get(org) else {
            return Vec::new();
        };
        let mut rows = Vec::new();
        for (name, project) in &organization.projects {
            if let Some(bucket) = project.buckets.get(INSTALLED_EXTENSIONS) {
                for item in bucket.items.values() {
                    rows.push((name.clone(), item.clone()));
                }
            }
        }
        rows
    })
}

fn locate_extension(state: &AppState, org: &str, project: Option<&str>, key: &[String]) -> Option<(String, Value)> {
    let key: Vec<String> = key.iter().map(|part| part.trim().to_string()).collect();
    state.read(|db| {
        let organization = db.orgs.get(org)?;
        for (name, stored) in &organization.projects {
            if let Some(project) = project {
                if !name.eq_ignore_ascii_case(project) && stored.id != project {
                    continue;
                }
            }
            if let Some(item) = stored.buckets.get(INSTALLED_EXTENSIONS).and_then(|bucket| {
                bucket.items.values().find(|item| extension_matches(item, &key))
            }) {
                return Some((name.clone(), item.clone()));
            }
        }
        None
    })
}

fn extension_matches(item: &Value, key: &[String]) -> bool {
    let id = value_key(&item["id"]);
    let extension_id = item.get("extensionId").and_then(|value| value.as_str()).unwrap_or("");
    let extension_name = item.get("extensionName").and_then(|value| value.as_str()).unwrap_or("");
    let publisher = item.get("publisherId").and_then(|value| value.as_str()).unwrap_or("");
    match key {
        [one] => id == one.as_str() || extension_id.eq_ignore_ascii_case(one) || extension_name.eq_ignore_ascii_case(one),
        [publisher_key, extension_key] => {
            publisher.eq_ignore_ascii_case(publisher_key) && extension_id.eq_ignore_ascii_case(extension_key)
        }
        _ => false,
    }
}

fn next_extension_id(state: &AppState, org: &str) -> i64 {
    all_extensions(state, org)
        .iter()
        .filter_map(|(_, item)| value_key(&item["id"]).strip_prefix("ext-")?.parse::<i64>().ok())
        .max()
        .unwrap_or(0)
        + 1
}

fn extension_create(state: &AppState, org: &str, project: &str, body: &[u8]) -> Result<Value, &'static str> {
    let (name, extension_id, publisher, publisher_name, version, flags) = extension_payload(body)?;
    let id = format!("ext-{}", next_extension_id(state, org));
    let item = json!({
        "id": id,
        "extensionId": extension_id,
        "extensionName": name,
        "publisherId": publisher,
        "publisherName": publisher_name,
        "version": version,
        "installState": {"flags": flags}
    });
    state.write(|db| -> Result<(), &'static str> {
        db.ensure_project(org, project);
        let Some(stored) = db.project_mut(org, project) else {
            return Err("extension introuvable");
        };
        let existing: Vec<Value> = stored
            .buckets
            .get(INSTALLED_EXTENSIONS)
            .map(|bucket| bucket.items.values().cloned().collect())
            .unwrap_or_default();
        if existing.iter().any(|stored| {
            stored.get("publisherId").and_then(|value| value.as_str()).is_some_and(|current| current.eq_ignore_ascii_case(&publisher))
                && stored
                    .get("extensionId")
                    .and_then(|value| value.as_str())
                    .is_some_and(|current| current.eq_ignore_ascii_case(&extension_id))
        }) {
            return Err("extension existante");
        }
        let bucket = stored.buckets.entry(INSTALLED_EXTENSIONS.to_string()).or_insert_with(Bucket::empty);
        bucket.put(item.clone());
        Ok(())
    })?;
    Ok(dress_extension(org, project, item))
}

fn extension_payload(body: &[u8]) -> Result<(String, String, String, String, String, String), &'static str> {
    let Some(payload) = parse_json(body) else {
        return Err("JSON illisible");
    };
    let Some(object) = payload.as_object() else {
        return Err("extension illisible");
    };
    let Some(name) = object
        .get("extensionName")
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
    else {
        return Err("nom absent");
    };
    let extension_id = match object.get("extensionId") {
        None => extension_slug(&name).ok_or("identifiant absent")?,
        Some(Value::String(value)) => {
            let value = value.trim();
            if extension_token(value) {
                value.to_string()
            } else {
                return Err("identifiant absent");
            }
        }
        Some(_) => return Err("identifiant absent"),
    };
    let publisher = match object.get("publisherId") {
        None => "azurz".to_string(),
        Some(Value::String(value)) if extension_token(value.trim()) => value.trim().to_string(),
        Some(_) => return Err("éditeur absent"),
    };
    let publisher_name = match object.get("publisherName") {
        None => publisher.clone(),
        Some(Value::String(value)) if !value.trim().is_empty() => value.trim().to_string(),
        Some(_) => return Err("éditeur absent"),
    };
    let version = match object.get("version") {
        None => "1.0.0".to_string(),
        Some(Value::String(value)) if extension_version(value.trim()) => value.trim().to_string(),
        Some(_) => return Err("version absente"),
    };
    let flags = match object.get("installState") {
        None => match object.get("flags") {
            None => "none".to_string(),
            Some(Value::String(value)) if value == "none" || value == "disabled" => value.clone(),
            Some(_) => return Err("état absent"),
        },
        Some(Value::Object(state)) => match state.get("flags") {
            Some(Value::String(value)) if value == "none" || value == "disabled" => value.clone(),
            _ => return Err("état absent"),
        },
        Some(_) => return Err("état absent"),
    };
    Ok((name, extension_id, publisher, publisher_name, version, flags))
}

fn extension_token(value: &str) -> bool {
    let mut chars = value.chars();
    matches!(chars.next(), Some(char) if char.is_ascii_alphabetic()) && chars.all(|char| char.is_ascii_alphanumeric())
}

fn extension_slug(name: &str) -> Option<String> {
    let slug: String = name.chars().filter(|char| char.is_ascii_alphanumeric()).collect();
    if extension_token(&slug) { Some(slug) } else { None }
}

fn extension_version(value: &str) -> bool {
    let mut parts = value.split('.');
    let numeric = |part: Option<&str>| part.is_some_and(|part| !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit()));
    numeric(parts.next()) && numeric(parts.next()) && numeric(parts.next()) && parts.next().is_none()
}

fn dress_extension(org: &str, project: &str, mut item: Value) -> Value {
    let publisher = item.get("publisherId").and_then(|value| value.as_str()).unwrap_or("azurz").to_string();
    let extension_id = item.get("extensionId").and_then(|value| value.as_str()).unwrap_or("").to_string();
    if let Some(object) = item.as_object_mut() {
        object.insert("project".into(), json!(project));
        object.insert(
            "url".into(),
            json!(format!("/{org}/{project}/_apis/extensionmanagement/installedextensions/{publisher}/{extension_id}")),
        );
    }
    item
}

const GROUP_ENTITLEMENTS: &str = "memberentitlementmanagement/groupentitlements";

pub fn group_entitlements(
    state: &AppState,
    method: &Method,
    org: &str,
    project: Option<&str>,
    rest: &[String],
    body: &[u8],
) -> Response {
    let Some(tail) = group_entitlement_tail(rest) else {
        return if method == Method::DELETE {
            fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée")
        } else {
            fail(StatusCode::NOT_FOUND, "droit de groupe introuvable")
        };
    };
    if tail.len() > 1 {
        return if method == Method::DELETE {
            fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée")
        } else {
            fail(StatusCode::NOT_FOUND, "droit de groupe introuvable")
        };
    }
    if tail.is_empty() {
        if method == Method::GET {
            let value = match project {
                Some(project) => project_group_entitlements(state, org, project),
                None => org_group_entitlements(state, org),
            };
            return Json(page(value)).into_response();
        }
        if method == Method::POST {
            let project = project.map(str::to_string).unwrap_or_else(|| state.names().1);
            return match group_entitlement_create(state, org, &project, body) {
                Ok(item) => Json(item).into_response(),
                Err(message) => fail(StatusCode::BAD_REQUEST, message),
            };
        }
        return fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée");
    }
    let id = pct_decode(&tail[0]);
    if method == Method::GET {
        return match locate_group_entitlement(state, org, project, id.trim()) {
            Some((owner, found)) => Json(dress_group_entitlement(org, &owner, found)).into_response(),
            None => fail(StatusCode::NOT_FOUND, "droit de groupe introuvable"),
        };
    }
    if method == Method::DELETE {
        let Some((owner, found)) = locate_group_entitlement(state, org, project, id.trim()) else {
            return fail(StatusCode::NOT_FOUND, "droit de groupe introuvable");
        };
        let stored_id = value_key(&found["id"]);
        let removed = state.write(|db| -> bool {
            db.project_mut(org, &owner)
                .and_then(|stored| stored.buckets.get_mut(GROUP_ENTITLEMENTS))
                .is_some_and(|bucket| bucket.remove(&stored_id))
        });
        return if removed {
            StatusCode::NO_CONTENT.into_response()
        } else {
            fail(StatusCode::NOT_FOUND, "droit de groupe introuvable")
        };
    }
    fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée")
}

fn group_entitlement_tail(rest: &[String]) -> Option<&[String]> {
    let index = rest.iter().position(|part| part.eq_ignore_ascii_case("groupentitlements"))?;
    Some(&rest[index + 1..])
}

fn project_group_entitlements(state: &AppState, org: &str, project: &str) -> Vec<Value> {
    let mut rows = list_group_entitlements(state, org, project);
    rows.sort_by(|left, right| value_key(&left["id"]).cmp(&value_key(&right["id"])));
    rows.into_iter().map(|item| dress_group_entitlement(org, project, item)).collect()
}

fn org_group_entitlements(state: &AppState, org: &str) -> Vec<Value> {
    let mut rows = all_group_entitlements(state, org);
    rows.sort_by(|left, right| value_key(&left.1["id"]).cmp(&value_key(&right.1["id"])));
    rows.into_iter().map(|(project, item)| dress_group_entitlement(org, &project, item)).collect()
}

fn list_group_entitlements(state: &AppState, org: &str, project: &str) -> Vec<Value> {
    state.read(|db| {
        db.project(org, project)
            .map(|stored| stored.list_bucket(GROUP_ENTITLEMENTS))
            .unwrap_or_default()
    })
}

fn all_group_entitlements(state: &AppState, org: &str) -> Vec<(String, Value)> {
    state.read(|db| {
        let Some(organization) = db.orgs.get(org) else {
            return Vec::new();
        };
        let mut rows = Vec::new();
        for (name, project) in &organization.projects {
            if let Some(bucket) = project.buckets.get(GROUP_ENTITLEMENTS) {
                for item in bucket.items.values() {
                    rows.push((name.clone(), item.clone()));
                }
            }
        }
        rows
    })
}

fn locate_group_entitlement(state: &AppState, org: &str, project: Option<&str>, id: &str) -> Option<(String, Value)> {
    state.read(|db| {
        let organization = db.orgs.get(org)?;
        for (name, stored) in &organization.projects {
            if let Some(project) = project {
                if !name.eq_ignore_ascii_case(project) && stored.id != project {
                    continue;
                }
            }
            if let Some(item) = stored.buckets.get(GROUP_ENTITLEMENTS).and_then(|bucket| {
                bucket.items.values().find(|item| {
                    value_key(&item["id"]) == id
                        || item
                            .get("group")
                            .and_then(|group| group.get("displayName"))
                            .and_then(|value| value.as_str())
                            .is_some_and(|name| name.eq_ignore_ascii_case(id))
                })
            }) {
                return Some((name.clone(), item.clone()));
            }
        }
        None
    })
}

fn next_group_entitlement_id(state: &AppState, org: &str) -> i64 {
    all_group_entitlements(state, org)
        .iter()
        .filter_map(|(_, item)| value_key(&item["id"]).strip_prefix("gent-")?.parse::<i64>().ok())
        .max()
        .unwrap_or(0)
        + 1
}

fn group_entitlement_create(state: &AppState, org: &str, project: &str, body: &[u8]) -> Result<Value, &'static str> {
    let (name, license, license_name, entitlements) = group_entitlement_payload(project, body)?;
    let id = format!("gent-{}", next_group_entitlement_id(state, org));
    let item = json!({
        "id": id,
        "group": {
            "displayName": name,
            "origin": "vsts",
            "originId": id,
            "subjectKind": "group"
        },
        "licenseRule": {
            "licensingSource": "account",
            "accountLicenseType": license,
            "licenseDisplayName": license_name
        },
        "projectEntitlements": entitlements,
        "status": "applied"
    });
    state.write(|db| -> Result<(), &'static str> {
        db.ensure_project(org, project);
        let Some(stored) = db.project_mut(org, project) else {
            return Err("droit de groupe introuvable");
        };
        let existing: Vec<Value> = stored
            .buckets
            .get(GROUP_ENTITLEMENTS)
            .map(|bucket| bucket.items.values().cloned().collect())
            .unwrap_or_default();
        if existing.iter().any(|stored| {
            stored
                .get("group")
                .and_then(|group| group.get("displayName"))
                .and_then(|value| value.as_str())
                .is_some_and(|current| current.eq_ignore_ascii_case(&name))
        }) {
            return Err("droit de groupe existant");
        }
        let bucket = stored.buckets.entry(GROUP_ENTITLEMENTS.to_string()).or_insert_with(Bucket::empty);
        bucket.put(item.clone());
        Ok(())
    })?;
    Ok(dress_group_entitlement(org, project, item))
}

fn group_entitlement_payload(project: &str, body: &[u8]) -> Result<(String, &'static str, &'static str, Vec<Value>), &'static str> {
    let Some(payload) = parse_json(body) else {
        return Err("JSON illisible");
    };
    let Some(object) = payload.as_object() else {
        return Err("droit de groupe illisible");
    };
    let Some(group) = object.get("group").and_then(|value| value.as_object()) else {
        return Err("nom absent");
    };
    let Some(name) = group
        .get("displayName")
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
    else {
        return Err("nom absent");
    };
    if let Some(origin) = group.get("origin") {
        if origin.as_str() != Some("vsts") {
            return Err("origine absente");
        }
    }
    if let Some(kind) = group.get("subjectKind") {
        if kind.as_str() != Some("group") {
            return Err("sujet absent");
        }
    }
    let (license, license_name) = match object.get("licenseRule") {
        None => ("express", "Basic"),
        Some(rule) => {
            let Some(rule) = rule.as_object() else {
                return Err("licence absente");
            };
            if let Some(source) = rule.get("licensingSource") {
                if source.as_str() != Some("account") {
                    return Err("licence absente");
                }
            }
            let Some(raw) = rule.get("accountLicenseType").and_then(|value| value.as_str()).map(str::trim).filter(|value| !value.is_empty()) else {
                return Err("licence absente");
            };
            let Some(pair) = canonical_license(raw) else {
                return Err("licence inconnue");
            };
            pair
        }
    };
    let entitlements = match object.get("projectEntitlements") {
        None => vec![json!({
            "projectRef": {"id": project, "name": project},
            "group": {"groupType": "projectContributor", "displayName": "Contributeurs"}
        })],
        Some(value) => {
            let rows = read_project_entitlements(value)?;
            if rows.is_empty() {
                return Err("projet absent");
            }
            rows
        }
    };
    Ok((name, license, license_name, entitlements))
}

fn dress_group_entitlement(org: &str, project: &str, mut item: Value) -> Value {
    let id = value_key(&item["id"]);
    if let Some(object) = item.as_object_mut() {
        object.insert("project".into(), json!(project));
        object.insert(
            "url".into(),
            json!(format!("/{org}/{project}/_apis/memberentitlementmanagement/groupentitlements/{id}")),
        );
    }
    item
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

fn work_item_kind_rows() -> &'static [(&'static str, &'static str, &'static str)] {
    &[
        ("Epic", "Microsoft.VSTS.WorkItemTypes.Epic", "E06C00"),
        ("Feature", "Microsoft.VSTS.WorkItemTypes.Feature", "773B93"),
        ("User Story", "Microsoft.VSTS.WorkItemTypes.UserStory", "0098C7"),
        ("Bug", "Microsoft.VSTS.WorkItemTypes.Bug", "CC293D"),
        ("Task", "Microsoft.VSTS.WorkItemTypes.Task", "F2CB1D"),
        ("Issue", "Microsoft.VSTS.WorkItemTypes.Issue", "B4009E"),
        ("Test Case", "Microsoft.VSTS.WorkItemTypes.TestCase", "004B50"),
    ]
}

fn canonical_work_item_type(name: &str) -> Option<&'static str> {
    let name = name.trim();
    work_item_kind_rows()
        .iter()
        .find(|(kind, _, _)| kind.eq_ignore_ascii_case(name))
        .map(|(kind, _, _)| *kind)
}

fn work_item_type_body(name: &str, reference: &str, color: &str, states: bool) -> Value {
    let mut item = json!({
        "name": name,
        "referenceName": reference,
        "description": name,
        "color": color,
        "isDisabled": false,
        "xmlForm": "",
        "fields": []
    });
    if states {
        item["states"] = json!(type_state_values());
    }
    item
}

fn work_item_type_document(name: &str) -> Value {
    let (_, reference, color) = work_item_kind_rows()
        .iter()
        .find(|(kind, _, _)| *kind == name)
        .copied()
        .unwrap_or(("User Story", "Microsoft.VSTS.WorkItemTypes.UserStory", "0098C7"));
    work_item_type_body(name, reference, color, true)
}

fn work_item_types() -> Value {
    let value = work_item_kind_rows()
        .iter()
        .map(|(name, reference, color)| work_item_type_body(name, reference, color, false))
        .collect();
    page(value)
}

fn catalog_state_names() -> Vec<String> {
    type_state_values()
        .iter()
        .filter_map(|state| state.get("name").and_then(|value| value.as_str()).map(str::to_string))
        .collect()
}

fn transition_edges(names: &[String]) -> Vec<Value> {
    names.iter().map(|name| json!({"to": name, "actions": null})).collect()
}

fn work_item_transitions(extras: &[String]) -> Value {
    let catalog = catalog_state_names();
    let mut extra_names = Vec::new();
    for extra in extras {
        if catalog.iter().any(|name| name.eq_ignore_ascii_case(extra)) {
            continue;
        }
        if extra_names.iter().any(|name: &String| name.eq_ignore_ascii_case(extra)) {
            continue;
        }
        extra_names.push(extra.clone());
    }
    let mut broad = catalog.clone();
    broad.extend(extra_names.iter().cloned());
    let mut graph = Map::new();
    graph.insert(String::new(), Value::Array(transition_edges(&broad)));
    for name in &catalog {
        graph.insert(name.clone(), Value::Array(transition_edges(&broad)));
    }
    for extra in &extra_names {
        let mut narrow = catalog.clone();
        narrow.push(extra.clone());
        graph.insert(extra.clone(), Value::Array(transition_edges(&narrow)));
    }
    Value::Object(graph)
}

fn transition_allowed(current: &str, next: &str, extras: &[String]) -> Result<(), &'static str> {
    let graph = work_item_transitions(extras);
    let Some(object) = graph.as_object() else {
        return Err("transition inconnue");
    };
    if let Some(edges) = object.iter().find(|(key, _)| key.eq_ignore_ascii_case(current)).and_then(|(_, value)| value.as_array()) {
        if edges.iter().any(|edge| edge.get("to").and_then(|value| value.as_str()).is_some_and(|name| name.eq_ignore_ascii_case(next))) {
            return Ok(());
        }
        return Err("transition inconnue");
    }
    if catalog_state_names().iter().any(|name| name.eq_ignore_ascii_case(next)) {
        return Ok(());
    }
    Err("transition inconnue")
}

fn type_state_values() -> Vec<Value> {
    [
        ("New", "b2b2b2", "Proposed"),
        ("Active", "007acc", "InProgress"),
        ("Resolved", "ff9d00", "Resolved"),
        ("Closed", "339933", "Completed"),
    ]
    .into_iter()
    .map(|(name, color, category)| json!({"name": name, "color": color, "category": category}))
    .collect()
}

fn canonical_work_item_state(value: &Value, extras: &[String]) -> Result<String, &'static str> {
    let text = value.as_str().ok_or("état illisible")?.trim();
    if text.is_empty() {
        return Err("état absent");
    }
    if let Some(name) = type_state_values()
        .iter()
        .find_map(|state| state.get("name").and_then(|value| value.as_str()).filter(|name| name.eq_ignore_ascii_case(text)))
    {
        return Ok(name.to_string());
    }
    if let Some(extra) = extras.iter().find(|extra| extra.eq_ignore_ascii_case(text)) {
        return Ok(extra.clone());
    }
    Err("état inconnu")
}

fn mapped_work_item_states(state: &AppState, org: &str, project: &str, kind: &str) -> Vec<String> {
    state.read(|db| {
        let Some(stored) = db.project(org, project) else {
            return Vec::new();
        };
        let mut extras = Vec::new();
        for (key, bucket) in &stored.buckets {
            if !key.starts_with("work/boards/") {
                continue;
            }
            for item in bucket.items.values() {
                let Some(columns) = item.get("columns").and_then(|value| value.as_array()) else {
                    continue;
                };
                for column in columns {
                    let Some(mappings) = column.get("stateMappings").and_then(|value| value.as_object()) else {
                        continue;
                    };
                    for (mapped_kind, mapped) in mappings {
                        if !mapped_kind.eq_ignore_ascii_case(kind) {
                            continue;
                        }
                        let Some(text) = mapped.as_str().map(str::trim).filter(|text| !text.is_empty()) else {
                            continue;
                        };
                        if extras.iter().any(|extra: &String| extra.eq_ignore_ascii_case(text)) {
                            continue;
                        }
                        extras.push(text.to_string());
                    }
                }
            }
        }
        extras
    })
}

fn catalog_fields() -> Vec<Value> {
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
    fields
        .iter()
        .map(|(reference, kind, name)| json!({"referenceName": reference, "name": name, "type": kind, "readOnly": false}))
        .collect()
}

fn wit_fields() -> Value {
    page(catalog_fields())
}

const CUSTOM_FIELDS: &str = "wit/fields";

pub fn custom_fields(
    state: &AppState,
    method: &Method,
    org: &str,
    project: Option<&str>,
    rest: &[String],
    body: &[u8],
) -> Response {
    let Some(tail) = fields_tail(rest) else {
        return if method == Method::DELETE {
            fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée")
        } else {
            fail(StatusCode::NOT_FOUND, "champ introuvable")
        };
    };
    if tail.is_empty() {
        if method == Method::GET {
            let value = match project {
                Some(project) => project_fields(state, org, project),
                None => org_fields(state, org),
            };
            return Json(page(value)).into_response();
        }
        if method == Method::POST {
            let project = project.map(str::to_string).unwrap_or_else(|| state.names().1);
            return match field_create(state, org, &project, body) {
                Ok(item) => Json(item).into_response(),
                Err(message) => fail(StatusCode::BAD_REQUEST, message),
            };
        }
        return fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée");
    }
    if tail.len() != 1 {
        return if method == Method::DELETE {
            fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée")
        } else {
            fail(StatusCode::NOT_FOUND, "champ introuvable")
        };
    }
    let id = tail[0].as_str();
    if method == Method::GET {
        if let Some((owner, found)) = locate_field(state, org, project, id) {
            return Json(dress_field(org, &owner, found)).into_response();
        }
        if let Some(found) = catalog_field(id) {
            return Json(found).into_response();
        }
        return fail(StatusCode::NOT_FOUND, "champ introuvable");
    }
    if method == Method::DELETE {
        if let Some((owner, found)) = locate_field(state, org, project, id) {
            let stored_id = field_id(&found);
            let removed = state.write(|db| -> bool {
                db.project_mut(org, &owner)
                    .and_then(|stored| stored.buckets.get_mut(CUSTOM_FIELDS))
                    .is_some_and(|bucket| bucket.remove(&stored_id))
            });
            return if removed {
                StatusCode::NO_CONTENT.into_response()
            } else {
                fail(StatusCode::NOT_FOUND, "champ introuvable")
            };
        }
        if catalog_field(id).is_some() {
            return fail(StatusCode::BAD_REQUEST, "champ système");
        }
        return fail(StatusCode::NOT_FOUND, "champ introuvable");
    }
    fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée")
}

fn fields_tail(rest: &[String]) -> Option<&[String]> {
    let index = rest.iter().position(|part| part.eq_ignore_ascii_case("fields"))?;
    Some(&rest[index + 1..])
}

fn project_fields(state: &AppState, org: &str, project: &str) -> Vec<Value> {
    let mut value = catalog_fields();
    let mut custom = list_fields(state, org, project);
    custom.sort_by(|left, right| field_id(left).cmp(&field_id(right)));
    value.extend(custom.into_iter().map(|item| dress_field(org, project, item)));
    value
}

fn org_fields(state: &AppState, org: &str) -> Vec<Value> {
    let mut value = catalog_fields();
    let mut rows = all_fields(state, org);
    rows.sort_by(|left, right| field_id(&left.1).cmp(&field_id(&right.1)));
    value.extend(rows.into_iter().map(|(project, item)| dress_field(org, &project, item)));
    value
}

fn list_fields(state: &AppState, org: &str, project: &str) -> Vec<Value> {
    state.read(|db| db.project(org, project).map(|stored| stored.list_bucket(CUSTOM_FIELDS)).unwrap_or_default())
}

fn all_fields(state: &AppState, org: &str) -> Vec<(String, Value)> {
    state.read(|db| {
        let Some(organization) = db.orgs.get(org) else {
            return Vec::new();
        };
        let mut rows = Vec::new();
        for (name, project) in &organization.projects {
            if let Some(bucket) = project.buckets.get(CUSTOM_FIELDS) {
                for item in bucket.items.values() {
                    rows.push((name.clone(), item.clone()));
                }
            }
        }
        rows
    })
}

fn locate_field(state: &AppState, org: &str, project: Option<&str>, id: &str) -> Option<(String, Value)> {
    state.read(|db| {
        let organization = db.orgs.get(org)?;
        for (name, stored) in &organization.projects {
            if let Some(project) = project {
                if !name.eq_ignore_ascii_case(project) && stored.id != project {
                    continue;
                }
            }
            if let Some(item) = stored
                .buckets
                .get(CUSTOM_FIELDS)
                .and_then(|bucket| bucket.items.values().find(|item| field_matches(item, id)))
            {
                return Some((name.clone(), item.clone()));
            }
        }
        None
    })
}

fn field_matches(item: &Value, id: &str) -> bool {
    field_id(item) == id
        || item.get("name").and_then(|value| value.as_str()).is_some_and(|name| name.eq_ignore_ascii_case(id))
        || item
            .get("referenceName")
            .and_then(|value| value.as_str())
            .is_some_and(|name| name.eq_ignore_ascii_case(id))
}

fn field_id(item: &Value) -> String {
    value_key(&item["id"])
}

fn catalog_field(id: &str) -> Option<Value> {
    catalog_fields().into_iter().find(|item| {
        item["referenceName"].as_str().is_some_and(|name| name.eq_ignore_ascii_case(id))
            || item["name"].as_str().is_some_and(|name| name.eq_ignore_ascii_case(id))
    })
}

fn next_field_id(state: &AppState, org: &str) -> i64 {
    state.read(|db| {
        let Some(organization) = db.orgs.get(org) else {
            return 1;
        };
        organization
            .projects
            .values()
            .filter_map(|project| project.buckets.get(CUSTOM_FIELDS))
            .flat_map(|bucket| bucket.items.values())
            .filter_map(|item| field_id(item).strip_prefix("field-")?.parse::<i64>().ok())
            .max()
            .unwrap_or(0)
            + 1
    })
}

fn field_create(state: &AppState, org: &str, project: &str, body: &[u8]) -> Result<Value, &'static str> {
    let (name, reference, kind, description, read_only) = field_payload(body)?;
    if catalog_conflict(&name, &reference) {
        return Err("champ existant");
    }
    let id = format!("field-{}", next_field_id(state, org));
    let item = json!({
        "id": id,
        "name": name,
        "referenceName": reference,
        "type": kind,
        "description": description,
        "readOnly": read_only
    });
    state.write(|db| -> Result<(), &'static str> {
        db.ensure_project(org, project);
        let Some(stored) = db.project_mut(org, project) else {
            return Err("champ introuvable");
        };
        let existing: Vec<Value> = stored
            .buckets
            .get(CUSTOM_FIELDS)
            .map(|bucket| bucket.items.values().cloned().collect())
            .unwrap_or_default();
        if custom_conflict(&existing, &name, &reference) {
            return Err("champ existant");
        }
        let bucket = stored.buckets.entry(CUSTOM_FIELDS.to_string()).or_insert_with(Bucket::empty);
        bucket.put(item.clone());
        Ok(())
    })?;
    Ok(dress_field(org, project, item))
}

fn field_payload(body: &[u8]) -> Result<(String, String, String, String, bool), &'static str> {
    let Some(payload) = parse_json(body) else {
        return Err("JSON illisible");
    };
    let Some(object) = payload.as_object() else {
        return Err("champ illisible");
    };
    let Some(name) = object
        .get("name")
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
    else {
        return Err("nom absent");
    };
    let reference = match object.get("referenceName") {
        None => {
            let tail: String = name.chars().filter(|ch| ch.is_ascii_alphanumeric()).collect();
            let reference = format!("Custom.{tail}");
            if !valid_reference(&reference) {
                return Err("référence absente");
            }
            reference
        }
        Some(Value::String(value)) => {
            let reference = value.trim().to_string();
            if !valid_reference(&reference) {
                return Err("référence absente");
            }
            reference
        }
        Some(_) => return Err("référence absente"),
    };
    let kind = match object.get("type") {
        None => "string".to_string(),
        Some(Value::String(value))
            if matches!(
                value.as_str(),
                "string" | "integer" | "double" | "dateTime" | "html" | "boolean" | "identity" | "treePath"
            ) =>
        {
            value.clone()
        }
        Some(_) => return Err("type absent"),
    };
    let description = match object.get("description") {
        None => String::new(),
        Some(Value::String(value)) => value.clone(),
        Some(_) => return Err("description absente"),
    };
    let read_only = match object.get("readOnly") {
        None => false,
        Some(Value::Bool(value)) => *value,
        Some(_) => return Err("lecture absente"),
    };
    Ok((name, reference, kind, description, read_only))
}

fn valid_reference(value: &str) -> bool {
    let Some((head, tail)) = value.split_once('.') else {
        return false;
    };
    if head.contains('.') || tail.contains('.') {
        return false;
    }
    reference_ident(head, false) && reference_ident(tail, true)
}

fn reference_ident(value: &str, allow_underscore: bool) -> bool {
    let mut chars = value.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !first.is_ascii_alphabetic() {
        return false;
    }
    chars.all(|ch| ch.is_ascii_alphanumeric() || (allow_underscore && ch == '_'))
}

fn catalog_conflict(name: &str, reference: &str) -> bool {
    catalog_fields().iter().any(|item| {
        let catalog_name = item["name"].as_str().unwrap_or("");
        let catalog_reference = item["referenceName"].as_str().unwrap_or("");
        catalog_name.eq_ignore_ascii_case(name)
            || catalog_name.eq_ignore_ascii_case(reference)
            || catalog_reference.eq_ignore_ascii_case(name)
            || catalog_reference.eq_ignore_ascii_case(reference)
    })
}

fn custom_conflict(items: &[Value], name: &str, reference: &str) -> bool {
    items.iter().any(|item| {
        let existing_name = item["name"].as_str().unwrap_or("");
        let existing_reference = item["referenceName"].as_str().unwrap_or("");
        existing_name.eq_ignore_ascii_case(name)
            || existing_name.eq_ignore_ascii_case(reference)
            || existing_reference.eq_ignore_ascii_case(name)
            || existing_reference.eq_ignore_ascii_case(reference)
    })
}

fn dress_field(org: &str, project: &str, mut item: Value) -> Value {
    let id = field_id(&item);
    if let Some(object) = item.as_object_mut() {
        object.insert("project".into(), json!(project));
        object.insert("url".into(), json!(format!("/{org}/{project}/_apis/wit/fields/{id}")));
    }
    item
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

pub fn prepare_work_item_ops(state: &AppState, org: &str, project: &str, kind: &str, current: &str, ops: &[Value]) -> Result<Vec<Value>, &'static str> {
    ensure_classification(state, org, project);
    let areas = work_item_paths(state, org, project, "areas");
    let iterations = work_item_paths(state, org, project, "iterations");
    let mapped = mapped_work_item_states(state, org, project, kind);
    let mut cursor = current.to_string();
    let mut prepared = Vec::new();
    for op in ops {
        let mut op = op.clone();
        let path = op.get("path").and_then(|value| value.as_str()).unwrap_or("");
        let Some(field) = path.strip_prefix("/fields/") else {
            prepared.push(op);
            continue;
        };
        let Some(value) = op.get("value").cloned() else {
            prepared.push(op);
            continue;
        };
        if field == "System.AssignedTo" && identity_is_blank(&value) {
            if let Some(object) = op.as_object_mut() {
                object.insert("op".into(), json!("remove"));
                object.remove("value");
            }
            prepared.push(op);
            continue;
        }
        let next = match field {
            "System.AreaPath" => json!(canonical_tree_path(&areas, &value, "chemin de zone absent", "zone inconnue")?),
            "System.IterationPath" => json!(canonical_tree_path(
                &iterations,
                &value,
                "chemin d'itération absent",
                "itération inconnue"
            )?),
            "System.AssignedTo" => canonical_identity(&value)?,
            "System.Tags" => json!(canonical_tags(&value)?),
            "System.State" => {
                let next = canonical_work_item_state(&value, &mapped)?;
                transition_allowed(&cursor, &next, &mapped)?;
                cursor = next.clone();
                json!(next)
            }
            _ => value,
        };
        if let Some(object) = op.as_object_mut() {
            object.insert("value".into(), next);
        }
        prepared.push(op);
    }
    Ok(prepared)
}

fn work_item_paths(state: &AppState, org: &str, project: &str, kind: &str) -> Vec<String> {
    classification_nodes_of(state, org, project, kind)
        .iter()
        .filter_map(|node| {
            let path = text_of(node, "path")?;
            work_item_path(project, kind, &path)
        })
        .collect()
}

fn work_item_path(project: &str, kind: &str, node_path: &str) -> Option<String> {
    let marker = if kind == "iterations" { "Iteration" } else { "Area" };
    let root = format!("\\{project}\\{marker}");
    if node_path.eq_ignore_ascii_case(&root) {
        return Some(project.to_string());
    }
    let prefix = format!("{root}\\");
    if node_path.len() > prefix.len() && node_path[..prefix.len()].eq_ignore_ascii_case(&prefix) {
        return Some(format!("{project}\\{}", &node_path[prefix.len()..]));
    }
    None
}

fn canonical_tree_path(known: &[String], value: &Value, absent: &'static str, unknown: &'static str) -> Result<String, &'static str> {
    let text = value.as_str().unwrap_or("").trim();
    if text.is_empty() {
        return Err(absent);
    }
    known
        .iter()
        .find(|path| path.eq_ignore_ascii_case(text))
        .cloned()
        .ok_or(unknown)
}

fn identity_is_blank(value: &Value) -> bool {
    match value {
        Value::String(text) => text.trim().is_empty(),
        Value::Null => true,
        Value::Object(object) => {
            let display = object.get("displayName").and_then(|item| item.as_str()).unwrap_or("").trim();
            let unique = object.get("uniqueName").and_then(|item| item.as_str()).unwrap_or("").trim();
            display.is_empty() && unique.is_empty()
        }
        _ => false,
    }
}

fn canonical_identity(value: &Value) -> Result<Value, &'static str> {
    let (display, unique) = match value {
        Value::String(text) => {
            let name = text.trim();
            if name.is_empty() {
                return Err("assigné absent");
            }
            (name.to_string(), name.to_string())
        }
        Value::Object(object) => {
            let display = object.get("displayName").and_then(|item| item.as_str()).unwrap_or("").trim().to_string();
            let unique = object.get("uniqueName").and_then(|item| item.as_str()).unwrap_or("").trim().to_string();
            let display = if display.is_empty() { unique.clone() } else { display };
            let unique = if unique.is_empty() { display.clone() } else { unique };
            if display.is_empty() {
                return Err("assigné absent");
            }
            (display, unique)
        }
        _ => return Err("assigné absent"),
    };
    Ok(json!({"displayName": display, "uniqueName": unique}))
}

fn canonical_tags(value: &Value) -> Result<String, &'static str> {
    let text = value.as_str().ok_or("étiquettes illisibles")?;
    let mut tags = Vec::new();
    for tag in text.split(';') {
        let tag = tag.trim();
        if tag.is_empty() || tags.iter().any(|existing: &String| existing == tag) {
            continue;
        }
        tags.push(tag.to_string());
    }
    Ok(tags.join("; "))
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

fn fill_definition(project: &str, mut item: Value) -> Value {
    if let Some(object) = item.as_object_mut() {
        object.entry("path").or_insert(json!("\\"));
        object.entry("type").or_insert(json!("build"));
        object.entry("queueStatus").or_insert(json!("enabled"));
        object.entry("revision").or_insert(json!(1));
        object.entry("variableGroups").or_insert(json!([]));
        object.entry("variables").or_insert(json!({}));
        object.entry("triggers").or_insert(json!([]));
        object.entry("project").or_insert(json!({"name": project}));
    }
    item
}

fn dress_definition(project: &str, item: Value) -> Value {
    let mut item = fill_definition(project, item);
    if let Some(entries) = item.get_mut("variables").and_then(|value| value.as_object_mut()) {
        for spec in entries.values_mut() {
            let is_secret = spec.get("isSecret").and_then(|value| value.as_bool()).unwrap_or(false);
            if is_secret {
                if let Some(object) = spec.as_object_mut() {
                    object.insert("value".into(), json!(""));
                }
            }
            if let Some(object) = spec.as_object_mut() {
                object.entry("allowOverride").or_insert(json!(true));
            }
        }
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
        let next = list(state, org, project, key)
            .into_iter()
            .filter_map(|item| item.get("id").and_then(|value| value.as_i64()))
            .max()
            .unwrap_or(0)
            + 1;
        let settings = payload.get("settings").cloned().unwrap_or(json!({}));
        let name = policy_type_catalog()
            .into_iter()
            .find(|item| text_of(item, "id").as_deref() == Some(type_id.as_str()))
            .and_then(|item| text_of(&item, "displayName"))
            .unwrap_or_else(|| type_id.clone());
        let item = dress_policy(
            org,
            project,
            json!({
                "id": next,
                "name": name,
                "revision": 1,
                "isEnabled": payload.get("isEnabled").cloned().unwrap_or(json!(true)),
                "isBlocking": payload.get("isBlocking").cloned().unwrap_or(json!(true)),
                "isDeleted": false,
                "type": {"id": type_id},
                "settings": settings,
                "createdDate": "2026-10-29T00:00:00Z",
                "createdBy": {"displayName": "azurz"}
            }),
        );
        put_item(state, org, project, key, item.clone());
        return Json(item).into_response();
    }
    if method == Method::DELETE {
        if rest.len() != 3 {
            return fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée");
        }
        let Some(found) = get_item(state, org, project, key, &rest[2]) else {
            return fail(StatusCode::NOT_FOUND, "stratégie introuvable");
        };
        let stored_id = value_key(&found["id"]);
        state.write(|db| {
            if let Some(stored) = db.project_mut(org, project) {
                if let Some(bucket) = stored.buckets.get_mut(key) {
                    bucket.remove(&stored_id);
                }
            }
        });
        return StatusCode::NO_CONTENT.into_response();
    }
    let id = rest.get(2).map(String::as_str).unwrap_or("");
    if method == Method::GET {
        return match get_item(state, org, project, key, id) {
            Some(item) => Json(dress_policy(org, project, item)).into_response(),
            None => fail(StatusCode::NOT_FOUND, "stratégie introuvable"),
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

fn canonical_group_type(name: &str) -> Option<&'static str> {
    if name.eq_ignore_ascii_case("vsts") {
        Some("Vsts")
    } else if name.eq_ignore_ascii_case("azurekeyvault") {
        Some("AzureKeyVault")
    } else {
        None
    }
}

fn group_variables(incoming: &Value, previous: Option<&Value>) -> Result<Value, &'static str> {
    let Some(object) = incoming.as_object() else {
        return Ok(previous.cloned().unwrap_or(json!({})));
    };
    let mut stored = Map::new();
    for (key, spec) in object {
        let name = key.trim();
        if name.is_empty() {
            return Err("nom de variable absent");
        }
        let is_secret = spec.get("isSecret").and_then(|value| value.as_bool()).unwrap_or(false);
        let incoming_value = spec.get("value").and_then(|value| value.as_str()).unwrap_or("");
        let previous_spec = previous.and_then(|variables| variables.get(name).or_else(|| variables.get(key)));
        let previous_value = previous_spec.and_then(|item| item.get("value")).and_then(|value| value.as_str()).unwrap_or("");
        let previous_secret = previous_spec.and_then(|item| item.get("isSecret")).and_then(|value| value.as_bool()).unwrap_or(false);
        let value = if incoming_value.is_empty() && previous_secret {
            previous_value
        } else {
            incoming_value
        };
        stored.insert(name.to_string(), json!({"value": value, "isSecret": is_secret}));
    }
    Ok(Value::Object(stored))
}

fn dress_group(org: &str, project: &str, item: &Value) -> Value {
    let id = value_key(&item["id"]);
    let name = text_of(item, "name").unwrap_or_default();
    let variables = item.get("variables").cloned().unwrap_or(json!({}));
    let dressed = variables
        .as_object()
        .map(|object| {
            let mut shown = Map::new();
            for (key, spec) in object {
                let is_secret = spec.get("isSecret").and_then(|value| value.as_bool()).unwrap_or(false);
                let value = if is_secret {
                    ""
                } else {
                    spec.get("value").and_then(|value| value.as_str()).unwrap_or("")
                };
                shown.insert(key.clone(), json!({"value": value, "isSecret": is_secret}));
            }
            Value::Object(shown)
        })
        .unwrap_or(json!({}));
    json!({
        "id": item.get("id").cloned().unwrap_or(json!(id)),
        "name": name,
        "description": text_of(item, "description").unwrap_or_default(),
        "type": text_of(item, "type").unwrap_or_else(|| "Vsts".into()),
        "variables": dressed,
        "variableGroupProjectReferences": [{"name": name, "projectReference": {"name": project}}],
        "url": format!("/{org}/{project}/_apis/distributedtask/variablegroups/{id}")
    })
}

fn build_folders(state: &AppState, method: &Method, org: &str, project: &str, rest: &[String], body: &[u8]) -> Response {
    let key = "build/folders";
    if rest.len() == 2 {
        if method == &Method::GET {
            let mut rows = list(state, org, project, key);
            rows.push(json!({"id": "\\", "path": "\\", "description": ""}));
            rows.sort_by(|left, right| text_of(left, "path").unwrap_or_default().cmp(&text_of(right, "path").unwrap_or_default()));
            let value = rows.into_iter().map(|item| dress_build_folder(org, project, item)).collect();
            return Json(page(value)).into_response();
        }
        if method == &Method::POST {
            return create_build_folder(state, org, project, body);
        }
        return fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée");
    }
    let raw = rest[2..].iter().map(|part| pct_decode(part)).collect::<Vec<_>>().join("/");
    let path = match canonical_folder_path(&raw) {
        Ok(path) => path,
        Err("dossier racine") => {
            if method == &Method::GET {
                return Json(dress_build_folder(org, project, json!({"id": "\\", "path": "\\", "description": ""}))).into_response();
            }
            if method == &Method::DELETE {
                return fail(StatusCode::BAD_REQUEST, "dossier racine");
            }
            return fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée");
        }
        Err(message) => return fail(StatusCode::NOT_FOUND, message),
    };
    if method == &Method::GET {
        return match find_folder(&list(state, org, project, key), &path) {
            Some(item) => Json(dress_build_folder(org, project, item)).into_response(),
            None => fail(StatusCode::NOT_FOUND, "dossier introuvable"),
        };
    }
    if method == &Method::DELETE {
        let rows = list(state, org, project, key);
        let Some(item) = find_folder(&rows, &path) else {
            return fail(StatusCode::NOT_FOUND, "dossier introuvable");
        };
        let id = value_key(&item["id"]);
        let prefix = format!("{path}\\");
        for child in &rows {
            let child_path = text_of(child, "path").unwrap_or_default();
            if child_path.eq_ignore_ascii_case(&path) {
                continue;
            }
            if child_path.len() > prefix.len() && child_path[..prefix.len()].eq_ignore_ascii_case(&prefix) {
                delete_item(state, org, project, key, &value_key(&child["id"]));
            }
        }
        return if delete_item(state, org, project, key, &id) {
            StatusCode::NO_CONTENT.into_response()
        } else {
            fail(StatusCode::NOT_FOUND, "dossier introuvable")
        };
    }
    fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée")
}

fn create_build_folder(state: &AppState, org: &str, project: &str, body: &[u8]) -> Response {
    let Some(payload) = parse_json(body) else {
        return fail(StatusCode::BAD_REQUEST, "JSON illisible");
    };
    let Some(object) = payload.as_object() else {
        return fail(StatusCode::BAD_REQUEST, "chemin absent");
    };
    let raw = object.get("path").and_then(|value| value.as_str()).unwrap_or("");
    if raw.trim().is_empty() {
        return fail(StatusCode::BAD_REQUEST, "chemin absent");
    }
    let path = match canonical_folder_path(raw) {
        Ok(path) => path,
        Err(message) => return fail(StatusCode::BAD_REQUEST, message),
    };
    let description = match object.get("description") {
        None => String::new(),
        Some(value) => match value.as_str() {
            Some(text) => text.trim().to_string(),
            None => return fail(StatusCode::BAD_REQUEST, "description absente"),
        },
    };
    let existing = list(state, org, project, "build/folders");
    if find_folder(&existing, &path).is_some() {
        return fail(StatusCode::BAD_REQUEST, "dossier déjà présent");
    }
    let item = json!({"id": path, "path": path, "description": description});
    put_item(state, org, project, "build/folders", item.clone());
    Json(dress_build_folder(org, project, item)).into_response()
}

fn canonical_folder_path(raw: &str) -> Result<String, &'static str> {
    let normalized = raw.trim().replace('/', "\\");
    let body = normalized.trim_matches('\\').trim();
    if body.is_empty() {
        return Err("dossier racine");
    }
    let mut parts = Vec::new();
    for part in body.split('\\') {
        let part = part.trim();
        if part.is_empty() || part == "." || part == ".." {
            return Err("chemin absent");
        }
        parts.push(part.to_string());
    }
    Ok(format!("\\{}", parts.join("\\")))
}

fn find_folder(rows: &[Value], path: &str) -> Option<Value> {
    rows.iter()
        .find(|item| text_of(item, "path").is_some_and(|current| current.eq_ignore_ascii_case(path)))
        .cloned()
}

fn dress_build_folder(org: &str, project: &str, mut item: Value) -> Value {
    let path = text_of(&item, "path").unwrap_or_else(|| "\\".into());
    if let Some(object) = item.as_object_mut() {
        object.entry("description").or_insert(json!(""));
        object.insert("project".into(), json!({"name": project}));
        object.insert(
            "url".into(),
            json!(format!("/{org}/{project}/_apis/build/folders/{}", path.replace('\\', "%5C"))),
        );
    }
    item
}

fn deployment_groups(state: &AppState, method: &Method, org: &str, project: &str, rest: &[String], body: &[u8]) -> Response {
    let key = "distributedtask/deploymentgroups";
    if rest.len() == 2 {
        if method == &Method::GET {
            let mut rows = list(state, org, project, key);
            rows.sort_by_key(|item| item.get("id").and_then(json_i64).unwrap_or(0));
            let value = rows.into_iter().map(|item| dress_deployment_group(org, project, item)).collect();
            return Json(page(value)).into_response();
        }
        if method == &Method::POST {
            return create_deployment_group(state, org, project, body);
        }
        return fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée");
    }
    let id = pct_decode(&rest[2]).trim().to_string();
    if rest.get(3).is_some_and(|value| value.eq_ignore_ascii_case("targets")) {
        return deployment_targets(state, method, org, project, &id, rest, body);
    }
    if rest.len() != 3 {
        if method == &Method::DELETE {
            return fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée");
        }
        return fail(StatusCode::NOT_FOUND, "groupe de déploiement introuvable");
    }
    if id.is_empty() {
        return fail(StatusCode::NOT_FOUND, "groupe de déploiement introuvable");
    }
    if method == &Method::GET {
        return match get_item(state, org, project, key, &id) {
            Some(item) => Json(dress_deployment_group(org, project, item)).into_response(),
            None => fail(StatusCode::NOT_FOUND, "groupe de déploiement introuvable"),
        };
    }
    if method == &Method::DELETE {
        return if delete_item(state, org, project, key, &id) {
            StatusCode::NO_CONTENT.into_response()
        } else {
            fail(StatusCode::NOT_FOUND, "groupe de déploiement introuvable")
        };
    }
    fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée")
}

fn create_deployment_group(state: &AppState, org: &str, project: &str, body: &[u8]) -> Response {
    let Some(payload) = parse_json(body) else {
        return fail(StatusCode::BAD_REQUEST, "JSON illisible");
    };
    let Some(object) = payload.as_object() else {
        return fail(StatusCode::BAD_REQUEST, "nom absent");
    };
    let name = object.get("name").and_then(|value| value.as_str()).map(str::trim).unwrap_or("");
    if name.is_empty() {
        return fail(StatusCode::BAD_REQUEST, "nom absent");
    }
    let description = object
        .get("description")
        .and_then(|value| value.as_str())
        .map(str::trim)
        .unwrap_or("")
        .to_string();
    let next = list(state, org, project, "distributedtask/deploymentgroups")
        .iter()
        .filter_map(|item| item.get("id").and_then(json_i64))
        .max()
        .unwrap_or(0)
        + 1;
    let item = json!({
        "id": next,
        "name": name,
        "description": description
    });
    put_item(state, org, project, "distributedtask/deploymentgroups", item.clone());
    Json(dress_deployment_group(org, project, item)).into_response()
}

fn deployment_targets(
    state: &AppState,
    method: &Method,
    org: &str,
    project: &str,
    group_id: &str,
    rest: &[String],
    body: &[u8],
) -> Response {
    if group_id.is_empty() || get_item(state, org, project, "distributedtask/deploymentgroups", group_id).is_none() {
        return fail(StatusCode::NOT_FOUND, "groupe de déploiement introuvable");
    }
    let key = format!("distributedtask/deploymentgroups/{group_id}/targets");
    if rest.len() == 4 {
        if method == &Method::GET {
            let mut rows = list(state, org, project, &key);
            rows.sort_by_key(|item| item.get("id").and_then(json_i64).unwrap_or(0));
            let value = rows
                .into_iter()
                .map(|item| dress_deployment_target(state, org, project, group_id, item))
                .collect();
            return Json(page(value)).into_response();
        }
        if method == &Method::POST {
            return create_deployment_target(state, org, project, group_id, body);
        }
        return fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée");
    }
    if rest.len() != 5 {
        if method == &Method::DELETE {
            return fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée");
        }
        return fail(StatusCode::NOT_FOUND, "cible introuvable");
    }
    let target_id = pct_decode(&rest[4]).trim().to_string();
    if target_id.is_empty() {
        return fail(StatusCode::NOT_FOUND, "cible introuvable");
    }
    if method == &Method::GET {
        return match get_item(state, org, project, &key, &target_id) {
            Some(item) => Json(dress_deployment_target(state, org, project, group_id, item)).into_response(),
            None => fail(StatusCode::NOT_FOUND, "cible introuvable"),
        };
    }
    if method == &Method::DELETE {
        return if delete_item(state, org, project, &key, &target_id) {
            StatusCode::NO_CONTENT.into_response()
        } else {
            fail(StatusCode::NOT_FOUND, "cible introuvable")
        };
    }
    fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée")
}

fn create_deployment_target(state: &AppState, org: &str, project: &str, group_id: &str, body: &[u8]) -> Response {
    let Some(payload) = parse_json(body) else {
        return fail(StatusCode::BAD_REQUEST, "JSON illisible");
    };
    let Some(object) = payload.as_object() else {
        return fail(StatusCode::BAD_REQUEST, "nom absent");
    };
    let name = object
        .get("name")
        .and_then(|value| value.as_str())
        .or_else(|| {
            object
                .get("machine")
                .and_then(|value| value.get("name"))
                .and_then(|value| value.as_str())
        })
        .map(str::trim)
        .unwrap_or("");
    if name.is_empty() {
        return fail(StatusCode::BAD_REQUEST, "nom absent");
    }
    let tags = match object.get("tags") {
        None => Vec::new(),
        Some(value) => match deployment_target_tags(value) {
            Ok(tags) => tags,
            Err(message) => return fail(StatusCode::BAD_REQUEST, message),
        },
    };
    let key = format!("distributedtask/deploymentgroups/{group_id}/targets");
    let existing = list(state, org, project, &key);
    if existing.iter().any(|item| text_of(item, "name").is_some_and(|current| current.eq_ignore_ascii_case(name))) {
        return fail(StatusCode::BAD_REQUEST, "cible déjà présente");
    }
    let next = existing.iter().filter_map(|item| item.get("id").and_then(json_i64)).max().unwrap_or(0) + 1;
    let item = json!({"id": next, "name": name, "tags": tags, "status": "online"});
    put_item(state, org, project, &key, item.clone());
    Json(dress_deployment_target(state, org, project, group_id, item)).into_response()
}

fn deployment_target_tags(value: &Value) -> Result<Vec<String>, &'static str> {
    let Some(rows) = value.as_array() else {
        return Err("étiquette absente");
    };
    let mut tags = Vec::new();
    for row in rows {
        let Some(text) = row.as_str().map(str::trim) else {
            return Err("étiquette absente");
        };
        if text.is_empty() {
            return Err("étiquette absente");
        }
        tags.push(text.to_string());
    }
    Ok(tags)
}

fn dress_deployment_target(state: &AppState, org: &str, project: &str, group_id: &str, mut item: Value) -> Value {
    let id = value_key(&item["id"]);
    let group_name = get_item(state, org, project, "distributedtask/deploymentgroups", group_id)
        .and_then(|group| text_of(&group, "name"))
        .unwrap_or_default();
    if let Some(object) = item.as_object_mut() {
        object.entry("tags").or_insert(json!([]));
        object.entry("status").or_insert(json!("online"));
        object.insert("deploymentGroup".into(), json!({"id": group_id.parse::<i64>().unwrap_or(0), "name": group_name}));
        object.insert(
            "url".into(),
            json!(format!("/{org}/{project}/_apis/distributedtask/deploymentgroups/{group_id}/targets/{id}")),
        );
    }
    item
}

fn dress_deployment_group(org: &str, project: &str, mut item: Value) -> Value {
    let id = value_key(&item["id"]);
    if let Some(object) = item.as_object_mut() {
        object.entry("description").or_insert(json!(""));
        object.insert("project".into(), json!({"name": project}));
        object.insert(
            "url".into(),
            json!(format!("/{org}/{project}/_apis/distributedtask/deploymentgroups/{id}")),
        );
    }
    item
}

fn task_groups(state: &AppState, method: &Method, org: &str, project: &str, rest: &[String], body: &[u8]) -> Response {
    let key = "distributedtask/taskgroups";
    if rest.len() == 2 {
        if method == &Method::GET {
            let mut rows = list(state, org, project, key);
            rows.sort_by_key(|item| item.get("id").and_then(json_i64).unwrap_or(0));
            let value = rows.into_iter().map(|item| dress_task_group(org, project, item)).collect();
            return Json(page(value)).into_response();
        }
        if method == &Method::POST {
            return create_task_group(state, org, project, body);
        }
        return fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée");
    }
    if rest.len() != 3 {
        if method == &Method::DELETE {
            return fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée");
        }
        return fail(StatusCode::NOT_FOUND, "groupe de tâches introuvable");
    }
    let id = pct_decode(&rest[2]).trim().to_string();
    if id.is_empty() {
        return fail(StatusCode::NOT_FOUND, "groupe de tâches introuvable");
    }
    if method == &Method::GET {
        return match get_item(state, org, project, key, &id) {
            Some(item) => Json(dress_task_group(org, project, item)).into_response(),
            None => fail(StatusCode::NOT_FOUND, "groupe de tâches introuvable"),
        };
    }
    if method == &Method::DELETE {
        return if delete_item(state, org, project, key, &id) {
            StatusCode::NO_CONTENT.into_response()
        } else {
            fail(StatusCode::NOT_FOUND, "groupe de tâches introuvable")
        };
    }
    fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée")
}

fn create_task_group(state: &AppState, org: &str, project: &str, body: &[u8]) -> Response {
    let Some(payload) = parse_json(body) else {
        return fail(StatusCode::BAD_REQUEST, "JSON illisible");
    };
    let Some(object) = payload.as_object() else {
        return fail(StatusCode::BAD_REQUEST, "nom absent");
    };
    let name = object.get("name").and_then(|value| value.as_str()).map(str::trim).unwrap_or("");
    if name.is_empty() {
        return fail(StatusCode::BAD_REQUEST, "nom absent");
    }
    let tasks = match task_group_steps(object.get("tasks")) {
        Ok(tasks) => tasks,
        Err(message) => return fail(StatusCode::BAD_REQUEST, message),
    };
    let description = object.get("description").and_then(|value| value.as_str()).unwrap_or("").trim().to_string();
    let next = list(state, org, project, "distributedtask/taskgroups")
        .iter()
        .filter_map(|item| item.get("id").and_then(json_i64))
        .max()
        .unwrap_or(0)
        + 1;
    let item = json!({
        "id": next,
        "name": name,
        "description": description,
        "tasks": tasks,
        "category": "Build",
        "author": "azurz",
        "revision": 1,
        "version": {"major": 1, "minor": 0, "patch": 0, "isTest": false}
    });
    put_item(state, org, project, "distributedtask/taskgroups", item.clone());
    Json(dress_task_group(org, project, item)).into_response()
}

fn task_group_steps(value: Option<&Value>) -> Result<Vec<Value>, &'static str> {
    let Some(value) = value else {
        return Err("tâche absente");
    };
    let Some(rows) = value.as_array() else {
        return Err("tâche absente");
    };
    if rows.is_empty() {
        return Err("tâche absente");
    }
    let mut out = Vec::new();
    for row in rows {
        let Some(object) = row.as_object() else {
            return Err("tâche absente");
        };
        let display = object.get("displayName").and_then(|item| item.as_str()).map(str::trim).unwrap_or("");
        if display.is_empty() {
            return Err("tâche absente");
        }
        let task = object.get("task").and_then(|item| item.as_object());
        let task_id = task
            .and_then(|item| item.get("id"))
            .and_then(|item| item.as_str())
            .map(str::trim)
            .filter(|item| !item.is_empty())
            .unwrap_or("cmd");
        let version = task
            .and_then(|item| item.get("versionSpec"))
            .and_then(|item| item.as_str())
            .map(str::trim)
            .filter(|item| !item.is_empty())
            .unwrap_or("1.*");
        let inputs = object.get("inputs").cloned().filter(|item| item.is_object()).unwrap_or_else(|| json!({}));
        let enabled = object.get("enabled").and_then(|item| item.as_bool()).unwrap_or(true);
        out.push(json!({
            "displayName": display,
            "enabled": enabled,
            "inputs": inputs,
            "task": {"id": task_id, "versionSpec": version}
        }));
    }
    Ok(out)
}

fn dress_task_group(org: &str, project: &str, mut item: Value) -> Value {
    let id = value_key(&item["id"]);
    if let Some(object) = item.as_object_mut() {
        object.entry("description").or_insert(json!(""));
        object.entry("category").or_insert(json!("Build"));
        object.entry("author").or_insert(json!("azurz"));
        object.entry("revision").or_insert(json!(1));
        object.entry("version").or_insert(json!({"major": 1, "minor": 0, "patch": 0, "isTest": false}));
        object.entry("tasks").or_insert(json!([]));
        object.insert("project".into(), json!({"name": project}));
        object.insert(
            "url".into(),
            json!(format!("/{org}/{project}/_apis/distributedtask/taskgroups/{id}")),
        );
    }
    item
}

fn variable_groups(state: &AppState, method: &Method, org: &str, project: &str, rest: &[String], body: &[u8]) -> Response {
    let key = "distributedtask/variablegroups";
    if rest.len() == 2 && method == Method::GET {
        let value = list(state, org, project, key).iter().map(|item| dress_group(org, project, item)).collect();
        return Json(page(value)).into_response();
    }
    if rest.len() == 2 && method == Method::POST {
        let payload = parse_json(body).unwrap_or(json!({}));
        let name = text_of(&payload, "name").unwrap_or_default();
        let name = name.trim().to_string();
        if name.is_empty() {
            return fail(StatusCode::BAD_REQUEST, "nom de groupe absent");
        }
        let kind = match payload.get("type").and_then(|value| value.as_str()) {
            Some(value) => match canonical_group_type(value) {
                Some(kind) => kind,
                None => return fail(StatusCode::BAD_REQUEST, "type de groupe inconnu"),
            },
            None => "Vsts",
        };
        if list(state, org, project, key)
            .iter()
            .any(|item| text_of(item, "name").is_some_and(|stored| stored.eq_ignore_ascii_case(&name)))
        {
            return fail(StatusCode::BAD_REQUEST, "groupe déjà présent");
        }
        let variables = match group_variables(payload.get("variables").unwrap_or(&json!({})), None) {
            Ok(variables) => variables,
            Err(message) => return fail(StatusCode::BAD_REQUEST, message),
        };
        let next = list(state, org, project, key)
            .iter()
            .filter_map(|item| item.get("id").and_then(|value| value.as_i64()))
            .max()
            .unwrap_or(0)
            + 1;
        let item = json!({
            "id": next,
            "name": name,
            "description": text_of(&payload, "description").unwrap_or_default(),
            "type": kind,
            "variables": variables
        });
        put_item(state, org, project, key, item.clone());
        return Json(dress_group(org, project, &item)).into_response();
    }
    let id = rest.get(2).map(String::as_str).unwrap_or("").to_string();
    if id.is_empty() {
        return fail(StatusCode::NOT_FOUND, "groupe introuvable");
    }
    if method == Method::GET {
        return match get_item(state, org, project, key, &id) {
            Some(item) => Json(dress_group(org, project, &item)).into_response(),
            None => fail(StatusCode::NOT_FOUND, "groupe introuvable"),
        };
    }
    if method == Method::PUT || method == Method::PATCH {
        let payload = parse_json(body).unwrap_or(json!({}));
        let Some(existing) = get_item(state, org, project, key, &id) else {
            return fail(StatusCode::NOT_FOUND, "groupe introuvable");
        };
        let name = text_of(&payload, "name").unwrap_or_else(|| text_of(&existing, "name").unwrap_or_default());
        let name = name.trim().to_string();
        if name.is_empty() {
            return fail(StatusCode::BAD_REQUEST, "nom de groupe absent");
        }
        let kind = match payload.get("type").and_then(|value| value.as_str()) {
            Some(value) => match canonical_group_type(value) {
                Some(kind) => kind,
                None => return fail(StatusCode::BAD_REQUEST, "type de groupe inconnu"),
            },
            None => "Vsts",
        };
        if list(state, org, project, key).iter().any(|item| {
            text_of(item, "name").is_some_and(|stored| stored.eq_ignore_ascii_case(&name))
                && value_key(&item["id"]) != id
        }) {
            return fail(StatusCode::BAD_REQUEST, "groupe déjà présent");
        }
        let variables = if let Some(incoming) = payload.get("variables") {
            match group_variables(incoming, existing.get("variables")) {
                Ok(variables) => variables,
                Err(message) => return fail(StatusCode::BAD_REQUEST, message),
            }
        } else {
            existing.get("variables").cloned().unwrap_or(json!({}))
        };
        let item = json!({
            "id": existing.get("id").cloned().unwrap_or(json!(id)),
            "name": name,
            "description": text_of(&payload, "description").unwrap_or_else(|| text_of(&existing, "description").unwrap_or_default()),
            "type": kind,
            "variables": variables
        });
        put_item(state, org, project, key, item.clone());
        return Json(dress_group(org, project, &item)).into_response();
    }
    if method == Method::DELETE {
        let Some(existing) = get_item(state, org, project, key, &id) else {
            return fail(StatusCode::NOT_FOUND, "groupe introuvable");
        };
        let notice = dress_group(org, project, &existing);
        if !delete_item(state, org, project, key, &id) {
            return fail(StatusCode::NOT_FOUND, "groupe introuvable");
        }
        deliver_work_item(state, org, project, "library.variablegroup.deleted", &notice);
        return StatusCode::NO_CONTENT.into_response();
    }
    fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée")
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
    if method == Method::DELETE && rest.len() != 3 {
        return fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée");
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
        let stored_id = value_key(&item["id"]);
        state.write(|db| {
            if let Some(stored) = db.project_mut(org, project) {
                if let Some(bucket) = stored.buckets.get_mut(key) {
                    bucket.remove(&stored_id);
                }
            }
        });
        return StatusCode::NO_CONTENT.into_response();
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
        return agent_queues(state, method, org, project, rest, body);
    }
    if resource != "pools" {
        return fail(StatusCode::NOT_FOUND, "ressource introuvable");
    }
    if rest.len() <= 2 {
        if method == Method::GET {
            let value = org_list(state, org, "distributedtask/pools")
                .into_iter()
                .map(|item| dress_pool(state, org, item))
                .collect();
            return Json(page(value)).into_response();
        }
        if method == Method::POST {
            return create_pool(state, org, body);
        }
        return fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée");
    }
    let pool_id = &rest[2];
    let Some(pool) = org_get(state, org, "distributedtask/pools", pool_id) else {
        return fail(StatusCode::NOT_FOUND, "pool introuvable");
    };
    let pool_key = value_key(&pool["id"]);
    if rest.len() == 3 {
        if method == Method::GET {
            return Json(dress_pool(state, org, pool)).into_response();
        }
        if method == Method::DELETE {
            return delete_pool(state, org, &pool);
        }
        return fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée");
    }
    match rest.get(3).map(|value| value.to_ascii_lowercase()).as_deref() {
        Some("agents") => pool_agents(state, method, org, &pool_key, rest, body),
        Some("jobrequests") => pool_jobs(state, method, org, &pool_key, rest, query, body),
        _ if method == Method::DELETE => fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée"),
        _ => fail(StatusCode::NOT_FOUND, "ressource introuvable"),
    }
}

fn agent_queues(
    state: &AppState,
    method: &Method,
    org: &str,
    project: Option<&str>,
    rest: &[String],
    body: &[u8],
) -> Response {
    let Some(project) = project else {
        return fail(StatusCode::NOT_FOUND, "file introuvable");
    };
    let key = "distributedtask/queues";
    if rest.len() <= 2 {
        if method == Method::GET {
            let value = list(state, org, project, key)
                .into_iter()
                .map(|item| dress_queue(org, project, item))
                .collect();
            return Json(page(value)).into_response();
        }
        if method == Method::POST {
            return create_queue(state, org, project, body);
        }
        return fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée");
    }
    if rest.len() != 3 {
        if method == Method::DELETE {
            return fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée");
        }
        return fail(StatusCode::NOT_FOUND, "file introuvable");
    }
    let Some(current) = get_item(state, org, project, key, &rest[2]) else {
        return fail(StatusCode::NOT_FOUND, "file introuvable");
    };
    if method == Method::GET {
        return Json(dress_queue(org, project, current)).into_response();
    }
    if method == Method::DELETE {
        if value_key(&current["id"]) == "1" {
            return fail(StatusCode::BAD_REQUEST, "file par défaut");
        }
        let stored_id = value_key(&current["id"]);
        delete_item(state, org, project, key, &stored_id);
        return StatusCode::NO_CONTENT.into_response();
    }
    fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée")
}

fn create_queue(state: &AppState, org: &str, project: &str, body: &[u8]) -> Response {
    if !body.is_empty() && parse_json(body).is_none() {
        return fail(StatusCode::BAD_REQUEST, "JSON illisible");
    }
    let payload = parse_json(body).unwrap_or(Value::Null);
    let name = text_of(&payload, "name").unwrap_or_default().trim().to_string();
    if name.is_empty() {
        return fail(StatusCode::BAD_REQUEST, "nom de file absent");
    }
    let pool_id = payload
        .get("pool")
        .map(|pool| value_key(&pool["id"]))
        .filter(|id| !id.is_empty())
        .or_else(|| payload.get("poolId").map(value_key).filter(|id| !id.is_empty()));
    let Some(pool_id) = pool_id else {
        return fail(StatusCode::BAD_REQUEST, "pool absent");
    };
    let Some(pool) = org_get(state, org, "distributedtask/pools", &pool_id) else {
        return fail(StatusCode::NOT_FOUND, "pool introuvable");
    };
    let pool_key = value_key(&pool["id"]);
    let pool_name = text_of(&pool, "name").unwrap_or_default();
    let next = list(state, org, project, "distributedtask/queues")
        .into_iter()
        .filter_map(|item| item.get("id").and_then(|value| value.as_i64()))
        .max()
        .unwrap_or(0)
        + 1;
    let numeric_pool = pool_key.parse::<i64>().unwrap_or(1);
    let item = json!({
        "id": next,
        "name": name,
        "pool": {"id": numeric_pool, "name": pool_name}
    });
    put_item(state, org, project, "distributedtask/queues", item.clone());
    Json(dress_queue(org, project, item)).into_response()
}

fn dress_queue(org: &str, project: &str, mut item: Value) -> Value {
    let id = value_key(&item["id"]);
    if let Some(object) = item.as_object_mut() {
        object
            .entry("url")
            .or_insert_with(|| json!(format!("/{org}/{project}/_apis/distributedtask/queues/{id}")));
    }
    item
}

fn create_pool(state: &AppState, org: &str, body: &[u8]) -> Response {
    if !body.is_empty() && parse_json(body).is_none() {
        return fail(StatusCode::BAD_REQUEST, "JSON illisible");
    }
    let payload = parse_json(body).unwrap_or(Value::Null);
    let name = text_of(&payload, "name").unwrap_or_default().trim().to_string();
    if name.is_empty() {
        return fail(StatusCode::BAD_REQUEST, "nom de pool absent");
    }
    let auto_provision = payload.get("autoProvision").and_then(|value| value.as_bool()).unwrap_or(false);
    let Some(item) = state.write(|db| {
        db.ensure_org(org);
        let organization = db.orgs.get_mut(org)?;
        let bucket = organization
            .buckets
            .entry("distributedtask/pools".into())
            .or_insert_with(Bucket::empty);
        let next = bucket
            .items
            .values()
            .filter_map(|item| item.get("id").and_then(|value| value.as_i64()))
            .max()
            .unwrap_or(0)
            + 1;
        let item = json!({
            "id": next,
            "name": name,
            "isHosted": false,
            "poolType": "automation",
            "size": 0,
            "autoProvision": auto_provision,
            "createdOn": "2026-10-29T00:00:00Z",
            "url": format!("/{org}/_apis/distributedtask/pools/{next}")
        });
        bucket.put(item.clone());
        if auto_provision {
            for project in organization.projects.values_mut() {
                let queues = project
                    .buckets
                    .entry("distributedtask/queues".into())
                    .or_insert_with(Bucket::empty);
                let queue_id = queues
                    .items
                    .values()
                    .filter_map(|item| item.get("id").and_then(|value| value.as_i64()))
                    .max()
                    .unwrap_or(0)
                    + 1;
                queues.put(json!({
                    "id": queue_id,
                    "name": name,
                    "pool": {"id": next, "name": name}
                }));
            }
        }
        Some(item)
    }) else {
        return fail(StatusCode::NOT_FOUND, "pool introuvable");
    };
    Json(item).into_response()
}

fn delete_pool(state: &AppState, org: &str, pool: &Value) -> Response {
    let id = value_key(&pool["id"]);
    if id == "1" {
        return fail(StatusCode::BAD_REQUEST, "pool par défaut");
    }
    state.write(|db| {
        let Some(organization) = db.orgs.get_mut(org) else {
            return;
        };
        if let Some(bucket) = organization.buckets.get_mut("distributedtask/pools") {
            bucket.remove(&id);
        }
        organization.buckets.remove(&format!("distributedtask/pools/{id}/agents"));
        organization.buckets.remove(&format!("distributedtask/pools/{id}/jobrequests"));
        for project in organization.projects.values_mut() {
            let Some(queues) = project.buckets.get_mut("distributedtask/queues") else {
                continue;
            };
            let doomed: Vec<String> = queues
                .items
                .iter()
                .filter(|(_, item)| value_key(&item["pool"]["id"]) == id)
                .map(|(key, _)| key.clone())
                .collect();
            for key in doomed {
                queues.items.remove(&key);
            }
        }
    });
    StatusCode::NO_CONTENT.into_response()
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
        let next = org_list(state, org, &key)
            .into_iter()
            .filter_map(|item| item.get("id").and_then(|value| value.as_i64()))
            .max()
            .unwrap_or(0)
            + 1;
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
    if method == Method::DELETE && rest.len() != 5 {
        return fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée");
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
        if method == Method::DELETE {
            let stored_id = value_key(&current["id"]);
            state.write(|db| {
                if let Some(organization) = db.orgs.get_mut(org) {
                    if let Some(bucket) = organization.buckets.get_mut(&key) {
                        bucket.remove(&stored_id);
                    }
                }
            });
            return StatusCode::NO_CONTENT.into_response();
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
        let next = org_list(state, org, &key)
            .into_iter()
            .filter_map(|item| {
                item.get("requestId")
                    .and_then(|value| value.as_i64())
                    .or_else(|| item.get("id").and_then(|value| value.as_i64()))
            })
            .max()
            .unwrap_or(0)
            + 1;
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
    if method == Method::DELETE && rest.len() != 5 {
        return fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée");
    }
    if rest.len() >= 5 {
        let request_id = &rest[4];
        let Some(current) = org_get(state, org, &key, request_id) else {
            return fail(StatusCode::NOT_FOUND, "demande introuvable");
        };
        if method == Method::GET {
            return Json(current).into_response();
        }
        if method == Method::DELETE {
            let stored_id = value_key(&current["id"]);
            state.write(|db| {
                if let Some(organization) = db.orgs.get_mut(org) {
                    if let Some(bucket) = organization.buckets.get_mut(&key) {
                        bucket.remove(&stored_id);
                    }
                }
            });
            return StatusCode::NO_CONTENT.into_response();
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
    if let Some(object) = item.as_object_mut() {
        object.entry("tags".to_string()).or_insert_with(|| json!([]));
    }
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

fn project_build_tags(state: &AppState, method: &Method, org: &str, project: &str, rest: &[String]) -> Response {
    if rest.len() != 2 {
        if method == &Method::DELETE {
            return fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée");
        }
        return fail(StatusCode::NOT_FOUND, "étiquette introuvable");
    }
    if method == &Method::GET {
        let mut tags = Vec::new();
        for build in list(state, org, project, "build/builds") {
            for tag in tag_names(&build) {
                if !tags.iter().any(|existing: &String| existing == &tag) {
                    tags.push(tag);
                }
            }
        }
        tags.sort();
        return Json(tags).into_response();
    }
    fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée")
}

fn build_tags(
    state: &AppState,
    method: &Method,
    org: &str,
    project: &str,
    id: &str,
    rest: &[String],
    body: &[u8],
) -> Response {
    let Some(current) = get_item(state, org, project, "build/builds", id) else {
        return fail(StatusCode::NOT_FOUND, "build introuvable");
    };
    let mut build = promote_build(state, org, project, current);
    if rest.len() == 4 {
        if method == &Method::GET {
            return Json(tag_names(&build)).into_response();
        }
        if method == &Method::POST {
            return add_build_tags(state, org, project, &mut build, body);
        }
        return fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée");
    }
    if rest.len() != 5 {
        if method == &Method::DELETE {
            return fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée");
        }
        return fail(StatusCode::NOT_FOUND, "étiquette introuvable");
    }
    let tag = pct_decode(&rest[4]).trim().to_string();
    if tag.is_empty() {
        return fail(StatusCode::BAD_REQUEST, "étiquette absente");
    }
    if method == &Method::PUT {
        let mut tags = tag_names(&build);
        if !tags.iter().any(|existing| existing == &tag) {
            tags.push(tag);
        }
        return Json(save_build_tags(state, org, project, &mut build, tags)).into_response();
    }
    if method == &Method::DELETE {
        let mut tags = tag_names(&build);
        let Some(index) = tags.iter().position(|existing| existing == &tag) else {
            return fail(StatusCode::NOT_FOUND, "étiquette introuvable");
        };
        tags.remove(index);
        return Json(save_build_tags(state, org, project, &mut build, tags)).into_response();
    }
    if method == &Method::GET {
        if tag_names(&build).iter().any(|existing| existing == &tag) {
            return Json(json!(tag)).into_response();
        }
        return fail(StatusCode::NOT_FOUND, "étiquette introuvable");
    }
    fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée")
}

fn add_build_tags(state: &AppState, org: &str, project: &str, build: &mut Value, body: &[u8]) -> Response {
    let Some(payload) = parse_json(body) else {
        return fail(StatusCode::BAD_REQUEST, "JSON illisible");
    };
    let Some(rows) = payload.as_array() else {
        return fail(StatusCode::BAD_REQUEST, "étiquette absente");
    };
    let mut tags = tag_names(build);
    for row in rows {
        let Some(name) = row.as_str().map(str::trim).filter(|name| !name.is_empty()) else {
            return fail(StatusCode::BAD_REQUEST, "étiquette absente");
        };
        if !tags.iter().any(|existing| existing == name) {
            tags.push(name.to_string());
        }
    }
    Json(save_build_tags(state, org, project, build, tags)).into_response()
}

fn save_build_tags(state: &AppState, org: &str, project: &str, build: &mut Value, tags: Vec<String>) -> Value {
    if let Some(object) = build.as_object_mut() {
        object.insert("tags".into(), json!(tags));
    }
    put_item(state, org, project, "build/builds", build.clone());
    json!(tag_names(build))
}

fn tag_names(build: &Value) -> Vec<String> {
    build
        .get("tags")
        .and_then(|value| value.as_array())
        .map(|rows| rows.iter().filter_map(|row| row.as_str().map(str::to_string)).collect())
        .unwrap_or_default()
}

fn build_retention(
    state: &AppState,
    method: &Method,
    org: &str,
    project: &str,
    rest: &[String],
    query: &str,
    body: &[u8],
) -> Response {
    if rest.get(2).is_none_or(|value| !value.eq_ignore_ascii_case("leases")) {
        return fail(StatusCode::NOT_FOUND, "conservation introuvable");
    }
    if rest.len() == 3 {
        if method == &Method::GET {
            let mut leases = list(state, org, project, "build/retention/leases")
                .into_iter()
                .filter(|item| lease_matches(item, query))
                .collect::<Vec<_>>();
            leases.sort_by_key(|item| item.get("leaseId").and_then(json_i64).unwrap_or(0));
            return Json(leases).into_response();
        }
        if method == &Method::POST {
            return add_retention_leases(state, org, project, body);
        }
        if method == &Method::DELETE {
            let Some(ids) = lease_ids_from_query(query) else {
                return fail(StatusCode::BAD_REQUEST, "conservation absente");
            };
            return delete_retention_leases(state, org, project, &ids);
        }
        return fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée");
    }
    if rest.len() == 4 {
        let id = pct_decode(&rest[3]).trim().to_string();
        if id.is_empty() {
            return fail(StatusCode::BAD_REQUEST, "conservation absente");
        }
        if method == &Method::GET {
            return match get_item(state, org, project, "build/retention/leases", &id) {
                Some(item) => Json(item).into_response(),
                None => fail(StatusCode::NOT_FOUND, "conservation introuvable"),
            };
        }
        if method == &Method::DELETE {
            return delete_retention_leases(state, org, project, &[id]);
        }
        return fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée");
    }
    if method == &Method::DELETE {
        return fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée");
    }
    fail(StatusCode::NOT_FOUND, "conservation introuvable")
}

fn add_retention_leases(state: &AppState, org: &str, project: &str, body: &[u8]) -> Response {
    let Some(payload) = parse_json(body) else {
        return fail(StatusCode::BAD_REQUEST, "JSON illisible");
    };
    let Some(rows) = payload.as_array() else {
        return fail(StatusCode::BAD_REQUEST, "conservation absente");
    };
    if rows.is_empty() {
        return fail(StatusCode::BAD_REQUEST, "conservation absente");
    }
    let mut drafts = Vec::new();
    for row in rows {
        let Some(object) = row.as_object() else {
            return fail(StatusCode::BAD_REQUEST, "conservation absente");
        };
        let Some(run_id) = object.get("runId").and_then(json_i64) else {
            return fail(StatusCode::BAD_REQUEST, "conservation absente");
        };
        let Some(definition_id) = object.get("definitionId").and_then(json_i64) else {
            return fail(StatusCode::BAD_REQUEST, "conservation absente");
        };
        let protect = object.get("protectPipeline").and_then(|value| value.as_bool()).unwrap_or(false);
        let days = match object.get("daysValid").and_then(json_i64) {
            Some(days) if days < 0 => return fail(StatusCode::BAD_REQUEST, "durée absente"),
            Some(days) => days,
            None if protect => 0,
            None => return fail(StatusCode::BAD_REQUEST, "durée absente"),
        };
        let owner = object
            .get("ownerId")
            .and_then(|value| value.as_str())
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("azurz")
            .to_string();
        let Some(build) = get_item(state, org, project, "build/builds", &run_id.to_string()) else {
            return fail(StatusCode::NOT_FOUND, "build introuvable");
        };
        if get_item(state, org, project, "build/definitions", &definition_id.to_string()).is_none() {
            return fail(StatusCode::NOT_FOUND, "définition introuvable");
        }
        let build_definition = build.get("definition").and_then(|value| value.get("id")).and_then(json_i64);
        if build_definition != Some(definition_id) {
            return fail(StatusCode::BAD_REQUEST, "définition différente");
        }
        drafts.push((run_id, definition_id, owner, days, protect));
    }
    let created = state.write(|db| {
        db.ensure_project(org, project);
        let stored = db.project_mut(org, project)?;
        let bucket = stored
            .buckets
            .entry("build/retention/leases".into())
            .or_insert_with(Bucket::empty);
        let mut next = bucket
            .items
            .values()
            .filter_map(|item| item.get("leaseId").and_then(json_i64))
            .max()
            .unwrap_or(0);
        let mut out = Vec::new();
        for (run_id, definition_id, owner, days, protect) in drafts {
            next += 1;
            let valid_until = if protect {
                Value::Null
            } else {
                json!(lab_offset_date(days))
            };
            let item = json!({
                "id": next,
                "leaseId": next,
                "definitionId": definition_id,
                "runId": run_id,
                "ownerId": owner,
                "daysValid": days,
                "protectPipeline": protect,
                "createdOn": "2026-10-29T00:00:00Z",
                "validUntil": valid_until
            });
            bucket.put(item.clone());
            out.push(item);
        }
        Some(out)
    });
    match created {
        Some(items) => Json(items).into_response(),
        None => fail(StatusCode::NOT_FOUND, "projet inconnu"),
    }
}

fn delete_retention_leases(state: &AppState, org: &str, project: &str, ids: &[String]) -> Response {
    let mut unique = Vec::new();
    for id in ids {
        let id = id.trim().to_string();
        if id.is_empty() {
            return fail(StatusCode::BAD_REQUEST, "conservation absente");
        }
        if unique.iter().any(|existing: &String| existing == &id) {
            continue;
        }
        unique.push(id);
    }
    if unique.is_empty() {
        return fail(StatusCode::BAD_REQUEST, "conservation absente");
    }
    if unique
        .iter()
        .any(|id| get_item(state, org, project, "build/retention/leases", id).is_none())
    {
        return fail(StatusCode::NOT_FOUND, "conservation introuvable");
    }
    for id in &unique {
        delete_item(state, org, project, "build/retention/leases", id);
    }
    StatusCode::NO_CONTENT.into_response()
}

fn lease_ids_from_query(query: &str) -> Option<Vec<String>> {
    let raw = query_param(query, "ids")?;
    let decoded = pct_decode(raw);
    Some(
        decoded
            .split(',')
            .map(|part| part.trim().to_string())
            .filter(|part| !part.is_empty())
            .collect(),
    )
}

fn lease_matches(item: &Value, query: &str) -> bool {
    numeric_query(query, "runId", item.get("runId").and_then(json_i64))
        && numeric_query(query, "definitionId", item.get("definitionId").and_then(json_i64))
        && owner_query(query, text_of(item, "ownerId").unwrap_or_default())
}

fn numeric_query(query: &str, key: &str, actual: Option<i64>) -> bool {
    let Some(raw) = query_param(query, key) else {
        return true;
    };
    let decoded = pct_decode(raw);
    let trimmed = decoded.trim();
    if trimmed.is_empty() {
        return true;
    }
    trimmed.parse::<i64>().ok() == actual
}

fn owner_query(query: &str, actual: String) -> bool {
    let Some(raw) = query_param(query, "ownerId") else {
        return true;
    };
    let decoded = pct_decode(raw);
    let trimmed = decoded.trim();
    trimmed.is_empty() || trimmed == actual
}

fn json_i64(value: &Value) -> Option<i64> {
    value.as_i64().or_else(|| value.as_u64().and_then(|number| i64::try_from(number).ok()))
}

fn lab_offset_date(days: i64) -> String {
    let mut year = 2026i32;
    let mut month = 10u32;
    let mut day = 29i64 + days;
    loop {
        let length = match month {
            1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
            4 | 6 | 9 | 11 => 30,
            _ => {
                if year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) {
                    29
                } else {
                    28
                }
            }
        };
        if day <= i64::from(length) {
            break;
        }
        day -= i64::from(length);
        month += 1;
        if month > 12 {
            month = 1;
            year += 1;
        }
    }
    format!("{year:04}-{month:02}-{day:02}T00:00:00Z")
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
