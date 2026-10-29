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
        ("work", "teamsettings") => team_settings(state, method, org, project, rest, body),
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

fn team_settings(state: &AppState, method: &Method, org: &str, project: &str, rest: &[String], body: &[u8]) -> Response {
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
    if method != Method::GET {
        return fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée");
    }
    Json(json!({
        "backlogIteration": {"name": project, "path": format!("{project}\\")},
        "defaultIteration": {"name": "Sprint 1"},
        "defaultIterationMacro": "@currentIteration"
    }))
    .into_response()
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
        let value = state.read(|db| db.project(org, project).map(|stored| stored.list_bucket(key)).unwrap_or_default());
        return Json(page(value)).into_response();
    }
    if rest.len() == 2 && method == Method::POST {
        let payload = parse_json(body).unwrap_or(json!({}));
        let created = state.write(|db| {
            db.ensure_project(org, project);
            let stored = db.project_mut(org, project)?;
            let next = stored.list_bucket(key).len() + 1;
            let name = payload.get("name").and_then(|value| value.as_str()).unwrap_or("Tableau de bord");
            let widgets = payload.get("widgets").cloned().unwrap_or(json!([]));
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
            Some(item) => Json(item).into_response(),
            None => fail(StatusCode::NOT_FOUND, "projet inconnu"),
        };
    }
    if method == Method::GET {
        let id = rest.get(2).map(String::as_str).unwrap_or("");
        return match state.read(|db| db.project(org, project).and_then(|stored| stored.get_bucket(key, id))) {
            Some(item) => Json(item).into_response(),
            None => fail(StatusCode::NOT_FOUND, "tableau de bord introuvable"),
        };
    }
    fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée")
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
