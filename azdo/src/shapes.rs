use axum::http::{Method, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::{json, Value};

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
        ("pipelines", "pipelines") => pipelines(state, method, org, project, rest, body),
        ("pipelines", "runs") => pipeline_runs(state, method, org, project, rest),
        ("testplan", "plans") | ("test", "plans") => test_plans(state, method, org, project, rest, body),
        ("test", "runs") => test_runs(state, method, org, project, rest, body),
        ("wiki", "wikis") => wikis(state, method, org, project, rest, query, body),
        ("packaging", "feeds") => feeds(state, method, org, project, rest, body),
        ("release", "definitions") => simple_named(state, method, org, project, "release/definitions", body),
        ("release", "releases") => simple_named(state, method, org, project, "release/releases", body),
        _ => return None,
    };
    Some(response)
}

pub fn org_dispatch(state: &AppState, method: &Method, org: &str, rest: &[String], body: &[u8]) -> Option<Response> {
    let area = rest.first()?.to_ascii_lowercase();
    let resource = rest.get(1)?.to_ascii_lowercase();
    let response = match (area.as_str(), resource.as_str()) {
        ("hooks", "subscriptions") => subscriptions(state, method, org, rest, body),
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
        "commits" => commits(state, method, org, project, repo_id, body),
        "pushes" => pushes(state, method, org, project, repo_id, body),
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
        "commits" => commits(state, method, org, project, &repo_id, body),
        "pushes" => pushes(state, method, org, project, &repo_id, body),
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

fn commits(state: &AppState, method: &Method, org: &str, project: &str, repo_id: &str, body: &[u8]) -> Response {
    let key = format!("git/repositories/{repo_id}/commits");
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

fn pushes(state: &AppState, method: &Method, org: &str, project: &str, repo_id: &str, body: &[u8]) -> Response {
    let key = format!("git/repositories/{repo_id}/pushes");
    if method == Method::GET {
        return Json(page(list(state, org, project, &key))).into_response();
    }
    if method != Method::POST {
        return fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée");
    }
    let payload = parse_json(body).unwrap_or(json!({}));
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
    Json(item).into_response()
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
    if method == Method::GET && rest.len() <= 4 {
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
                Some(item) => Json(item).into_response(),
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
        let item = json!({
            "id": next,
            "buildNumber": format!("20261029.{next}"),
            "status": "notStarted",
            "definition": {"id": definition_id, "name": definition_name},
            "project": {"name": project},
            "sourceBranch": text_of(&payload, "sourceBranch").unwrap_or_else(|| "refs/heads/main".into()),
            "sourceVersion": object_id(&format!("build-{next}")),
            "queueTime": "2026-10-29T00:00:00Z",
            "url": format!("/{org}/{project}/_apis/build/builds/{next}"),
            "requestedBy": {"displayName": "azurz"}
        });
        put_item(state, org, project, key, item.clone());
        return Json(item).into_response();
    }
    let id = rest.get(2).map(String::as_str).unwrap_or("");
    if rest.len() >= 4 && rest[3].eq_ignore_ascii_case("timeline") && method == Method::GET {
        return Json(json!({
            "records": [{
                "id": "1",
                "parentId": null,
                "type": "Task",
                "name": "Build",
                "state": "completed",
                "result": "succeeded"
            }]
        }))
        .into_response();
    }
    if rest.len() >= 4 && rest[3].eq_ignore_ascii_case("logs") && method == Method::GET {
        return Json(page(vec![json!({"id": 1, "type": "Container", "lineCount": 1, "url": format!("/{org}/{project}/_apis/build/builds/{id}/logs/1")})])).into_response();
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
        let suites = format!("testplan/plans/{plan_id}/suites");
        if method == Method::GET {
            return Json(page(list(state, org, project, &suites))).into_response();
        }
        if method == Method::POST {
            let payload = parse_json(body).unwrap_or(json!({}));
            let name = text_of(&payload, "name").unwrap_or_else(|| "Suite".into());
            let next = list(state, org, project, &suites).len() as i64 + 1;
            let item = json!({"id": next, "name": name, "plan": {"id": plan_id.parse::<i64>().unwrap_or(next)}, "suiteType": "staticTestSuite"});
            put_item(state, org, project, &suites, item.clone());
            return Json(item).into_response();
        }
    }
    fail(StatusCode::NOT_FOUND, "plan introuvable")
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

fn simple_named(state: &AppState, method: &Method, org: &str, project: &str, key: &str, body: &[u8]) -> Response {
    if method == Method::GET {
        return Json(page(list(state, org, project, key))).into_response();
    }
    if method != Method::POST {
        return fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée");
    }
    let payload = parse_json(body).unwrap_or(json!({}));
    let name = text_of(&payload, "name").unwrap_or_else(|| "release".into());
    let next = list(state, org, project, key).len() as i64 + 1;
    let item = json!({
        "id": next,
        "name": name,
        "path": "\\",
        "status": "active",
        "projectReference": {"name": project}
    });
    put_item(state, org, project, key, item.clone());
    Json(item).into_response()
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
