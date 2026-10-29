use axum::http::{Method, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::{json, Map, Value};

use crate::http::{fail, parse_json, pct_decode};
use crate::AppState;

pub fn dispatch(
    state: &AppState,
    method: &Method,
    org: &str,
    project: &str,
    team: Option<&str>,
    rest: &[String],
    body: &[u8],
) -> Option<Response> {
    let area = rest.first()?.to_ascii_lowercase();
    let resource = rest.get(1).map(|value| value.to_ascii_lowercase()).unwrap_or_default();
    let response = match (area.as_str(), resource.as_str()) {
        ("work", "boards") => boards(state, method, org, project, team, rest, body),
        ("work", "backlogs") => backlogs(state, method, org, project, team, rest),
        ("work", "plans") => delivery_plans(state, method, org, project, rest, body),
        ("work", "processes") => {
            if rest.iter().any(|part| part.eq_ignore_ascii_case("lists")) {
                picklists(state, method, org, project, rest, body)
            } else {
                work_processes(state, method, org, Some(project), rest, body)
            }
        }
        ("work", "teamsettings") => team_settings(state, method, org, project, team, rest, body),
        ("dashboard", "dashboards") => dashboards(state, method, org, project, team, rest, body),
        ("serviceendpoint", "types") if rest.len() == 2 => endpoint_types(method),
        ("serviceendpoint", "endpoints") => endpoints(state, method, org, project, rest, body),
        _ => return None,
    };
    Some(response)
}

fn boards(
    state: &AppState,
    method: &Method,
    org: &str,
    project: &str,
    team: Option<&str>,
    rest: &[String],
    body: &[u8],
) -> Response {
    let team_name = team.map(str::to_string).unwrap_or_else(|| format!("{project} Team"));
    if rest.len() >= 4 && rest[3].eq_ignore_ascii_case("columns") {
        let board = &rest[2];
        if method == Method::GET {
            return Json(page(load_columns(state, org, project, &team_name, board))).into_response();
        }
        if method == Method::PUT {
            return put_columns(state, org, project, &team_name, board, body);
        }
        return fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée");
    }
    if rest.len() >= 4 && rest[3].eq_ignore_ascii_case("rows") {
        let board = &rest[2];
        if method == Method::GET {
            return Json(page(load_rows(state, org, project, &team_name, board))).into_response();
        }
        if method == Method::PUT {
            return put_rows(state, org, project, &team_name, board, body);
        }
        return fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée");
    }
    if method != Method::GET {
        return fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée");
    }
    if rest.len() >= 3 {
        let columns = load_columns(state, org, project, &team_name, &rest[2]);
        let rows = load_rows(state, org, project, &team_name, &rest[2]);
        return Json(board_document(org, project, &team_name, &rest[2], columns, rows)).into_response();
    }
    let columns = load_columns(state, org, project, &team_name, "Stories");
    let rows = load_rows(state, org, project, &team_name, "Stories");
    Json(page(vec![board_document(org, project, &team_name, "Stories", columns, rows)])).into_response()
}

fn backlogs(state: &AppState, method: &Method, org: &str, project: &str, team: Option<&str>, rest: &[String]) -> Response {
    if method != Method::GET {
        return fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée");
    }
    if rest.len() >= 4 && rest[3].eq_ignore_ascii_case("workitems") {
        let backlog = &rest[2];
        let rows = state.read(|db| {
            db.project(org, project)
                .map(|stored| {
                    stored
                        .work_items
                        .values()
                        .filter(|item| backlog_matches(backlog, item))
                        .filter_map(|item| item.get("id").cloned())
                        .map(|id| json!({"target": {"id": id}, "id": id}))
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default()
        });
        return Json(page(rows)).into_response();
    }
    let _ = team;
    Json(page(vec![
        json!({"id": "Microsoft.EpicCategory", "name": "Epics", "rank": 1, "workItemTypes": [{"name": "Epic"}]}),
        json!({"id": "Microsoft.FeatureCategory", "name": "Features", "rank": 2, "workItemTypes": [{"name": "Feature"}]}),
        json!({"id": "Microsoft.RequirementCategory", "name": "Backlog items", "rank": 3, "url": format!("/{org}/{project}/_apis/work/backlogs/Microsoft.RequirementCategory"), "workItemTypes": [{"name": "User Story"}, {"name": "Bug"}]}),
        json!({"id": "Microsoft.TaskCategory", "name": "Tasks", "rank": 4, "workItemTypes": [{"name": "Task"}]}),
    ]))
    .into_response()
}

fn team_settings(
    state: &AppState,
    method: &Method,
    org: &str,
    project: &str,
    team: Option<&str>,
    rest: &[String],
    body: &[u8],
) -> Response {
    if rest.len() >= 3 && rest[2].eq_ignore_ascii_case("iterations") {
        if rest.len() >= 5 {
            let iteration = &rest[3];
            if rest[4].eq_ignore_ascii_case("capacities") {
                return capacities(state, method, org, project, iteration, rest, body);
            }
            if rest[4].eq_ignore_ascii_case("teamdaysoff") {
                return team_days_off(state, method, org, project, iteration, body);
            }
        }
        if rest.len() == 3 && method == Method::POST {
            return add_team_iteration(state, org, project, body);
        }
        if rest.len() == 4 && method == Method::DELETE {
            return remove_team_iteration(state, org, project, &rest[3]);
        }
        if method != Method::GET {
            return fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée");
        }
        let value = team_iterations(state, org, project);
        if rest.len() >= 4 {
            let id = &rest[3];
            return match value.into_iter().find(|item| iteration_matches(item, id)) {
                Some(item) => Json(dress_iteration(item)).into_response(),
                None => fail(StatusCode::NOT_FOUND, "itération introuvable"),
            };
        }
        return Json(page(value.into_iter().map(dress_iteration).collect())).into_response();
    }
    let Some(team_id) = resolve_team_id(state, org, project, team) else {
        return fail(StatusCode::NOT_FOUND, "équipe introuvable");
    };
    if method == Method::GET {
        return Json(team_settings_document(state, org, project, &team_id)).into_response();
    }
    if method == Method::PATCH || method == Method::PUT {
        return patch_team_settings(state, org, project, &team_id, body);
    }
    fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée")
}

pub fn teams(state: &AppState, method: &Method, org: &str, project_key: &str, tail: &[String], body: &[u8]) -> Response {
    let Some(project_name) = state.read(|db| db.project_name(org, project_key)) else {
        return fail(StatusCode::NOT_FOUND, "projet inconnu");
    };
    state.write(|db| {
        if let Some(stored) = db.project_mut(org, project_key) {
            stored.seed_missing(&project_name);
        }
    });
    let project_id = state
        .read(|db| db.project(org, project_key).map(|stored| stored.id.clone()))
        .unwrap_or_default();
    if tail.is_empty() {
        if method == Method::GET {
            let value = team_rows(state, org, project_key)
                .into_iter()
                .map(|item| dress_team(org, &project_name, &project_id, item))
                .collect();
            return Json(page(value)).into_response();
        }
        if method == Method::POST {
            return create_team(state, org, project_key, &project_name, &project_id, body);
        }
        return fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée");
    }
    let Some(team) = find_team(state, org, project_key, &tail[0]) else {
        return fail(StatusCode::NOT_FOUND, "équipe introuvable");
    };
    let team_id = item_id(&team);
    if tail.get(1).is_some_and(|part| part.eq_ignore_ascii_case("members")) {
        return team_members(state, method, org, project_key, &team_id, &tail[2..], body);
    }
    if tail.len() != 1 {
        return fail(StatusCode::NOT_FOUND, "équipe introuvable");
    }
    if method == Method::GET {
        return Json(dress_team(org, &project_name, &project_id, team)).into_response();
    }
    if method == Method::PATCH || method == Method::PUT {
        return patch_team(state, org, project_key, &project_name, &project_id, &team_id, body);
    }
    if method == Method::DELETE {
        if team_id == "1" {
            return fail(StatusCode::BAD_REQUEST, "équipe par défaut");
        }
        let removed = state.write(|db| {
            db.project_mut(org, project_key)
                .and_then(|stored| stored.buckets.get_mut("core/teams"))
                .is_some_and(|bucket| bucket.remove(&team_id))
        });
        return if removed {
            StatusCode::NO_CONTENT.into_response()
        } else {
            fail(StatusCode::NOT_FOUND, "équipe introuvable")
        };
    }
    fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée")
}

fn create_team(state: &AppState, org: &str, project_key: &str, project_name: &str, project_id: &str, body: &[u8]) -> Response {
    let payload = parse_json(body).unwrap_or(json!({}));
    let name = item_text(&payload, "name").unwrap_or_default();
    if name.trim().is_empty() {
        return fail(StatusCode::BAD_REQUEST, "nom absent");
    }
    if team_rows(state, org, project_key).iter().any(|item| item_text(item, "name").is_some_and(|current| current.eq_ignore_ascii_case(&name))) {
        return fail(StatusCode::BAD_REQUEST, "équipe existante");
    }
    let description = item_text(&payload, "description").unwrap_or_default();
    let created = state.write(|db| {
        let stored = db.project_mut(org, project_key)?;
        let id = format!("team-{}", stored.list_bucket("core/teams").len() + 1);
        let item = json!({"id": id, "name": name, "description": description});
        stored.put_bucket("core/teams", item.clone());
        stored.put_bucket(&format!("core/teams/{id}/members"), member_item("azurz", "azurz", true));
        Some(item)
    });
    match created {
        Some(item) => Json(dress_team(org, project_name, project_id, item)).into_response(),
        None => fail(StatusCode::NOT_FOUND, "projet inconnu"),
    }
}

fn patch_team(
    state: &AppState,
    org: &str,
    project_key: &str,
    project_name: &str,
    project_id: &str,
    team_id: &str,
    body: &[u8],
) -> Response {
    let payload = parse_json(body).unwrap_or(json!({}));
    if let Some(name) = payload.get("name").and_then(|value| value.as_str()) {
        if name.trim().is_empty() {
            return fail(StatusCode::BAD_REQUEST, "nom absent");
        }
        if team_rows(state, org, project_key).iter().any(|item| {
            item_id(item) != team_id && item_text(item, "name").is_some_and(|current| current.eq_ignore_ascii_case(name))
        }) {
            return fail(StatusCode::BAD_REQUEST, "équipe existante");
        }
    }
    let updated = state.write(|db| {
        let stored = db.project_mut(org, project_key)?;
        let mut item = stored.get_bucket("core/teams", team_id)?;
        if let Some(object) = item.as_object_mut() {
            if let Some(name) = payload.get("name").and_then(|value| value.as_str()) {
                object.insert("name".into(), json!(name));
            }
            if let Some(description) = payload.get("description").and_then(|value| value.as_str()) {
                object.insert("description".into(), json!(description));
            }
        }
        Some(stored.put_bucket("core/teams", item))
    });
    match updated {
        Some(item) => Json(dress_team(org, project_name, project_id, item)).into_response(),
        None => fail(StatusCode::NOT_FOUND, "équipe introuvable"),
    }
}

fn team_members(state: &AppState, method: &Method, org: &str, project_key: &str, team_id: &str, rest: &[String], body: &[u8]) -> Response {
    let key = format!("core/teams/{team_id}/members");
    if rest.len() > 1 {
        return fail(StatusCode::NOT_FOUND, "membre introuvable");
    }
    if let Some(member_id) = rest.first() {
        if method != Method::DELETE {
            return fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée");
        }
        return remove_team_member(state, org, project_key, &key, member_id);
    }
    if method == Method::GET {
        let value = state.read(|db| db.project(org, project_key).map(|stored| stored.list_bucket(&key)).unwrap_or_default());
        return Json(page(value)).into_response();
    }
    if method != Method::POST && method != Method::PUT {
        return fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée");
    }
    let payload = parse_json(body).unwrap_or(json!({}));
    let unique = item_text(&payload, "uniqueName").unwrap_or_default();
    if unique.is_empty() {
        return fail(StatusCode::BAD_REQUEST, "identité absente");
    }
    let display = item_text(&payload, "displayName").unwrap_or_else(|| unique.clone());
    let admin = payload.get("isTeamAdmin").and_then(|value| value.as_bool()).unwrap_or(false);
    let saved = state.write(|db| {
        let stored = db.project_mut(org, project_key)?;
        Some(stored.put_bucket(&key, member_item(&display, &unique, admin)))
    });
    match saved {
        Some(item) => Json(item).into_response(),
        None => fail(StatusCode::NOT_FOUND, "projet inconnu"),
    }
}

fn remove_team_member(state: &AppState, org: &str, project_key: &str, key: &str, member_id: &str) -> Response {
    let stored_id = state.read(|db| {
        db.project(org, project_key)
            .map(|stored| stored.list_bucket(key))
            .unwrap_or_default()
            .into_iter()
            .find(|item| member_matches(item, member_id))
            .map(|item| item_id(&item))
            .filter(|id| !id.is_empty())
    });
    let Some(stored_id) = stored_id else {
        return fail(StatusCode::NOT_FOUND, "membre introuvable");
    };
    let removed = state.write(|db| {
        db.project_mut(org, project_key)
            .and_then(|stored| stored.buckets.get_mut(key))
            .is_some_and(|bucket| bucket.remove(&stored_id))
    });
    if removed {
        StatusCode::NO_CONTENT.into_response()
    } else {
        fail(StatusCode::NOT_FOUND, "membre introuvable")
    }
}

fn member_matches(item: &Value, token: &str) -> bool {
    item_id(item).eq_ignore_ascii_case(token)
        || item
            .get("identity")
            .and_then(|identity| identity.get("uniqueName"))
            .and_then(|value| value.as_str())
            .is_some_and(|name| name.eq_ignore_ascii_case(token))
}

fn resolve_team_id(state: &AppState, org: &str, project_key: &str, team: Option<&str>) -> Option<String> {
    let project_name = state.read(|db| db.project_name(org, project_key))?;
    state.write(|db| {
        if let Some(stored) = db.project_mut(org, project_key) {
            stored.seed_missing(&project_name);
        }
    });
    let token = team.unwrap_or("1");
    find_team(state, org, project_key, token).map(|item| item_id(&item))
}

fn team_settings_document(state: &AppState, org: &str, project: &str, team_id: &str) -> Value {
    let key = settings_key(team_id);
    state
        .read(|db| db.project(org, project).and_then(|stored| stored.get_bucket(&key, team_id)))
        .map(settings_body)
        .unwrap_or_else(|| default_team_settings(project))
}

fn patch_team_settings(state: &AppState, org: &str, project: &str, team_id: &str, body: &[u8]) -> Response {
    let payload = parse_json(body).unwrap_or(json!({}));
    let mut current = team_settings_document(state, org, project, team_id);
    if let Some(behavior) = payload.get("bugsBehavior").and_then(|value| value.as_str()) {
        if !matches!(behavior, "off" | "asTasks" | "asRequirements") {
            return fail(StatusCode::BAD_REQUEST, "comportement des bugs inconnu");
        }
        if let Some(object) = current.as_object_mut() {
            object.insert("bugsBehavior".into(), json!(behavior));
        }
    }
    if let Some(days) = payload.get("workingDays").and_then(|value| value.as_array()) {
        let mut normalized = Vec::new();
        for day in days {
            let Some(name) = day.as_str().map(|value| value.to_ascii_lowercase()) else {
                return fail(StatusCode::BAD_REQUEST, "jour ouvré inconnu");
            };
            if !matches!(name.as_str(), "monday" | "tuesday" | "wednesday" | "thursday" | "friday" | "saturday" | "sunday") {
                return fail(StatusCode::BAD_REQUEST, "jour ouvré inconnu");
            }
            normalized.push(json!(name));
        }
        if let Some(object) = current.as_object_mut() {
            object.insert("workingDays".into(), json!(normalized));
        }
    }
    if let Some(iteration) = payload.get("backlogIteration").filter(|value| value.is_object()) {
        if let Some(object) = current.as_object_mut() {
            object.insert("backlogIteration".into(), iteration.clone());
        }
    }
    if let Some(object) = current.as_object_mut() {
        object.insert("id".into(), json!(team_id));
    }
    let key = settings_key(team_id);
    state.write(|db| {
        if let Some(stored) = db.project_mut(org, project) {
            stored.put_bucket(&key, current.clone());
        }
    });
    Json(settings_body(current)).into_response()
}

fn default_team_settings(project: &str) -> Value {
    json!({
        "backlogIteration": {"name": project, "path": format!("{project}\\")},
        "defaultIteration": {"name": "Sprint 1"},
        "defaultIterationMacro": "@currentIteration",
        "bugsBehavior": "off",
        "workingDays": ["monday", "tuesday", "wednesday", "thursday", "friday"]
    })
}

fn settings_key(team_id: &str) -> String {
    format!("work/teamsettings/document/{team_id}")
}

fn settings_body(mut item: Value) -> Value {
    if let Some(object) = item.as_object_mut() {
        object.remove("id");
    }
    item
}

fn team_rows(state: &AppState, org: &str, project: &str) -> Vec<Value> {
    state.read(|db| db.project(org, project).map(|stored| stored.list_bucket("core/teams")).unwrap_or_default())
}

fn find_team(state: &AppState, org: &str, project: &str, token: &str) -> Option<Value> {
    team_rows(state, org, project).into_iter().find(|item| item_id(item) == token || item_text(item, "name").as_deref() == Some(token))
}

fn dress_team(org: &str, project_name: &str, project_id: &str, mut item: Value) -> Value {
    if let Some(object) = item.as_object_mut() {
        let id = object.get("id").map(value_text).unwrap_or_default();
        object.insert("projectName".into(), json!(project_name));
        object.insert("projectId".into(), json!(project_id));
        object.insert("url".into(), json!(format!("/{org}/_apis/projects/{project_name}/teams/{id}")));
        object.entry("description").or_insert(json!(""));
    }
    item
}

fn member_item(display: &str, unique: &str, admin: bool) -> Value {
    json!({
        "id": unique,
        "isTeamAdmin": admin,
        "identity": {"id": unique, "displayName": display, "uniqueName": unique}
    })
}

fn capacities(state: &AppState, method: &Method, org: &str, project: &str, iteration: &str, rest: &[String], body: &[u8]) -> Response {
    if iteration_by_id(state, org, project, iteration).is_none() {
        return fail(StatusCode::NOT_FOUND, "itération introuvable");
    }
    let key = format!("work/teamsettings/iterations/{iteration}/capacities");
    let member = rest.get(5).map(String::as_str);
    if member.is_none() && method == Method::GET {
        let value = state.read(|db| db.project(org, project).map(|stored| stored.list_bucket(&key)).unwrap_or_default());
        return Json(page(value)).into_response();
    }
    if member.is_none() && method == Method::PUT {
        let payload = parse_json(body).unwrap_or(json!([]));
        let rows = payload
            .get("value")
            .and_then(|value| value.as_array())
            .cloned()
            .or_else(|| payload.as_array().cloned())
            .unwrap_or_default();
        let items: Vec<Value> = rows.iter().map(|row| capacity_item(org, project, iteration, row)).collect();
        state.write(|db| {
            db.ensure_project(org, project);
            let Some(stored) = db.project_mut(org, project) else {
                return;
            };
            if let Some(bucket) = stored.buckets.get_mut(&key) {
                bucket.items.clear();
            }
            for item in items {
                stored.put_bucket(&key, item);
            }
        });
        let value = state.read(|db| db.project(org, project).map(|stored| stored.list_bucket(&key)).unwrap_or_default());
        return Json(page(value)).into_response();
    }
    let Some(member) = member else {
        return fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée");
    };
    if method == Method::GET {
        return match state.read(|db| db.project(org, project).and_then(|stored| stored.get_bucket(&key, member))) {
            Some(item) => Json(item).into_response(),
            None => fail(StatusCode::NOT_FOUND, "capacité introuvable"),
        };
    }
    if method == Method::PUT || method == Method::PATCH {
        let payload = parse_json(body).unwrap_or(json!({}));
        let current = state.read(|db| db.project(org, project).and_then(|stored| stored.get_bucket(&key, member)));
        let mut item = if method == Method::PATCH {
            current.unwrap_or_else(|| capacity_item(org, project, iteration, &json!({"teamMember": {"displayName": member, "uniqueName": member}})))
        } else {
            let mut seeded = payload.clone();
            if seeded.get("teamMember").is_none() {
                seeded["teamMember"] = json!({"displayName": member, "uniqueName": member});
            }
            capacity_item(org, project, iteration, &seeded)
        };
        if method == Method::PATCH {
            if let Some(object) = item.as_object_mut() {
                if let Some(activities) = payload.get("activities") {
                    object.insert("activities".into(), activities.clone());
                }
                if let Some(days_off) = payload.get("daysOff") {
                    object.insert("daysOff".into(), days_off.clone());
                }
                if let Some(team_member) = payload.get("teamMember") {
                    object.insert("teamMember".into(), team_member.clone());
                }
            }
        }
        if let Some(object) = item.as_object_mut() {
            object.insert("id".into(), json!(member));
        }
        state.write(|db| {
            db.ensure_project(org, project);
            if let Some(stored) = db.project_mut(org, project) {
                stored.put_bucket(&key, item.clone());
            }
        });
        return Json(item).into_response();
    }
    fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée")
}

fn team_days_off(state: &AppState, method: &Method, org: &str, project: &str, iteration: &str, body: &[u8]) -> Response {
    if iteration_by_id(state, org, project, iteration).is_none() {
        return fail(StatusCode::NOT_FOUND, "itération introuvable");
    }
    let key = format!("work/teamsettings/iterations/{iteration}/teamdaysoff");
    if method == Method::GET {
        let stored = state.read(|db| db.project(org, project).and_then(|stored| stored.get_bucket(&key, "days")));
        let days_off = stored.and_then(|item| item.get("daysOff").cloned()).unwrap_or(json!([]));
        return Json(json!({
            "daysOff": days_off,
            "url": format!("/{org}/{project}/_apis/work/teamsettings/iterations/{iteration}/teamdaysoff")
        }))
        .into_response();
    }
    if method == Method::PUT || method == Method::PATCH {
        let payload = parse_json(body).unwrap_or(json!({}));
        let days_off = payload.get("daysOff").cloned().unwrap_or(json!([]));
        let item = json!({
            "id": "days",
            "daysOff": days_off,
            "url": format!("/{org}/{project}/_apis/work/teamsettings/iterations/{iteration}/teamdaysoff")
        });
        state.write(|db| {
            db.ensure_project(org, project);
            if let Some(stored) = db.project_mut(org, project) {
                stored.put_bucket(&key, item.clone());
            }
        });
        return Json(json!({"daysOff": days_off, "url": item["url"]})).into_response();
    }
    fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée")
}

fn team_iterations(state: &AppState, org: &str, project: &str) -> Vec<Value> {
    state.read(|db| {
        db.project(org, project)
            .map(|stored| stored.list_bucket("work/teamsettings/iterations"))
            .unwrap_or_default()
    })
}

fn classification_iterations(state: &AppState, org: &str, project: &str) -> Vec<Value> {
    let _ = crate::shapes::classification_nodes(
        state,
        &Method::GET,
        org,
        project,
        &["wit".into(), "classificationnodes".into(), "iterations".into()],
        "$depth=1",
        b"",
    );
    state.read(|db| {
        db.project(org, project)
            .map(|stored| stored.list_bucket("wit/classification/iterations"))
            .unwrap_or_default()
    })
}

fn iteration_matches(item: &Value, id: &str) -> bool {
    item_id(item).eq_ignore_ascii_case(id)
        || item_text(item, "name").is_some_and(|name| name.eq_ignore_ascii_case(id))
        || item_text(item, "identifier").is_some_and(|name| name.eq_ignore_ascii_case(id))
}

fn add_team_iteration(state: &AppState, org: &str, project: &str, body: &[u8]) -> Response {
    let Some(payload) = parse_json(body).filter(|value| value.is_object()) else {
        return fail(StatusCode::BAD_REQUEST, "itération illisible");
    };
    let Some(raw_id) = payload.get("id") else {
        return fail(StatusCode::BAD_REQUEST, "itération absente");
    };
    let Some(id) = raw_id.as_str() else {
        return fail(StatusCode::BAD_REQUEST, "itération illisible");
    };
    if id.trim().is_empty() {
        return fail(StatusCode::BAD_REQUEST, "itération absente");
    }
    let nodes = classification_iterations(state, org, project);
    let Some(node) = nodes.iter().find(|node| iteration_matches(node, id.trim())).cloned() else {
        return fail(StatusCode::NOT_FOUND, "itération introuvable");
    };
    if item_text(&node, "identifier").as_deref() == Some("iteration") {
        return fail(StatusCode::BAD_REQUEST, "itération racine");
    }
    let selected = team_iterations(state, org, project);
    let name = item_text(&node, "name").unwrap_or_default();
    let identifier = item_text(&node, "identifier").unwrap_or_default();
    if selected.iter().any(|item| {
        item_text(item, "name").is_some_and(|current| current.eq_ignore_ascii_case(&name))
            || item_text(item, "identifier").is_some_and(|current| !identifier.is_empty() && current.eq_ignore_ascii_case(&identifier))
    }) {
        return fail(StatusCode::BAD_REQUEST, "itération déjà retenue");
    }
    let mut attributes = node.get("attributes").cloned().filter(|value| value.is_object()).unwrap_or_else(|| json!({}));
    let frame = match iteration_time_frame(&attributes) {
        Ok(frame) => frame,
        Err(message) => return fail(StatusCode::BAD_REQUEST, message),
    };
    if let Some(object) = attributes.as_object_mut() {
        object.insert("timeFrame".into(), json!(frame));
    }
    let path = item_text(&node, "path").unwrap_or_else(|| format!("{project}\\{name}"));
    let created = state.write(|db| {
        let stored = db.project_mut(org, project)?;
        let item = json!({
            "name": name,
            "path": path,
            "identifier": identifier,
            "attributes": attributes
        });
        Some(stored.put_bucket("work/teamsettings/iterations", item))
    });
    match created {
        Some(item) => Json(dress_iteration(item)).into_response(),
        None => fail(StatusCode::NOT_FOUND, "projet inconnu"),
    }
}

fn remove_team_iteration(state: &AppState, org: &str, project: &str, id: &str) -> Response {
    let selected = team_iterations(state, org, project);
    let Some(current) = selected.into_iter().find(|item| iteration_matches(item, id)) else {
        return fail(StatusCode::NOT_FOUND, "itération introuvable");
    };
    let dressed = dress_iteration(current.clone());
    let frame = dressed
        .get("attributes")
        .and_then(|value| value.get("timeFrame"))
        .and_then(|value| value.as_str())
        .unwrap_or("current");
    if frame == "current" {
        return fail(StatusCode::BAD_REQUEST, "itération courante");
    }
    let stored_id = item_id(&current);
    let removed = state.write(|db| {
        db.project_mut(org, project)
            .and_then(|stored| stored.buckets.get_mut("work/teamsettings/iterations"))
            .is_some_and(|bucket| bucket.remove(&stored_id))
    });
    if removed {
        StatusCode::NO_CONTENT.into_response()
    } else {
        fail(StatusCode::NOT_FOUND, "itération introuvable")
    }
}

fn iteration_time_frame(attributes: &Value) -> Result<&'static str, &'static str> {
    let start = attributes.get("startDate").filter(|value| !value.is_null());
    let finish = attributes.get("finishDate").filter(|value| !value.is_null());
    match (start, finish) {
        (None, None) => Ok("future"),
        (Some(start), Some(finish)) => {
            let start = normalize_lab_date(start)?;
            let finish = normalize_lab_date(finish)?;
            let now = "2026-10-29T00:00:00Z";
            if start.as_str() > now {
                Ok("future")
            } else if finish.as_str() < now {
                Ok("past")
            } else {
                Ok("current")
            }
        }
        _ => Err("date illisible"),
    }
}

fn normalize_lab_date(value: &Value) -> Result<String, &'static str> {
    let text = value.as_str().ok_or("date illisible")?;
    let bytes = text.as_bytes();
    if bytes.len() < 10 || bytes[4] != b'-' || bytes[7] != b'-' {
        return Err("date illisible");
    }
    if !bytes[..10].iter().all(|byte| byte.is_ascii_digit() || *byte == b'-') {
        return Err("date illisible");
    }
    if text.len() == 10 {
        return Ok(format!("{text}T00:00:00Z"));
    }
    if bytes.get(10) == Some(&b'T') {
        return Ok(text.to_string());
    }
    Err("date illisible")
}

fn iteration_by_id(state: &AppState, org: &str, project: &str, id: &str) -> Option<Value> {
    state.read(|db| {
        db.project(org, project).and_then(|stored| {
            stored
                .list_bucket("work/teamsettings/iterations")
                .into_iter()
                .find(|item| item_id(item) == id || item_text(item, "name").as_deref() == Some(id))
        })
    })
}

fn capacity_item(org: &str, project: &str, iteration: &str, payload: &Value) -> Value {
    let member = payload.get("teamMember").cloned().unwrap_or(json!({}));
    let unique = item_text(&member, "uniqueName")
        .or_else(|| item_text(&member, "displayName"))
        .unwrap_or_else(|| "azurz".into());
    let display = item_text(&member, "displayName").unwrap_or_else(|| unique.clone());
    json!({
        "id": unique,
        "teamMember": {"displayName": display, "uniqueName": unique},
        "activities": payload.get("activities").cloned().unwrap_or(json!([{"name": "Development", "capacityPerDay": 0}])),
        "daysOff": payload.get("daysOff").cloned().unwrap_or(json!([])),
        "url": format!("/{org}/{project}/_apis/work/teamsettings/iterations/{iteration}/capacities/{unique}")
    })
}

fn dress_iteration(mut item: Value) -> Value {
    let attributes = item.as_object_mut().and_then(|object| object.get_mut("attributes")).and_then(|value| value.as_object_mut());
    if let Some(attributes) = attributes {
        if attributes.get("startDate").is_none_or(|value| value.is_null()) {
            attributes.insert("startDate".into(), json!("2026-10-26T00:00:00Z"));
        }
        if attributes.get("finishDate").is_none_or(|value| value.is_null()) {
            attributes.insert("finishDate".into(), json!("2026-11-06T00:00:00Z"));
        }
        attributes.entry("timeFrame").or_insert(json!("current"));
    }
    item
}

fn item_id(item: &Value) -> String {
    item.get("id").map(value_text).unwrap_or_default()
}

fn item_text(item: &Value, key: &str) -> Option<String> {
    item.get(key).and_then(|value| value.as_str()).map(str::to_string)
}

fn value_text(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        Value::Number(number) => number.to_string(),
        _ => String::new(),
    }
}

fn dashboards(
    state: &AppState,
    method: &Method,
    org: &str,
    project: &str,
    team: Option<&str>,
    rest: &[String],
    body: &[u8],
) -> Response {
    let key = "dashboard/dashboards";
    if rest.len() == 2 && method == Method::GET {
        let value = state
            .read(|db| db.project(org, project).map(|stored| stored.list_bucket(key)).unwrap_or_default())
            .into_iter()
            .map(|item| dress_dashboard(state, org, project, item))
            .collect();
        return Json(page(value)).into_response();
    }
    if rest.len() == 2 && method == Method::POST {
        let payload = parse_json(body).unwrap_or(json!({}));
        let created = state.write(|db| {
            db.ensure_project(org, project);
            let stored = db.project_mut(org, project)?;
            let next = stored.list_bucket(key).len() + 1;
            let name = payload.get("name").and_then(|value| value.as_str()).unwrap_or("Tableau de bord");
            let widgets = payload
                .get("widgets")
                .and_then(|value| value.as_array())
                .cloned()
                .unwrap_or_default()
                .into_iter()
                .enumerate()
                .map(|(index, widget)| normalize_widget(widget, index as i64 + 1))
                .collect::<Vec<_>>();
            let item = json!({
                "id": format!("dash-{next}"),
                "name": name,
                "position": next,
                "refreshInterval": 0,
                "dashboardScope": if team.is_some() { "project_team" } else { "project" },
                "widgets": widgets,
                "url": format!("/{org}/{project}/_apis/dashboard/dashboards/dash-{next}")
            });
            Some(stored.put_bucket(key, item))
        });
        return match created {
            Some(item) => Json(dress_dashboard(state, org, project, item)).into_response(),
            None => fail(StatusCode::NOT_FOUND, "projet inconnu"),
        };
    }
    if rest.len() == 2 && method == Method::DELETE {
        return fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée");
    }
    let id = rest.get(2).map(String::as_str).unwrap_or("");
    if rest.get(3).is_some_and(|value| value.eq_ignore_ascii_case("widgets")) {
        return dashboard_widgets(state, method, org, project, id, rest, body);
    }
    if rest.len() == 3 && method == Method::GET {
        return match state.read(|db| db.project(org, project).and_then(|stored| stored.get_bucket(key, id))) {
            Some(item) => Json(dress_dashboard(state, org, project, item)).into_response(),
            None => fail(StatusCode::NOT_FOUND, "tableau de bord introuvable"),
        };
    }
    if rest.len() == 3 && (method == Method::PATCH || method == Method::PUT) {
        return update_dashboard(state, org, project, id, body);
    }
    if rest.len() == 3 && method == Method::DELETE {
        let Some(found) = load_dashboard(state, org, project, id) else {
            return fail(StatusCode::NOT_FOUND, "tableau de bord introuvable");
        };
        let stored_id = item_id(&found);
        state.write(|db| {
            if let Some(stored) = db.project_mut(org, project) {
                if let Some(bucket) = stored.buckets.get_mut(key) {
                    bucket.remove(&stored_id);
                }
            }
        });
        return StatusCode::NO_CONTENT.into_response();
    }
    if method == Method::DELETE {
        return fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée");
    }
    fail(StatusCode::NOT_FOUND, "tableau de bord introuvable")
}

fn update_dashboard(state: &AppState, org: &str, project: &str, id: &str, body: &[u8]) -> Response {
    let Some(previous) = load_dashboard(state, org, project, id) else {
        return fail(StatusCode::NOT_FOUND, "tableau de bord introuvable");
    };
    let Some(payload) = parse_json(body).filter(|value| value.is_object()) else {
        return fail(StatusCode::BAD_REQUEST, "tableau illisible");
    };
    let Some(name_value) = payload.get("name") else {
        return fail(StatusCode::BAD_REQUEST, "nom de tableau absent");
    };
    let Some(name_raw) = name_value.as_str() else {
        return fail(StatusCode::BAD_REQUEST, "tableau illisible");
    };
    let name = name_raw.trim();
    if name.is_empty() {
        return fail(StatusCode::BAD_REQUEST, "nom de tableau absent");
    }
    let position = previous.get("position").cloned().unwrap_or(json!(1));
    let widgets = widget_list(&previous);
    let mut updated = previous;
    if let Some(object) = updated.as_object_mut() {
        object.insert("name".into(), json!(name));
        object.insert("position".into(), position);
        object.insert("widgets".into(), json!(widgets));
    }
    state.write(|db| {
        if let Some(stored) = db.project_mut(org, project) {
            stored.put_bucket("dashboard/dashboards", updated.clone());
        }
    });
    crate::shapes::deliver_work_item(state, org, project, "dashboard.updated", &updated);
    Json(dress_dashboard(state, org, project, updated)).into_response()
}

fn dashboard_widgets(
    state: &AppState,
    method: &Method,
    org: &str,
    project: &str,
    dashboard_id: &str,
    rest: &[String],
    body: &[u8],
) -> Response {
    let Some(dashboard) = load_dashboard(state, org, project, dashboard_id) else {
        return fail(StatusCode::NOT_FOUND, "tableau de bord introuvable");
    };
    if rest.len() == 4 && method == Method::GET {
        let value = widget_list(&dashboard)
            .into_iter()
            .map(|widget| dress_widget(state, org, project, widget))
            .collect();
        return Json(page(value)).into_response();
    }
    if rest.len() == 4 && method == Method::POST {
        let payload = parse_json(body).unwrap_or(json!({}));
        let mut widgets = widget_list(&dashboard);
        let widget = normalize_widget(payload, widgets.len() as i64 + 1);
        widgets.push(widget.clone());
        save_widgets(state, org, project, dashboard, widgets);
        return Json(dress_widget(state, org, project, widget)).into_response();
    }
    if rest.len() >= 5 {
        let widget_id = &rest[4];
        let mut widgets = widget_list(&dashboard);
        let Some(index) = widgets.iter().position(|widget| item_id(widget) == *widget_id) else {
            return fail(StatusCode::NOT_FOUND, "widget introuvable");
        };
        if method == Method::GET {
            return Json(dress_widget(state, org, project, widgets[index].clone())).into_response();
        }
        if method == Method::DELETE {
            let removed = widgets.remove(index);
            save_widgets(state, org, project, dashboard, widgets);
            crate::shapes::deliver_work_item(state, org, project, "dashboard.widget.deleted", &removed);
            return StatusCode::NO_CONTENT.into_response();
        }
        if method == Method::PUT || method == Method::PATCH {
            let payload = parse_json(body).unwrap_or(json!({}));
            if let Some(object) = widgets[index].as_object_mut() {
                for field in ["name", "contributionId", "settings", "position", "size"] {
                    if let Some(value) = payload.get(field) {
                        object.insert(field.to_string(), value.clone());
                    }
                }
            }
            let updated = widgets[index].clone();
            save_widgets(state, org, project, dashboard, widgets);
            return Json(dress_widget(state, org, project, updated)).into_response();
        }
        return fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée");
    }
    fail(StatusCode::NOT_FOUND, "widget introuvable")
}

fn load_dashboard(state: &AppState, org: &str, project: &str, id: &str) -> Option<Value> {
    state.read(|db| db.project(org, project).and_then(|stored| stored.get_bucket("dashboard/dashboards", id)))
}

fn save_widgets(state: &AppState, org: &str, project: &str, mut dashboard: Value, widgets: Vec<Value>) {
    if let Some(object) = dashboard.as_object_mut() {
        object.insert("widgets".into(), json!(widgets));
    }
    state.write(|db| {
        if let Some(stored) = db.project_mut(org, project) {
            stored.put_bucket("dashboard/dashboards", dashboard);
        }
    });
}

fn widget_list(dashboard: &Value) -> Vec<Value> {
    dashboard.get("widgets").and_then(|value| value.as_array()).cloned().unwrap_or_default()
}

fn normalize_widget(mut widget: Value, index: i64) -> Value {
    if let Some(object) = widget.as_object_mut() {
        let id = object.get("id").map(value_text).filter(|value| !value.is_empty()).unwrap_or_else(|| format!("widget-{index}"));
        object.insert("id".into(), json!(id));
        object.entry("name").or_insert(json!("Widget"));
        object.entry("position").or_insert(json!({"row": 1, "column": index}));
        object.entry("size").or_insert(json!({"rowSpan": 1, "columnSpan": 2}));
    }
    widget
}

fn dress_dashboard(state: &AppState, org: &str, project: &str, mut dashboard: Value) -> Value {
    let widgets = widget_list(&dashboard)
        .into_iter()
        .map(|widget| dress_widget(state, org, project, widget))
        .collect::<Vec<_>>();
    if let Some(object) = dashboard.as_object_mut() {
        object.insert("widgets".into(), json!(widgets));
    }
    dashboard
}

fn dress_widget(state: &AppState, org: &str, project: &str, mut widget: Value) -> Value {
    let rendered = render_widget(state, org, project, &widget);
    if let Some(object) = widget.as_object_mut() {
        object.insert("rendered".into(), rendered);
    }
    widget
}

fn render_widget(state: &AppState, org: &str, project: &str, widget: &Value) -> Value {
    let title = item_text(widget, "name").unwrap_or_else(|| "Widget".into());
    let settings = widget_settings(widget);
    match widget_kind(widget) {
        "markdown" => {
            let text = settings.get("content").and_then(|value| value.as_str()).unwrap_or("").to_string();
            json!({"kind": "markdown", "title": title, "text": text, "value": Value::Null})
        }
        "build" => {
            let definition_id = settings.get("definitionId").and_then(|value| value.as_i64());
            let builds = state.read(|db| db.project(org, project).map(|stored| stored.list_bucket("build/builds")).unwrap_or_default());
            let latest = builds
                .into_iter()
                .filter(|item| {
                    definition_id
                        .map(|id| item.get("definition").and_then(|definition| definition.get("id")).and_then(|value| value.as_i64()) == Some(id))
                        .unwrap_or(true)
                })
                .max_by_key(|item| item.get("id").and_then(|value| value.as_i64()).unwrap_or(0));
            match latest {
                Some(build) => {
                    let number = item_text(&build, "buildNumber").unwrap_or_default();
                    let status = item_text(&build, "status").unwrap_or_else(|| "notStarted".into());
                    let result = item_text(&build, "result").unwrap_or_default();
                    json!({"kind": "build", "title": title, "text": format!("{number} {status} {result}").trim().to_string(), "value": if result.is_empty() { status } else { result }})
                }
                None => json!({"kind": "build", "title": title, "text": "aucune build", "value": Value::Null}),
            }
        }
        "query" => {
            let wanted = settings.get("state").and_then(|value| value.as_str()).map(str::to_string);
            let count = state.read(|db| {
                db.project(org, project)
                    .map(|stored| {
                        stored
                            .work_items
                            .values()
                            .filter(|item| {
                                wanted.as_ref().map(|state_name| {
                                    item.get("fields").and_then(|fields| fields.get("System.State")).and_then(|value| value.as_str()) == Some(state_name.as_str())
                                }).unwrap_or(true)
                            })
                            .count()
                    })
                    .unwrap_or(0)
            });
            json!({"kind": "query", "title": title, "text": "éléments", "value": count})
        }
        _ => json!({"kind": "note", "title": title, "text": "", "value": Value::Null}),
    }
}

fn widget_kind(widget: &Value) -> &'static str {
    let name = item_text(widget, "name").unwrap_or_default().to_ascii_lowercase();
    let contribution = item_text(widget, "contributionId").unwrap_or_default().to_ascii_lowercase();
    let haystack = format!("{name} {contribution}");
    if haystack.contains("markdown") {
        "markdown"
    } else if haystack.contains("build") {
        "build"
    } else if haystack.contains("query") || haystack.contains("work item") || haystack.contains("scalar") {
        "query"
    } else {
        "note"
    }
}

fn widget_settings(widget: &Value) -> Value {
    match widget.get("settings") {
        Some(Value::String(text)) => serde_json::from_str(text).unwrap_or_else(|_| json!({"content": text})),
        Some(value) => value.clone(),
        None => json!({}),
    }
}

pub fn endpoint_types(method: &Method) -> Response {
    if method != Method::GET {
        return fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée");
    }
    Json(page(endpoint_type_catalog())).into_response()
}

fn endpoint_type_catalog() -> Vec<Value> {
    vec![
        json!({"name": "generic", "displayName": "Generic", "description": "Connexion générique", "authenticationSchemes": [{"scheme": "UsernamePassword"}, {"scheme": "Token"}]}),
        json!({"name": "github", "displayName": "GitHub", "description": "Dépôt GitHub", "authenticationSchemes": [{"scheme": "Token"}, {"scheme": "UsernamePassword"}]}),
        json!({"name": "azurerm", "displayName": "Azure Resource Manager", "description": "Abonnement Azure", "authenticationSchemes": [{"scheme": "ServicePrincipal"}]}),
        json!({"name": "kubernetes", "displayName": "Kubernetes", "description": "Cluster Kubernetes", "authenticationSchemes": [{"scheme": "Kubernetes"}, {"scheme": "Token"}]}),
        json!({"name": "ssh", "displayName": "SSH", "description": "Hôte SSH", "authenticationSchemes": [{"scheme": "UsernamePassword"}]}),
        json!({"name": "dockerregistry", "displayName": "Docker Registry", "description": "Registre de conteneurs", "authenticationSchemes": [{"scheme": "UsernamePassword"}, {"scheme": "Token"}]}),
    ]
}

fn known_endpoint_type(name: &str) -> bool {
    endpoint_type_catalog()
        .iter()
        .any(|item| item.get("name").and_then(|value| value.as_str()).is_some_and(|stored| stored.eq_ignore_ascii_case(name)))
}

fn secret_parameter(key: &str) -> bool {
    matches!(
        key.to_ascii_lowercase().as_str(),
        "password" | "apitoken" | "token" | "pat" | "serviceprincipalkey" | "privatekey"
    )
}

fn authorization_from(payload: &Value, previous: Option<&Value>) -> Value {
    let incoming = payload.get("authorization");
    let previous_auth = previous.and_then(|item| item.get("authorization"));
    let scheme = incoming
        .and_then(|item| item.get("scheme"))
        .and_then(|value| value.as_str())
        .or_else(|| previous_auth.and_then(|item| item.get("scheme")).and_then(|value| value.as_str()))
        .unwrap_or("UsernamePassword");
    let mut params = previous_auth
        .and_then(|item| item.get("parameters"))
        .and_then(|value| value.as_object())
        .cloned()
        .unwrap_or_default();
    if let Some(incoming_params) = incoming.and_then(|item| item.get("parameters")).and_then(|value| value.as_object()) {
        for (key, value) in incoming_params {
            let blank_secret = value.as_str().is_some_and(|text| text.is_empty()) && secret_parameter(key);
            if blank_secret {
                continue;
            }
            params.insert(key.clone(), value.clone());
        }
    }
    json!({"scheme": scheme, "parameters": params})
}

fn dress_endpoint(mut item: Value) -> Value {
    let Some(params) = item
        .get_mut("authorization")
        .and_then(|auth| auth.get_mut("parameters"))
        .and_then(|value| value.as_object_mut())
    else {
        return item;
    };
    for (key, value) in params.iter_mut() {
        if secret_parameter(key) {
            *value = json!("");
        }
    }
    item
}

fn endpoint_name(payload: &Value) -> String {
    payload.get("name").and_then(|value| value.as_str()).unwrap_or("").trim().to_string()
}

fn endpoint_duplicate(items: &[Value], name: &str, except: Option<&str>) -> bool {
    items.iter().any(|item| {
        item.get("name").and_then(|value| value.as_str()).is_some_and(|stored| stored.eq_ignore_ascii_case(name))
            && except.is_none_or(|id| item.get("id").and_then(|value| value.as_str()) != Some(id))
    })
}

fn endpoints(state: &AppState, method: &Method, org: &str, project: &str, rest: &[String], body: &[u8]) -> Response {
    let key = "serviceendpoint/endpoints";
    if rest.len() == 2 && method == Method::GET {
        let value = state
            .read(|db| db.project(org, project).map(|stored| stored.list_bucket(key)).unwrap_or_default())
            .into_iter()
            .map(dress_endpoint)
            .collect();
        return Json(page(value)).into_response();
    }
    if rest.len() == 2 && method == Method::POST {
        let payload = parse_json(body).unwrap_or(json!({}));
        let name = endpoint_name(&payload);
        if name.is_empty() {
            return fail(StatusCode::BAD_REQUEST, "nom de connexion absent");
        }
        let kind = payload.get("type").and_then(|value| value.as_str()).unwrap_or("generic").to_string();
        if !known_endpoint_type(&kind) {
            return fail(StatusCode::BAD_REQUEST, "type de connexion inconnu");
        }
        let duplicate = state.read(|db| {
            db.project(org, project)
                .map(|stored| endpoint_duplicate(&stored.list_bucket(key), &name, None))
                .unwrap_or(false)
        });
        if duplicate {
            return fail(StatusCode::BAD_REQUEST, "connexion déjà présente");
        }
        let created = state.write(|db| {
            db.ensure_project(org, project);
            let stored = db.project_mut(org, project)?;
            let next = stored
                .list_bucket(key)
                .iter()
                .filter_map(|item| {
                    item.get("id")
                        .and_then(|value| value.as_str())
                        .and_then(|id| id.strip_prefix("endpoint-"))
                        .and_then(|number| number.parse::<u64>().ok())
                })
                .max()
                .unwrap_or(0)
                + 1;
            let item = json!({
                "id": format!("endpoint-{next:08}"),
                "name": name,
                "type": kind,
                "url": payload.get("url").cloned().unwrap_or(json!("")),
                "isReady": true,
                "isShared": false,
                "owner": "library",
                "description": payload.get("description").cloned().unwrap_or(json!("")),
                "authorization": authorization_from(&payload, None),
                "data": payload.get("data").cloned().unwrap_or(json!({})),
                "serviceEndpointProjectReferences": [{"projectReference": {"name": project}, "name": name}]
            });
            Some(stored.put_bucket(key, item))
        });
        return match created {
            Some(item) => Json(dress_endpoint(item)).into_response(),
            None => fail(StatusCode::NOT_FOUND, "projet inconnu"),
        };
    }
    if method == Method::DELETE && rest.len() != 3 {
        return fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée");
    }
    let id = rest.get(2).map(String::as_str).unwrap_or("").to_string();
    if id.is_empty() {
        return fail(StatusCode::NOT_FOUND, "connexion introuvable");
    }
    if method == Method::GET {
        return match state.read(|db| db.project(org, project).and_then(|stored| stored.get_bucket(key, &id))) {
            Some(item) => Json(dress_endpoint(item)).into_response(),
            None => fail(StatusCode::NOT_FOUND, "connexion introuvable"),
        };
    }
    if method == Method::PUT || method == Method::PATCH {
        let payload = parse_json(body).unwrap_or(json!({}));
        let name = endpoint_name(&payload);
        if name.is_empty() {
            return fail(StatusCode::BAD_REQUEST, "nom de connexion absent");
        }
        let existing = state.read(|db| db.project(org, project).and_then(|stored| stored.get_bucket(key, &id)));
        let Some(existing) = existing else {
            return fail(StatusCode::NOT_FOUND, "connexion introuvable");
        };
        let kind = payload
            .get("type")
            .and_then(|value| value.as_str())
            .map(str::to_string)
            .or_else(|| existing.get("type").and_then(|value| value.as_str()).map(str::to_string))
            .unwrap_or_else(|| "generic".into());
        if !known_endpoint_type(&kind) {
            return fail(StatusCode::BAD_REQUEST, "type de connexion inconnu");
        }
        let duplicate = state.read(|db| {
            db.project(org, project)
                .map(|stored| endpoint_duplicate(&stored.list_bucket(key), &name, Some(id.as_str())))
                .unwrap_or(false)
        });
        if duplicate {
            return fail(StatusCode::BAD_REQUEST, "connexion déjà présente");
        }
        let updated = state.write(|db| {
            let stored = db.project_mut(org, project)?;
            let current = stored.get_bucket(key, &id)?;
            let url = payload.get("url").cloned().unwrap_or_else(|| current.get("url").cloned().unwrap_or(json!("")));
            let description = payload
                .get("description")
                .cloned()
                .unwrap_or_else(|| current.get("description").cloned().unwrap_or(json!("")));
            let item = json!({
                "id": id,
                "name": name,
                "type": kind,
                "url": url,
                "isReady": true,
                "isShared": current.get("isShared").cloned().unwrap_or(json!(false)),
                "owner": "library",
                "description": description,
                "authorization": authorization_from(&payload, Some(&current)),
                "data": payload.get("data").cloned().unwrap_or_else(|| current.get("data").cloned().unwrap_or(json!({}))),
                "serviceEndpointProjectReferences": [{"projectReference": {"name": project}, "name": name}]
            });
            Some(stored.put_bucket(key, item))
        });
        return match updated {
            Some(item) => Json(dress_endpoint(item)).into_response(),
            None => fail(StatusCode::NOT_FOUND, "connexion introuvable"),
        };
    }
    if method == Method::DELETE {
        let Some(found) = state.read(|db| db.project(org, project).and_then(|stored| stored.get_bucket(key, &id))) else {
            return fail(StatusCode::NOT_FOUND, "connexion introuvable");
        };
        let stored_id = found.get("id").and_then(|value| value.as_str()).unwrap_or("").to_string();
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

fn column_key(team: &str, board: &str) -> String {
    format!("work/boards/{team}/{board}")
}

fn load_columns(state: &AppState, org: &str, project: &str, team: &str, board: &str) -> Vec<Value> {
    let key = column_key(team, board);
    state
        .read(|db| {
            db.project(org, project)
                .and_then(|stored| stored.get_bucket(&key, "columns"))
                .and_then(|item| item.get("columns").and_then(|value| value.as_array().cloned()))
        })
        .filter(|columns| !columns.is_empty())
        .unwrap_or_else(board_columns)
}

fn put_columns(state: &AppState, org: &str, project: &str, team: &str, board: &str, body: &[u8]) -> Response {
    let Some(payload) = parse_json(body) else {
        return fail(StatusCode::BAD_REQUEST, "colonnes illisibles");
    };
    let raw = if payload.is_array() {
        payload
    } else {
        payload.get("value").cloned().unwrap_or(Value::Null)
    };
    let columns = match normalize_columns(&raw) {
        Ok(columns) => columns,
        Err(message) => return fail(StatusCode::BAD_REQUEST, message),
    };
    let key = column_key(team, board);
    let stored = state.write(|db| {
        let Some(project_data) = db.project_mut(org, project) else {
            return false;
        };
        project_data.put_bucket(&key, json!({"id": "columns", "columns": columns}));
        true
    });
    if !stored {
        return fail(StatusCode::NOT_FOUND, "projet absent");
    }
    Json(page(columns)).into_response()
}

fn normalize_columns(raw: &Value) -> Result<Vec<Value>, &'static str> {
    let Some(items) = raw.as_array() else {
        return Err("colonnes illisibles");
    };
    let mut names = Vec::new();
    let mut columns = Vec::new();
    for item in items {
        let name = item.get("name").and_then(|value| value.as_str()).unwrap_or("").trim();
        if name.is_empty() {
            return Err("nom de colonne absent");
        }
        if names.iter().any(|existing: &String| existing.eq_ignore_ascii_case(name)) {
            return Err("colonne déjà présente");
        }
        names.push(name.to_string());
        let column_type = match item.get("columnType") {
            None | Some(Value::Null) => "inProgress",
            Some(Value::String(value)) if value == "incoming" => "incoming",
            Some(Value::String(value)) if value == "inProgress" => "inProgress",
            Some(Value::String(value)) if value == "outgoing" => "outgoing",
            Some(_) => return Err("type de colonne inconnu"),
        };
        let item_limit = match item.get("itemLimit") {
            None | Some(Value::Null) => 0,
            Some(value) => {
                let number = value.as_i64().ok_or("limite de colonne invalide")?;
                if number < 0 {
                    return Err("limite de colonne invalide");
                }
                number
            }
        };
        let is_split = item.get("isSplit").and_then(|value| value.as_bool()).unwrap_or(false);
        let state_mappings = column_mappings(item, name)?;
        columns.push(json!({
            "name": name,
            "itemLimit": item_limit,
            "isSplit": is_split,
            "stateMappings": state_mappings,
            "columnType": column_type
        }));
    }
    let incoming = columns.iter().filter(|column| column["columnType"] == "incoming").count();
    let outgoing = columns.iter().filter(|column| column["columnType"] == "outgoing").count();
    let first_incoming = columns.first().is_some_and(|column| column["columnType"] == "incoming");
    let last_outgoing = columns.last().is_some_and(|column| column["columnType"] == "outgoing");
    if incoming != 1 || outgoing != 1 || !first_incoming || !last_outgoing {
        return Err("colonnes d'entrée et de sortie requises");
    }
    Ok(columns)
}

fn column_mappings(item: &Value, name: &str) -> Result<Value, &'static str> {
    let mut mappings = Map::new();
    let provided = match item.get("stateMappings") {
        None | Some(Value::Null) => None,
        Some(Value::Object(object)) => Some(object),
        Some(_) => return Err("état de colonne absent"),
    };
    for key in ["User Story", "Bug", "Task"] {
        let text = match provided.and_then(|object| object.get(key)) {
            None => name.to_string(),
            Some(value) => mapping_text(value)?,
        };
        mappings.insert(key.to_string(), json!(text));
    }
    if let Some(object) = provided {
        for (key, value) in object {
            if key == "User Story" || key == "Bug" || key == "Task" {
                continue;
            }
            mappings.insert(key.clone(), json!(mapping_text(value)?));
        }
    }
    Ok(Value::Object(mappings))
}

fn mapping_text(value: &Value) -> Result<String, &'static str> {
    let text = value.as_str().unwrap_or("").trim();
    if text.is_empty() {
        Err("état de colonne absent")
    } else {
        Ok(text.to_string())
    }
}

fn allowed_mappings(columns: &[Value]) -> Value {
    let mut story = Vec::new();
    let mut bug = Vec::new();
    for column in columns {
        let name = column["name"].as_str().unwrap_or("");
        let mappings = column.get("stateMappings");
        story.push(json!(mappings
            .and_then(|item| item.get("User Story"))
            .and_then(|value| value.as_str())
            .unwrap_or(name)));
        bug.push(json!(mappings
            .and_then(|item| item.get("Bug"))
            .and_then(|value| value.as_str())
            .unwrap_or(name)));
    }
    json!({"User Story": story, "Bug": bug})
}

fn row_key(team: &str, board: &str) -> String {
    format!("work/boards/{team}/{board}/rows")
}

fn default_rows() -> Vec<Value> {
    vec![json!({"id": "row-1", "name": null})]
}

fn load_rows(state: &AppState, org: &str, project: &str, team: &str, board: &str) -> Vec<Value> {
    let key = row_key(team, board);
    state
        .read(|db| {
            db.project(org, project)
                .and_then(|stored| stored.get_bucket(&key, "rows"))
                .and_then(|item| item.get("rows").and_then(|value| value.as_array().cloned()))
        })
        .filter(|rows| !rows.is_empty())
        .unwrap_or_else(default_rows)
}

fn put_rows(state: &AppState, org: &str, project: &str, team: &str, board: &str, body: &[u8]) -> Response {
    let Some(payload) = parse_json(body) else {
        return fail(StatusCode::BAD_REQUEST, "lignes illisibles");
    };
    let raw = if payload.is_array() {
        payload
    } else {
        payload.get("value").cloned().unwrap_or(Value::Null)
    };
    let rows = match normalize_rows(&raw) {
        Ok(rows) => rows,
        Err(message) => return fail(StatusCode::BAD_REQUEST, message),
    };
    let key = row_key(team, board);
    let stored = state.write(|db| {
        let Some(project_data) = db.project_mut(org, project) else {
            return false;
        };
        project_data.put_bucket(&key, json!({"id": "rows", "rows": rows}));
        true
    });
    if !stored {
        return fail(StatusCode::NOT_FOUND, "projet absent");
    }
    Json(page(rows)).into_response()
}

fn normalize_rows(raw: &Value) -> Result<Vec<Value>, &'static str> {
    let Some(items) = raw.as_array() else {
        return Err("lignes illisibles");
    };
    if items.is_empty() {
        return Err("ligne par défaut requise");
    }
    let mut names = Vec::new();
    let mut rows = Vec::new();
    for (index, item) in items.iter().enumerate() {
        let name = match item.get("name") {
            None | Some(Value::Null) => None,
            Some(Value::String(text)) => {
                let trimmed = text.trim();
                if trimmed.is_empty() {
                    return Err(if index == 0 { "ligne par défaut requise" } else { "nom de ligne absent" });
                }
                Some(trimmed.to_string())
            }
            Some(_) => return Err("nom de ligne absent"),
        };
        if index == 0 && name.is_some() {
            return Err("ligne par défaut requise");
        }
        if index > 0 && name.is_none() {
            return Err("nom de ligne absent");
        }
        if let Some(label) = &name {
            if names.iter().any(|existing: &String| existing.eq_ignore_ascii_case(label)) {
                return Err("ligne déjà présente");
            }
            names.push(label.clone());
        }
        let id = item
            .get("id")
            .and_then(|value| value.as_str())
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
            .unwrap_or_else(|| format!("row-{}", index + 1));
        if rows.iter().any(|row: &Value| row["id"].as_str() == Some(id.as_str())) {
            return Err("ligne déjà présente");
        }
        rows.push(json!({"id": id, "name": name}));
    }
    Ok(rows)
}

fn board_document(org: &str, project: &str, team: &str, name: &str, columns: Vec<Value>, rows: Vec<Value>) -> Value {
    let allowed = allowed_mappings(&columns);
    json!({
        "id": name.to_ascii_lowercase(),
        "name": name,
        "url": format!("/{org}/{project}/{team}/_apis/work/boards/{name}"),
        "columns": columns,
        "rows": rows,
        "allowedMappings": allowed
    })
}

fn board_columns() -> Vec<Value> {
    ["New", "Active", "Resolved", "Closed"]
        .iter()
        .map(|name| {
            json!({
                "name": name,
                "itemLimit": 0,
                "isSplit": false,
                "stateMappings": {"User Story": name, "Bug": name, "Task": name},
                "columnType": if *name == "New" { "incoming" } else if *name == "Closed" { "outgoing" } else { "inProgress" }
            })
        })
        .collect()
}

fn backlog_matches(backlog: &str, item: &Value) -> bool {
    let kind = item
        .get("fields")
        .and_then(|fields| fields.get("System.WorkItemType"))
        .and_then(|value| value.as_str())
        .unwrap_or("");
    match backlog {
        "Microsoft.EpicCategory" => kind == "Epic",
        "Microsoft.FeatureCategory" => kind == "Feature",
        "Microsoft.TaskCategory" => kind == "Task",
        "Microsoft.RequirementCategory" => kind == "User Story" || kind == "Bug",
        _ => true,
    }
}

fn delivery_plans(state: &AppState, method: &Method, org: &str, project: &str, rest: &[String], body: &[u8]) -> Response {
    let key = "work/plans";
    if rest.len() == 2 {
        if method == Method::GET {
            let mut rows = state
                .read(|db| db.project(org, project).map(|stored| stored.list_bucket(key)).unwrap_or_default());
            rows.sort_by(|left, right| item_id(left).cmp(&item_id(right)));
            let value = rows.into_iter().map(|item| dress_delivery_plan(org, project, item)).collect();
            return Json(page(value)).into_response();
        }
        if method == Method::POST {
            let (name, description, mappings) = match delivery_plan_create(body) {
                Ok(fields) => fields,
                Err(message) => return fail(StatusCode::BAD_REQUEST, message),
            };
            let existing = state.read(|db| db.project(org, project).map(|stored| stored.list_bucket(key)).unwrap_or_default());
            if existing.iter().any(|item| item_text(item, "name").is_some_and(|stored| stored.eq_ignore_ascii_case(&name))) {
                return fail(StatusCode::BAD_REQUEST, "plan existant");
            }
            let next = existing
                .iter()
                .filter_map(|item| item_id(item).strip_prefix("dplan-").and_then(|number| number.parse::<i64>().ok()))
                .max()
                .unwrap_or(0)
                + 1;
            let id = format!("dplan-{next}");
            let mappings = mappings
                .into_iter()
                .map(|(team_id, category)| json!({"teamId": team_id, "categoryReferenceName": category}))
                .collect::<Vec<_>>();
            let item = json!({
                "id": id,
                "revision": 1,
                "name": name,
                "description": description,
                "type": "deliveryTimelineView",
                "createdBy": {"displayName": "azurz", "uniqueName": "azurz"},
                "createdDate": "2026-10-29T00:00:00Z",
                "properties": {"teamBacklogMappings": mappings}
            });
            state.write(|db| {
                db.ensure_project(org, project);
                if let Some(stored) = db.project_mut(org, project) {
                    stored.put_bucket(key, item.clone());
                }
            });
            return Json(dress_delivery_plan(org, project, item)).into_response();
        }
        return fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée");
    }
    if rest.len() != 3 {
        if method == Method::DELETE {
            return fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée");
        }
        return fail(StatusCode::NOT_FOUND, "plan de livraison introuvable");
    }
    let id = rest[2].as_str();
    if method == Method::GET {
        return match state.read(|db| db.project(org, project).and_then(|stored| stored.get_bucket(key, id))) {
            Some(item) => Json(dress_delivery_plan(org, project, item)).into_response(),
            None => fail(StatusCode::NOT_FOUND, "plan de livraison introuvable"),
        };
    }
    if method == Method::DELETE {
        let Some(found) = state.read(|db| db.project(org, project).and_then(|stored| stored.get_bucket(key, id))) else {
            return fail(StatusCode::NOT_FOUND, "plan de livraison introuvable");
        };
        let stored_id = item_id(&found);
        let removed = state.write(|db| {
            db.project_mut(org, project)
                .and_then(|stored| stored.buckets.get_mut(key))
                .is_some_and(|bucket| bucket.remove(&stored_id))
        });
        return if removed {
            StatusCode::NO_CONTENT.into_response()
        } else {
            fail(StatusCode::NOT_FOUND, "plan de livraison introuvable")
        };
    }
    fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée")
}

fn delivery_plan_create(body: &[u8]) -> Result<(String, String, Vec<(String, String)>), &'static str> {
    let Some(payload) = parse_json(body) else {
        return Err("JSON illisible");
    };
    let Some(object) = payload.as_object() else {
        return Err("plan illisible");
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
    let description = match object.get("description") {
        None => String::new(),
        Some(value) => {
            let Some(text) = value.as_str() else {
                return Err("description absente");
            };
            text.trim().to_string()
        }
    };
    if let Some(kind) = object.get("type") {
        if kind.as_str() != Some("deliveryTimelineView") {
            return Err("type absent");
        }
    }
    let mappings = match object.get("properties") {
        None => vec![("1".to_string(), "Microsoft.RequirementCategory".to_string())],
        Some(value) => {
            let Some(properties) = value.as_object() else {
                return Err("backlog absent");
            };
            match properties.get("teamBacklogMappings") {
                None => vec![("1".to_string(), "Microsoft.RequirementCategory".to_string())],
                Some(mappings) => delivery_mappings(mappings)?,
            }
        }
    };
    Ok((name, description, mappings))
}

fn delivery_mappings(value: &Value) -> Result<Vec<(String, String)>, &'static str> {
    let Some(rows) = value.as_array() else {
        return Err("backlog absent");
    };
    if rows.is_empty() {
        return Err("backlog absent");
    }
    let mut mappings = Vec::new();
    for row in rows {
        let Some(object) = row.as_object() else {
            return Err("backlog absent");
        };
        let Some(category) = object
            .get("categoryReferenceName")
            .and_then(|item| item.as_str())
            .map(str::trim)
            .filter(|item| {
                matches!(
                    *item,
                    "Microsoft.EpicCategory"
                        | "Microsoft.FeatureCategory"
                        | "Microsoft.RequirementCategory"
                        | "Microsoft.TaskCategory"
                )
            })
            .map(str::to_string)
        else {
            return Err("backlog absent");
        };
        let team_id = match object.get("teamId") {
            None => "1".to_string(),
            Some(item) => {
                let Some(text) = item.as_str().map(str::trim).filter(|text| !text.is_empty()) else {
                    return Err("backlog absent");
                };
                text.to_string()
            }
        };
        mappings.push((team_id, category));
    }
    Ok(mappings)
}

fn dress_delivery_plan(org: &str, project: &str, mut item: Value) -> Value {
    let id = item_id(&item);
    if let Some(object) = item.as_object_mut() {
        object.insert("url".into(), json!(format!("/{org}/{project}/_apis/work/plans/{id}")));
    }
    item
}

const PROCESSES: &str = "work/processes";
const PICKLISTS: &str = "work/processes/lists";
const AGILE: &str = "adcc42ab-9882-485e-a3ed-7678f01f66bc";
const SCRUM: &str = "6b724908-ef14-45cf-84f8-768b5384da45";
const CMMI: &str = "27450541-8e31-4150-9947-dc59f998fc01";
const BASIC: &str = "b8a3a935-7e91-48b8-a94c-606d37c3e9f2";

struct SystemProcess {
    type_id: &'static str,
    name: &'static str,
    description: &'static str,
    is_default: bool,
}

const SYSTEM_PROCESSES: &[SystemProcess] = &[
    SystemProcess {
        type_id: AGILE,
        name: "Agile",
        description: "This template is flexible and will work great for most teams using Agile planning methods, including those practicing Scrum.",
        is_default: true,
    },
    SystemProcess {
        type_id: SCRUM,
        name: "Scrum",
        description: "This template is for teams who follow the Scrum framework.",
        is_default: false,
    },
    SystemProcess {
        type_id: CMMI,
        name: "CMMI",
        description: "This template is for more formal projects requiring a framework for process improvement and an auditable record of decisions.",
        is_default: false,
    },
    SystemProcess {
        type_id: BASIC,
        name: "Basic",
        description: "This template is flexible for any process and great for teams getting started with Azure DevOps.",
        is_default: false,
    },
];

pub fn work_processes(
    state: &AppState,
    method: &Method,
    org: &str,
    project: Option<&str>,
    rest: &[String],
    body: &[u8],
) -> Response {
    let Some(tail) = process_tail(rest) else {
        return if method == Method::DELETE {
            fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée")
        } else {
            fail(StatusCode::NOT_FOUND, "processus introuvable")
        };
    };
    if tail.len() > 1 {
        return if method == Method::DELETE {
            fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée")
        } else {
            fail(StatusCode::NOT_FOUND, "processus introuvable")
        };
    }
    if tail.is_empty() {
        if method == Method::GET {
            let value = match project {
                Some(project) => project_processes(state, org, project),
                None => org_processes(state, org),
            };
            return Json(page(value)).into_response();
        }
        if method == Method::POST {
            let project = project.map(str::to_string).unwrap_or_else(|| state.names().1);
            return match process_create(state, org, &project, body) {
                Ok(item) => Json(item).into_response(),
                Err(message) => fail(StatusCode::BAD_REQUEST, message),
            };
        }
        return fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée");
    }
    let id = pct_decode(&tail[0]);
    if let Some(system) = system_process(id.trim()) {
        if method == Method::GET {
            let owner = project.unwrap_or("");
            return Json(dress_system(org, owner, system)).into_response();
        }
        if method == Method::DELETE {
            return fail(StatusCode::BAD_REQUEST, "processus système");
        }
        return fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée");
    }
    if method == Method::GET {
        return match locate_process(state, org, project, id.trim()) {
            Some((owner, found)) => Json(dress_process(org, &owner, found)).into_response(),
            None => fail(StatusCode::NOT_FOUND, "processus introuvable"),
        };
    }
    if method == Method::DELETE {
        let Some((owner, found)) = locate_process(state, org, project, id.trim()) else {
            return fail(StatusCode::NOT_FOUND, "processus introuvable");
        };
        let stored_id = item_id(&found);
        let removed = state.write(|db| -> bool {
            db.project_mut(org, &owner)
                .and_then(|stored| stored.buckets.get_mut(PROCESSES))
                .is_some_and(|bucket| bucket.remove(&stored_id))
        });
        return if removed {
            StatusCode::NO_CONTENT.into_response()
        } else {
            fail(StatusCode::NOT_FOUND, "processus introuvable")
        };
    }
    fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée")
}

fn process_tail(rest: &[String]) -> Option<&[String]> {
    let index = rest.iter().rposition(|part| part.eq_ignore_ascii_case("processes"))?;
    Some(&rest[index + 1..])
}

fn system_process(id: &str) -> Option<&'static SystemProcess> {
    SYSTEM_PROCESSES.iter().find(|process| process.type_id.eq_ignore_ascii_case(id) || process.name.eq_ignore_ascii_case(id))
}

fn dress_system(org: &str, project: &str, process: &SystemProcess) -> Value {
    let mut item = json!({
        "id": process.type_id,
        "typeId": process.type_id,
        "name": process.name,
        "description": process.description,
        "parentProcessTypeId": "00000000-0000-0000-0000-000000000000",
        "referenceName": Value::Null,
        "isEnabled": true,
        "isDefault": process.is_default,
        "customizationType": "System",
        "url": format!("/{org}/_apis/work/processes/{}", process.type_id)
    });
    if !project.is_empty() {
        if let Some(object) = item.as_object_mut() {
            object.insert("project".into(), json!(project));
        }
    }
    item
}

fn project_processes(state: &AppState, org: &str, project: &str) -> Vec<Value> {
    let mut rows: Vec<Value> = SYSTEM_PROCESSES.iter().map(|process| dress_system(org, project, process)).collect();
    let mut custom = list_processes(state, org, project);
    custom.sort_by(|left, right| item_id(left).cmp(&item_id(right)));
    rows.extend(custom.into_iter().map(|item| dress_process(org, project, item)));
    rows
}

fn org_processes(state: &AppState, org: &str) -> Vec<Value> {
    let mut rows: Vec<Value> = SYSTEM_PROCESSES.iter().map(|process| dress_system(org, "", process)).collect();
    let mut custom = all_processes(state, org);
    custom.sort_by(|left, right| item_id(&left.1).cmp(&item_id(&right.1)));
    rows.extend(custom.into_iter().map(|(project, item)| dress_process(org, &project, item)));
    rows
}

fn list_processes(state: &AppState, org: &str, project: &str) -> Vec<Value> {
    state.read(|db| db.project(org, project).map(|stored| stored.list_bucket(PROCESSES)).unwrap_or_default())
}

fn all_processes(state: &AppState, org: &str) -> Vec<(String, Value)> {
    state.read(|db| {
        let Some(organization) = db.orgs.get(org) else {
            return Vec::new();
        };
        let mut rows = Vec::new();
        for (name, project) in &organization.projects {
            if let Some(bucket) = project.buckets.get(PROCESSES) {
                for item in bucket.items.values() {
                    rows.push((name.clone(), item.clone()));
                }
            }
        }
        rows
    })
}

fn locate_process(state: &AppState, org: &str, project: Option<&str>, id: &str) -> Option<(String, Value)> {
    state.read(|db| {
        let organization = db.orgs.get(org)?;
        for (name, stored) in &organization.projects {
            if let Some(project) = project {
                if !name.eq_ignore_ascii_case(project) && stored.id != project {
                    continue;
                }
            }
            if let Some(item) = stored.buckets.get(PROCESSES).and_then(|bucket| {
                bucket.items.values().find(|item| {
                    item_id(item) == id
                        || item_text(item, "typeId").is_some_and(|stored| stored.eq_ignore_ascii_case(id))
                        || item_text(item, "name").is_some_and(|stored| stored.eq_ignore_ascii_case(id))
                        || item_text(item, "referenceName").is_some_and(|stored| stored.eq_ignore_ascii_case(id))
                })
            }) {
                return Some((name.clone(), item.clone()));
            }
        }
        None
    })
}

fn next_process_id(state: &AppState, org: &str) -> i64 {
    all_processes(state, org)
        .iter()
        .filter_map(|(_, item)| item_id(item).strip_prefix("proc-")?.parse::<i64>().ok())
        .max()
        .unwrap_or(0)
        + 1
}

fn process_create(state: &AppState, org: &str, project: &str, body: &[u8]) -> Result<Value, &'static str> {
    let (name, description, parent, reference) = process_payload(body)?;
    let id = format!("proc-{}", next_process_id(state, org));
    let item = json!({
        "id": id,
        "typeId": id,
        "name": name,
        "description": description,
        "parentProcessTypeId": parent,
        "referenceName": reference,
        "isEnabled": true,
        "isDefault": false,
        "customizationType": "Inherited"
    });
    state.write(|db| -> Result<(), &'static str> {
        db.ensure_project(org, project);
        let Some(stored) = db.project_mut(org, project) else {
            return Err("processus introuvable");
        };
        let existing = stored.list_bucket(PROCESSES);
        if existing.iter().any(|stored| item_text(stored, "name").is_some_and(|current| current.eq_ignore_ascii_case(&name)))
            || system_process(&name).is_some()
        {
            return Err("processus existant");
        }
        if existing.iter().any(|stored| item_text(stored, "referenceName").is_some_and(|current| current.eq_ignore_ascii_case(&reference))) {
            return Err("référence existante");
        }
        stored.put_bucket(PROCESSES, item.clone());
        Ok(())
    })?;
    Ok(dress_process(org, project, item))
}

fn process_payload(body: &[u8]) -> Result<(String, String, String, String), &'static str> {
    let Some(payload) = parse_json(body) else {
        return Err("JSON illisible");
    };
    let Some(object) = payload.as_object() else {
        return Err("processus illisible");
    };
    let Some(name) = object.get("name").and_then(|value| value.as_str()).map(str::trim).filter(|value| !value.is_empty()).map(str::to_string) else {
        return Err("nom absent");
    };
    let description = match object.get("description") {
        None => String::new(),
        Some(Value::String(value)) => value.trim().to_string(),
        Some(_) => return Err("description absente"),
    };
    let Some(parent) = object.get("parentProcessTypeId").and_then(|value| value.as_str()).map(str::trim).filter(|value| !value.is_empty()) else {
        return Err("parent absent");
    };
    let Some(parent_process) = SYSTEM_PROCESSES.iter().find(|process| process.type_id.eq_ignore_ascii_case(parent)) else {
        return Err("parent inconnu");
    };
    let reference = match object.get("referenceName") {
        None => process_reference(&name).ok_or("référence absente")?,
        Some(Value::String(value)) if process_reference_token(value.trim()) => value.trim().to_string(),
        Some(_) => return Err("référence absente"),
    };
    Ok((name, description, parent_process.type_id.to_string(), reference))
}

fn process_reference(name: &str) -> Option<String> {
    let slug: String = name.chars().filter(|char| char.is_ascii_alphanumeric()).collect();
    if process_reference_token(&format!("Inherited.{slug}")) {
        Some(format!("Inherited.{slug}"))
    } else {
        None
    }
}

fn process_reference_token(value: &str) -> bool {
    let mut parts = value.split('.');
    let token = |part: &str| {
        let mut chars = part.chars();
        matches!(chars.next(), Some(char) if char.is_ascii_alphabetic()) && chars.all(|char| char.is_ascii_alphanumeric())
    };
    parts.next().is_some_and(token) && parts.all(token) && value.contains('.')
}

fn dress_process(org: &str, project: &str, mut item: Value) -> Value {
    let id = item_id(&item);
    if let Some(object) = item.as_object_mut() {
        object.insert("project".into(), json!(project));
        object.insert("url".into(), json!(format!("/{org}/{project}/_apis/work/processes/{id}")));
    }
    item
}

pub fn picklists(state: &AppState, method: &Method, org: &str, project: &str, rest: &[String], body: &[u8]) -> Response {
    let Some(tail) = lists_tail(rest) else {
        return if method == Method::DELETE {
            fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée")
        } else {
            fail(StatusCode::NOT_FOUND, "liste de sélection introuvable")
        };
    };
    if tail.is_empty() {
        if method == Method::GET {
            let mut rows = list_picklists(state, org, project);
            rows.sort_by(|left, right| item_id(left).cmp(&item_id(right)));
            let value = rows.into_iter().map(|item| dress_picklist(org, project, item)).collect();
            return Json(page(value)).into_response();
        }
        if method == Method::POST {
            let (name, kind, items, suggested) = match picklist_create(body) {
                Ok(fields) => fields,
                Err(message) => return fail(StatusCode::BAD_REQUEST, message),
            };
            let existing = list_picklists(state, org, project);
            if existing.iter().any(|item| item_text(item, "name").is_some_and(|stored| stored.eq_ignore_ascii_case(&name))) {
                return fail(StatusCode::BAD_REQUEST, "liste existante");
            }
            let id = format!("list-{}", next_picklist_id(state, org));
            let item = json!({
                "id": id,
                "name": name,
                "type": kind,
                "items": items,
                "isSuggested": suggested,
                "project": project
            });
            state.write(|db| {
                db.ensure_project(org, project);
                if let Some(stored) = db.project_mut(org, project) {
                    stored.put_bucket(PICKLISTS, item.clone());
                }
            });
            return Json(dress_picklist(org, project, item)).into_response();
        }
        return fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée");
    }
    if tail.len() != 1 {
        return if method == Method::DELETE {
            fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée")
        } else {
            fail(StatusCode::NOT_FOUND, "liste de sélection introuvable")
        };
    }
    let id = tail[0].as_str();
    if method == Method::GET {
        return match state.read(|db| db.project(org, project).and_then(|stored| stored.get_bucket(PICKLISTS, id))) {
            Some(item) => Json(dress_picklist(org, project, item)).into_response(),
            None => fail(StatusCode::NOT_FOUND, "liste de sélection introuvable"),
        };
    }
    if method == Method::DELETE {
        let Some(found) = state.read(|db| db.project(org, project).and_then(|stored| stored.get_bucket(PICKLISTS, id))) else {
            return fail(StatusCode::NOT_FOUND, "liste de sélection introuvable");
        };
        let stored_id = item_id(&found);
        let removed = state.write(|db| -> bool {
            db.project_mut(org, project)
                .and_then(|stored| stored.buckets.get_mut(PICKLISTS))
                .is_some_and(|bucket| bucket.remove(&stored_id))
        });
        return if removed {
            StatusCode::NO_CONTENT.into_response()
        } else {
            fail(StatusCode::NOT_FOUND, "liste de sélection introuvable")
        };
    }
    fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée")
}

pub fn picklists_org(state: &AppState, method: &Method, org: &str, rest: &[String], body: &[u8]) -> Response {
    let Some(tail) = lists_tail(rest) else {
        return if method == Method::DELETE {
            fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée")
        } else {
            fail(StatusCode::NOT_FOUND, "liste de sélection introuvable")
        };
    };
    if tail.is_empty() {
        if method == Method::GET {
            let mut rows = all_picklists(state, org);
            rows.sort_by(|left, right| item_id(&left.1).cmp(&item_id(&right.1)));
            let value = rows
                .into_iter()
                .map(|(project, item)| dress_picklist(org, &project, item))
                .collect();
            return Json(page(value)).into_response();
        }
        if method == Method::POST {
            let project = state.names().1;
            return picklists(state, method, org, &project, rest, body);
        }
        return fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée");
    }
    if tail.len() != 1 {
        return if method == Method::DELETE {
            fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée")
        } else {
            fail(StatusCode::NOT_FOUND, "liste de sélection introuvable")
        };
    }
    let id = tail[0].as_str();
    let Some((project, found)) = locate_picklist(state, org, id) else {
        return fail(StatusCode::NOT_FOUND, "liste de sélection introuvable");
    };
    if method == Method::GET {
        return Json(dress_picklist(org, &project, found)).into_response();
    }
    if method == Method::DELETE {
        let stored_id = item_id(&found);
        let removed = state.write(|db| -> bool {
            db.project_mut(org, &project)
                .and_then(|stored| stored.buckets.get_mut(PICKLISTS))
                .is_some_and(|bucket| bucket.remove(&stored_id))
        });
        return if removed {
            StatusCode::NO_CONTENT.into_response()
        } else {
            fail(StatusCode::NOT_FOUND, "liste de sélection introuvable")
        };
    }
    fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée")
}

fn lists_tail(rest: &[String]) -> Option<&[String]> {
    let index = rest.iter().position(|part| part.eq_ignore_ascii_case("lists"))?;
    Some(&rest[index + 1..])
}

fn list_picklists(state: &AppState, org: &str, project: &str) -> Vec<Value> {
    state.read(|db| db.project(org, project).map(|stored| stored.list_bucket(PICKLISTS)).unwrap_or_default())
}

fn all_picklists(state: &AppState, org: &str) -> Vec<(String, Value)> {
    state.read(|db| {
        let Some(organization) = db.orgs.get(org) else {
            return Vec::new();
        };
        let mut rows = Vec::new();
        for (name, project) in &organization.projects {
            if let Some(bucket) = project.buckets.get(PICKLISTS) {
                for item in bucket.items.values() {
                    rows.push((name.clone(), item.clone()));
                }
            }
        }
        rows
    })
}

fn locate_picklist(state: &AppState, org: &str, id: &str) -> Option<(String, Value)> {
    state.read(|db| {
        let organization = db.orgs.get(org)?;
        for (name, project) in &organization.projects {
            if let Some(item) = project.get_bucket(PICKLISTS, id) {
                return Some((name.clone(), item));
            }
        }
        None
    })
}

fn next_picklist_id(state: &AppState, org: &str) -> i64 {
    state.read(|db| {
        let Some(organization) = db.orgs.get(org) else {
            return 1;
        };
        organization
            .projects
            .values()
            .filter_map(|project| project.buckets.get(PICKLISTS))
            .flat_map(|bucket| bucket.items.values())
            .filter_map(|item| item_id(item).strip_prefix("list-")?.parse::<i64>().ok())
            .max()
            .unwrap_or(0)
            + 1
    })
}

fn picklist_create(body: &[u8]) -> Result<(String, String, Vec<String>, bool), &'static str> {
    let Some(payload) = parse_json(body) else {
        return Err("JSON illisible");
    };
    let Some(object) = payload.as_object() else {
        return Err("liste illisible");
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
    let kind = match object.get("type") {
        None => "String".to_string(),
        Some(Value::String(value)) if matches!(value.as_str(), "String" | "Integer" | "Double") => value.clone(),
        Some(_) => return Err("type absent"),
    };
    let items = match object.get("items") {
        None => Vec::new(),
        Some(Value::Array(rows)) => {
            let mut items = Vec::new();
            for row in rows {
                let Some(text) = row.as_str().map(str::trim).filter(|text| !text.is_empty()) else {
                    return Err("éléments absents");
                };
                if !picklist_item(&kind, text) {
                    return Err("éléments absents");
                }
                items.push(text.to_string());
            }
            items
        }
        Some(_) => return Err("éléments absents"),
    };
    let suggested = match object.get("isSuggested") {
        None => false,
        Some(Value::Bool(value)) => *value,
        Some(_) => return Err("suggestion absente"),
    };
    Ok((name, kind, items, suggested))
}

fn picklist_item(kind: &str, text: &str) -> bool {
    match kind {
        "String" => true,
        "Integer" => {
            let digits = text.strip_prefix('-').unwrap_or(text);
            !digits.is_empty() && digits.chars().all(|ch| ch.is_ascii_digit())
        }
        "Double" => text.parse::<f64>().ok().is_some_and(|number| number.is_finite()),
        _ => false,
    }
}

fn dress_picklist(org: &str, project: &str, mut item: Value) -> Value {
    let id = item_id(&item);
    if let Some(object) = item.as_object_mut() {
        object.insert("project".into(), json!(project));
        object.insert(
            "url".into(),
            json!(format!("/{org}/{project}/_apis/work/processes/lists/{id}")),
        );
    }
    item
}

fn page(value: Vec<Value>) -> Value {
    json!({"count": value.len(), "value": value})
}
