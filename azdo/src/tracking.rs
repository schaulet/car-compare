use axum::http::{Method, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::{json, Value};

use crate::http::{fail, parse_json};
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
        ("work", "boards") => boards(state, method, org, project, team, rest),
        ("work", "backlogs") => backlogs(state, method, org, project, team, rest),
        ("work", "teamsettings") => team_settings(state, method, org, project, team, rest, body),
        ("dashboard", "dashboards") => dashboards(state, method, org, project, team, rest, body),
        ("serviceendpoint", "endpoints") => endpoints(state, method, org, project, rest, body),
        _ => return None,
    };
    Some(response)
}

fn boards(_state: &AppState, method: &Method, org: &str, project: &str, team: Option<&str>, rest: &[String]) -> Response {
    if method != Method::GET {
        return fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée");
    }
    let team_name = team.unwrap_or("AzurzDemo Team");
    if rest.len() >= 4 && rest[3].eq_ignore_ascii_case("columns") {
        return Json(page(board_columns())).into_response();
    }
    if rest.len() >= 3 {
        return Json(board_document(org, project, team_name, &rest[2])).into_response();
    }
    Json(page(vec![board_document(org, project, team_name, "Stories")])).into_response()
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
        if method != Method::GET {
            return fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée");
        }
        let value = state.read(|db| {
            db.project(org, project)
                .map(|stored| stored.list_bucket("work/teamsettings/iterations"))
                .unwrap_or_default()
        });
        if rest.len() >= 4 {
            let id = &rest[3];
            return match value.into_iter().find(|item| item_id(item) == *id || item_text(item, "name").as_deref() == Some(id.as_str())) {
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
        return team_members(state, method, org, project_key, &team_id, body);
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

fn team_members(state: &AppState, method: &Method, org: &str, project_key: &str, team_id: &str, body: &[u8]) -> Response {
    let key = format!("core/teams/{team_id}/members");
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
    fail(StatusCode::NOT_FOUND, "tableau de bord introuvable")
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
            widgets.remove(index);
            save_widgets(state, org, project, dashboard, widgets);
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

fn endpoints(state: &AppState, method: &Method, org: &str, project: &str, rest: &[String], body: &[u8]) -> Response {
    let key = "serviceendpoint/endpoints";
    if rest.len() == 2 && method == Method::GET {
        let value = state.read(|db| db.project(org, project).map(|stored| stored.list_bucket(key)).unwrap_or_default());
        return Json(page(value)).into_response();
    }
    if rest.len() == 2 && method == Method::POST {
        let payload = parse_json(body).unwrap_or(json!({}));
        let created = state.write(|db| {
            db.ensure_project(org, project);
            let stored = db.project_mut(org, project)?;
            let next = stored.list_bucket(key).len() + 1;
            let name = payload.get("name").and_then(|value| value.as_str()).unwrap_or("endpoint");
            let item = json!({
                "id": format!("endpoint-{next:08}"),
                "name": name,
                "type": payload.get("type").cloned().unwrap_or(json!("generic")),
                "url": payload.get("url").cloned().unwrap_or(json!("")),
                "isReady": true,
                "owner": "library",
                "description": payload.get("description").cloned().unwrap_or(json!("")),
                "authorization": {"scheme": "UsernamePassword"},
                "data": payload.get("data").cloned().unwrap_or(json!({})),
                "serviceEndpointProjectReferences": [{"projectReference": {"name": project}, "name": name}]
            });
            Some(stored.put_bucket(key, item))
        });
        return match created {
            Some(item) => Json(item).into_response(),
            None => fail(StatusCode::NOT_FOUND, "projet inconnu"),
        };
    }
    if method == Method::GET {
        let id = rest.get(2).map(String::as_str).unwrap_or("");
        return match state.read(|db| db.project(org, project).and_then(|stored| stored.get_bucket(key, id))) {
            Some(item) => Json(item).into_response(),
            None => fail(StatusCode::NOT_FOUND, "connexion introuvable"),
        };
    }
    if method == Method::DELETE {
        let id = rest.get(2).map(String::as_str).unwrap_or("").to_string();
        let removed = state.write(|db| {
            db.project_mut(org, project)
                .and_then(|stored| stored.buckets.get_mut(key))
                .map(|bucket| bucket.remove(&id))
                .unwrap_or(false)
        });
        return if removed {
            StatusCode::NO_CONTENT.into_response()
        } else {
            fail(StatusCode::NOT_FOUND, "connexion introuvable")
        };
    }
    fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée")
}

fn board_document(org: &str, project: &str, team: &str, name: &str) -> Value {
    json!({
        "id": name.to_ascii_lowercase(),
        "name": name,
        "url": format!("/{org}/{project}/{team}/_apis/work/boards/{name}"),
        "columns": board_columns(),
        "rows": [{"id": "row-1", "name": null}],
        "allowedMappings": {"User Story": ["New", "Active", "Resolved", "Closed"], "Bug": ["New", "Active", "Resolved", "Closed"]}
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

fn page(value: Vec<Value>) -> Value {
    json!({"count": value.len(), "value": value})
}
