use axum::body::Bytes;
use axum::extract::State;
use axum::http::{header, HeaderMap, Method, StatusCode, Uri};
use axum::response::{Html, IntoResponse, Response};
use axum::Json;
use base64::Engine;
use serde_json::{json, Map, Value};

use crate::store::{apply_wiql, fields_from_patch, Bucket, RESOURCE_AREAS};
use crate::web::{login_page, shell_page};
use crate::AppState;

pub async fn dispatch(
    State(state): State<AppState>,
    method: Method,
    uri: Uri,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let path = uri.path().to_string();
    let query = uri.query().unwrap_or("").to_string();
    if path == "/health" {
        let (org, project) = state.names();
        return Json(json!({"status": "ok", "organization": org, "project": project})).into_response();
    }
    if path == "/_signin" && method == Method::POST {
        return sign_in(&state, &body);
    }
    if path == "/_signout" && method == Method::POST {
        return sign_out(&state, &headers);
    }
    if path == "/_hooks/inbox" && method == Method::POST {
        return crate::shapes::receive_hook(&state, &body);
    }

    let auth = state.identify(&headers);
    if path_is_api(&path) {
        if !matches!(auth, Identity::User(_)) {
            return fail(StatusCode::UNAUTHORIZED, "PAT refusé");
        }
        return state.api(method, &path, &query, &body);
    }
    match auth {
        Identity::User(user) => {
            let (org, project) = scope_from_page(&path, &state);
            Html(shell_page(&org, &project, &user)).into_response()
        }
        Identity::Rejected => (
            StatusCode::UNAUTHORIZED,
            Html(login_page(&state.names().0, true)),
        )
            .into_response(),
        Identity::Anonymous => Html(login_page(&state.names().0, false)).into_response(),
    }
}

fn work_item_updates(id: i64, history: &[Value]) -> Vec<Value> {
    let mut previous_fields = Value::Object(Map::new());
    let mut previous_relations = Value::Array(Vec::new());
    let mut updates = Vec::new();
    for (index, snapshot) in history.iter().enumerate() {
        let fields = snapshot.get("fields").cloned().unwrap_or_else(|| Value::Object(Map::new()));
        let relations = snapshot.get("relations").cloned().unwrap_or_else(|| Value::Array(Vec::new()));
        let rev = snapshot.get("rev").and_then(|value| value.as_i64()).unwrap_or(index as i64 + 1);
        let mut record = json!({
            "id": index as i64 + 1,
            "workItemId": id,
            "rev": rev,
            "revisedBy": {"displayName": "azurz", "uniqueName": "azurz"},
            "revisedDate": "2026-10-29T00:00:00Z",
            "fields": diff_fields(&previous_fields, &fields)
        });
        if let Some(relations) = diff_relations(&previous_relations, &relations) {
            record["relations"] = relations;
        }
        updates.push(record);
        previous_fields = fields;
        previous_relations = relations;
    }
    updates
}

fn diff_fields(before: &Value, after: &Value) -> Value {
    let before = before.as_object();
    let after = after.as_object();
    let mut names = std::collections::BTreeSet::new();
    if let Some(before) = before {
        names.extend(before.keys().cloned());
    }
    if let Some(after) = after {
        names.extend(after.keys().cloned());
    }
    let mut fields = Map::new();
    for name in names {
        let old = before.and_then(|object| object.get(&name));
        let new = after.and_then(|object| object.get(&name));
        if old == new {
            continue;
        }
        let mut change = Map::new();
        if let Some(old) = old {
            change.insert("oldValue".into(), old.clone());
        }
        if let Some(new) = new {
            change.insert("newValue".into(), new.clone());
        }
        fields.insert(name, Value::Object(change));
    }
    Value::Object(fields)
}

fn diff_relations(before: &Value, after: &Value) -> Option<Value> {
    let before = before.as_array().map(|rows| rows.as_slice()).unwrap_or(&[]);
    let after = after.as_array().map(|rows| rows.as_slice()).unwrap_or(&[]);
    let mut added = Vec::new();
    let mut removed = Vec::new();
    let mut updated = Vec::new();
    for relation in after {
        match before.iter().find(|item| relation_identity(item) == relation_identity(relation)) {
            Some(previous) if previous != relation => updated.push(relation.clone()),
            None => added.push(relation.clone()),
            _ => {}
        }
    }
    for relation in before {
        if !after.iter().any(|item| relation_identity(item) == relation_identity(relation)) {
            removed.push(relation.clone());
        }
    }
    if added.is_empty() && removed.is_empty() && updated.is_empty() {
        None
    } else {
        Some(json!({"added": added, "removed": removed, "updated": updated}))
    }
}

fn relation_identity(relation: &Value) -> String {
    format!(
        "{} {}",
        relation.get("rel").and_then(|value| value.as_str()).unwrap_or(""),
        relation.get("url").and_then(|value| value.as_str()).unwrap_or("")
    )
}

fn sign_in(state: &AppState, body: &Bytes) -> Response {
    let form = form_pairs(std::str::from_utf8(body).unwrap_or(""));
    let username = form.get("username").cloned().unwrap_or_default();
    let password = form.get("password").cloned().unwrap_or_default();
    if !state.password_ok(&password) {
        return (
            StatusCode::UNAUTHORIZED,
            Html(login_page(&state.names().0, true)),
        )
            .into_response();
    }
    let token = state.open_session(if username.is_empty() {
        state.names().0
    } else {
        username
    });
    let (org, project) = state.names();
    redirect(
        &format!("/{org}/{project}"),
        &format!("azdo_session={token}; HttpOnly; Path=/; SameSite=Lax"),
    )
}

fn sign_out(state: &AppState, headers: &HeaderMap) -> Response {
    if let Some(token) = cookie_token(headers) {
        state.close_session(&token);
    }
    redirect("/", "azdo_session=; HttpOnly; Path=/; Max-Age=0; SameSite=Lax")
}

fn redirect(location: &str, cookie: &str) -> Response {
    Response::builder()
        .status(StatusCode::SEE_OTHER)
        .header(header::LOCATION, location)
        .header(header::SET_COOKIE, cookie)
        .body(axum::body::Body::empty())
        .unwrap_or_else(|_| StatusCode::NO_CONTENT.into_response())
}

fn path_is_api(path: &str) -> bool {
    path.split('/').any(|part| part == "_apis" || part == "_packaging")
}

fn scope_from_page(path: &str, state: &AppState) -> (String, String) {
    let (default_org, default_project) = state.names();
    let parts: Vec<String> = path
        .split('/')
        .filter(|part| !part.is_empty())
        .map(pct_decode)
        .collect();
    let org = parts.first().cloned().unwrap_or(default_org);
    let project = parts.get(1).cloned().unwrap_or(default_project);
    if project.starts_with('_') {
        (org, state.names().1)
    } else {
        (org, project)
    }
}

pub fn fail(status: StatusCode, message: &str) -> Response {
    (
        status,
        Json(json!({
            "message": message,
            "detail": message,
            "typeKey": "AzureDevOpsLab",
            "errorCode": status.as_u16()
        })),
    )
        .into_response()
}

impl AppState {
    pub fn api(&self, method: Method, path: &str, query: &str, body: &[u8]) -> Response {
        if path.split('/').any(|part| part == "_packaging") {
            return crate::surface::packaging(self, &method, path, body);
        }
        let Some((org, project, team, rest)) = split_api(path) else {
            return fail(StatusCode::NOT_FOUND, "route inconnue");
        };
        if rest.is_empty() {
            return fail(StatusCode::NOT_FOUND, "ressource absente");
        }
        let head = rest[0].to_ascii_lowercase();
        if head == "resourceareas" {
            let org_name = org.unwrap_or_else(|| self.names().0);
            let value: Vec<Value> = RESOURCE_AREAS
                .iter()
                .map(|area| {
                    json!({
                        "id": area,
                        "name": area,
                        "locationUrl": format!("/{org_name}/_apis/{area}")
                    })
                })
                .collect();
            return Json(json!({"count": value.len(), "value": value})).into_response();
        }
        if head == "connectiondata" {
            return Json(json!({
                "authenticatedUser": {"providerDisplayName": "azurz", "customDisplayName": "azurz"},
                "authorizedUser": {"providerDisplayName": "azurz"},
                "instanceId": "azdo-linux",
                "deploymentType": "hosted"
            }))
            .into_response();
        }
        let org = match org {
            Some(org) => org,
            None => return fail(StatusCode::NOT_FOUND, "organisation absente"),
        };
        if head == "projects" {
            return self.projects(&method, &org, &rest, body);
        }
        let Some(project) = project else {
            if let Some(response) = crate::shapes::org_dispatch(self, &method, &org, &rest, query, body) {
                return response;
            }
            if let Some(response) = crate::surface::org(self, &method, &org, &rest, body) {
                return response;
            }
            return fail(StatusCode::NOT_FOUND, "projet absent");
        };
        if head == "wit" {
            return self.wit(&method, &org, &project, &rest, query, body);
        }
        if let Some(response) = crate::surface::project(self, &method, &org, &project, &rest, query, body) {
            return response;
        }
        if let Some(response) = crate::tracking::dispatch(self, &method, &org, &project, team.as_deref(), &rest, body) {
            return response;
        }
        if let Some(response) = crate::shapes::dispatch(self, &method, &org, &project, &rest, query, body) {
            return response;
        }
        self.collection(&method, &org, &project, &rest, body)
    }

    fn projects(&self, method: &Method, org: &str, rest: &[String], body: &[u8]) -> Response {
        if rest.len() == 1 && method == Method::GET {
            let value = self.read(|db| {
                db.orgs
                    .get(org)
                    .map(|organization| {
                        organization
                            .projects
                            .iter()
                            .map(|(name, project)| project_json(org, name, project))
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default()
            });
            return Json(json!({"count": value.len(), "value": value})).into_response();
        }
        if rest.len() == 1 && method == Method::POST {
            let payload = parse_json(body).unwrap_or(json!({}));
            let name = payload
                .get("name")
                .and_then(|value| value.as_str())
                .unwrap_or("")
                .to_string();
            if name.is_empty() {
                return fail(StatusCode::BAD_REQUEST, "nom de projet absent");
            }
            let description = payload
                .get("description")
                .and_then(|value| value.as_str())
                .unwrap_or("Projet")
                .to_string();
            let created = self.write(|db| {
                db.ensure_project(org, &name);
                if let Some(project) = db.project_mut(org, &name) {
                    if !description.is_empty() {
                        project.description = description;
                    }
                }
                db.project(org, &name)
                    .map(|project| project_json(org, &name, project))
                    .unwrap_or(json!({}))
            });
            return Json(created).into_response();
        }
        if rest.len() >= 3 && rest[2].eq_ignore_ascii_case("teams") {
            return crate::tracking::teams(self, method, org, &rest[1], &rest[3..], body);
        }
        if rest.len() >= 2 && method == Method::GET {
            let key = &rest[1];
            let found = self.read(|db| {
                let name = db.project_name(org, key)?;
                let project = db.project(org, key)?;
                Some(project_json(org, &name, project))
            });
            return match found {
                Some(project) => Json(project).into_response(),
                None => fail(StatusCode::NOT_FOUND, "projet inconnu"),
            };
        }
        fail(StatusCode::NOT_FOUND, "route projet inconnue")
    }

    fn wit(
        &self,
        method: &Method,
        org: &str,
        project: &str,
        rest: &[String],
        query: &str,
        body: &[u8],
    ) -> Response {
        let resource = rest.get(1).map(|value| value.to_ascii_lowercase());
        if resource.as_deref() == Some("classificationnodes") {
            return crate::shapes::classification_nodes(self, method, org, project, rest, query, body);
        }
        if let Some(response) = crate::shapes::wit_catalog(method, org, project, rest) {
            return response;
        }
        if resource.as_deref() == Some("wiql") && method == Method::POST {
            let payload = parse_json(body).unwrap_or(json!({}));
            let mut text = payload.get("query").and_then(|value| value.as_str()).unwrap_or("").to_string();
            if text.is_empty() {
                if let Some(id) = rest.get(2) {
                    text = self
                        .read(|db| {
                            db.project(org, project)
                                .and_then(|stored| stored.get_bucket("wit/queries", id))
                                .and_then(|query| query.get("wiql").and_then(|value| value.as_str()).map(str::to_string))
                        })
                        .unwrap_or_default();
                }
            }
            let work_items = self.read(|db| {
                db.project(org, project)
                    .map(|stored| apply_wiql(stored.work_items.values(), &text))
                    .unwrap_or_default()
            });
            return Json(json!({"queryType": "flat", "workItems": work_items})).into_response();
        }
        if resource.as_deref() == Some("attachments") {
            return self.attachments(method, org, project, rest, query, body);
        }
        if resource.as_deref() == Some("queries") {
            return self.queries(method, org, project, rest, query, body);
        }
        if resource.as_deref() == Some("workitemsbatch") && method == Method::POST {
            return crate::surface::work_items_batch(self, org, project, body);
        }
        if resource.as_deref() == Some("recyclebin") {
            return self.recycle_bin(method, org, project, rest, body);
        }
        if resource.as_deref() != Some("workitems") {
            return self.collection(method, org, project, rest, body);
        }
        if rest.len() >= 3 && rest[2].starts_with('$') && method == Method::POST {
            let kind = rest[2].trim_start_matches('$').replace("%20", " ");
            let ops = parse_json(body).unwrap_or(json!([]));
            let ops = ops.as_array().cloned().unwrap_or_default();
            let ops = match crate::shapes::prepare_work_item_ops(self, org, project, &kind, &ops) {
                Ok(ops) => ops,
                Err(message) => return fail(StatusCode::BAD_REQUEST, message),
            };
            let fields = match fields_from_patch(&ops) {
                Ok(fields) => fields,
                Err(message) => return fail(StatusCode::BAD_REQUEST, &message),
            };
            let item = self.write(|db| {
                db.ensure_project(org, project);
                db.project_mut(org, project)
                    .map(|stored| stored.create_work_item(project, &kind, fields))
            });
            return match item {
                Some(item) => {
                    crate::shapes::deliver_work_item(self, org, project, "workitem.created", &item);
                    Json(item).into_response()
                }
                None => fail(StatusCode::NOT_FOUND, "projet inconnu"),
            };
        }
        if rest.len() >= 4 && rest[3].eq_ignore_ascii_case("updates") {
            let Ok(id) = rest[2].parse::<i64>() else {
                return fail(StatusCode::NOT_FOUND, "work item introuvable");
            };
            let history = self.read(|db| {
                db.project(org, project)
                    .and_then(|stored| stored.history.get(&id).cloned())
                    .unwrap_or_default()
            });
            if history.is_empty() && self.read(|db| db.project(org, project).and_then(|stored| stored.work_items.get(&id).cloned())).is_none() {
                return fail(StatusCode::NOT_FOUND, "work item introuvable");
            }
            let updates = work_item_updates(id, &history);
            if let Some(update_id) = rest.get(4) {
                return match updates.iter().find(|item| item.get("id").and_then(|value| value.as_i64()).map(|value| value.to_string()).as_deref() == Some(update_id.as_str())) {
                    Some(item) => Json(item.clone()).into_response(),
                    None => fail(StatusCode::NOT_FOUND, "mise à jour introuvable"),
                };
            }
            return Json(json!({"count": updates.len(), "value": updates})).into_response();
        }
        if rest.len() >= 4 && rest[3].eq_ignore_ascii_case("revisions") {
            let Ok(id) = rest[2].parse::<i64>() else {
                return fail(StatusCode::NOT_FOUND, "work item introuvable");
            };
            let history = self.read(|db| {
                db.project(org, project)
                    .and_then(|stored| stored.history.get(&id).cloned())
                    .unwrap_or_default()
            });
            if history.is_empty() && self.read(|db| db.project(org, project).and_then(|stored| stored.work_items.get(&id).cloned())).is_none() {
                return fail(StatusCode::NOT_FOUND, "work item introuvable");
            }
            return Json(json!({"count": history.len(), "value": history})).into_response();
        }
        if rest.len() >= 4 && rest[3].eq_ignore_ascii_case("comments") {
            return self.work_item_comments(method, org, project, rest, body);
        }
        if rest.len() == 2 && method == Method::GET {
            let ids = query_param(query, "ids");
            let value = self.read(|db| {
                let Some(stored) = db.project(org, project) else {
                    return Vec::new();
                };
                if let Some(ids) = ids {
                    ids.split(',')
                        .filter_map(|id| id.parse::<i64>().ok())
                        .filter_map(|id| stored.work_items.get(&id).cloned())
                        .collect()
                } else {
                    stored.work_items.values().cloned().collect()
                }
            });
            return Json(json!({"count": value.len(), "value": value})).into_response();
        }
        if rest.len() >= 3 {
            let Ok(id) = rest[2].parse::<i64>() else {
                return fail(StatusCode::NOT_FOUND, "work item introuvable");
            };
            if method == Method::GET {
                let found = self.read(|db| db.project(org, project).and_then(|stored| stored.work_items.get(&id).cloned()));
                return match found {
                    Some(item) => Json(item).into_response(),
                    None => fail(StatusCode::NOT_FOUND, "work item introuvable"),
                };
            }
            if method == Method::DELETE {
                let destroy = query_param(query, "destroy")
                    .is_some_and(|value| value.trim().eq_ignore_ascii_case("true"));
                let removed = self.write(|db| {
                    db.project_mut(org, project)
                        .and_then(|stored| stored.delete_work_item(project, id, destroy).ok())
                });
                return match removed {
                    Some(record) => {
                        if let Some(item) = record.get("resource") {
                            crate::shapes::deliver_work_item(self, org, project, "workitem.deleted", item);
                        }
                        Json(record).into_response()
                    }
                    None => fail(StatusCode::NOT_FOUND, "work item introuvable"),
                };
            }
            if method == Method::PATCH {
                let exists = self.read(|db| {
                    db.project(org, project)
                        .and_then(|stored| stored.work_items.get(&id).cloned())
                        .is_some()
                });
                if !exists {
                    return fail(StatusCode::NOT_FOUND, "work item introuvable");
                }
                let ops = parse_json(body).unwrap_or(json!([])).as_array().cloned().unwrap_or_default();
                let kind = self.read(|db| {
                    db.project(org, project)
                        .and_then(|stored| stored.work_items.get(&id))
                        .and_then(|item| item.get("fields"))
                        .and_then(|fields| fields.get("System.WorkItemType"))
                        .and_then(|value| value.as_str())
                        .unwrap_or("User Story")
                        .to_string()
                });
                let ops = match crate::shapes::prepare_work_item_ops(self, org, project, &kind, &ops) {
                    Ok(ops) => ops,
                    Err(message) => return fail(StatusCode::BAD_REQUEST, message),
                };
                let updated = self.write(|db| {
                    db.project_mut(org, project)
                        .and_then(|stored| stored.patch_work_item(id, &ops).ok())
                });
                return match updated {
                    Some(item) => {
                        crate::shapes::deliver_work_item(self, org, project, "workitem.updated", &item);
                        Json(item).into_response()
                    }
                    None => fail(StatusCode::NOT_FOUND, "work item introuvable"),
                };
            }
        }
        fail(StatusCode::NOT_FOUND, "route work item inconnue")
    }

    fn work_item_comments(&self, method: &Method, org: &str, project: &str, rest: &[String], body: &[u8]) -> Response {
        let Some(id) = rest.get(2).and_then(|value| value.parse::<i64>().ok()) else {
            return fail(StatusCode::NOT_FOUND, "work item introuvable");
        };
        let comment_id = if rest.len() >= 5 {
            match rest[4].parse::<i64>() {
                Ok(value) => Some(value),
                Err(_) => return fail(StatusCode::NOT_FOUND, "commentaire introuvable"),
            }
        } else {
            None
        };
        let live = self.read(|db| {
            db.project(org, project)
                .and_then(|stored| stored.work_items.get(&id).cloned())
                .is_some()
        });
        if matches!(*method, Method::POST | Method::PATCH | Method::DELETE) && !live {
            return fail(StatusCode::NOT_FOUND, "work item introuvable");
        }
        if method == Method::POST {
            if comment_id.is_some() {
                return fail(StatusCode::NOT_FOUND, "commentaire introuvable");
            }
            let Some(text) = readable_comment(body) else {
                return fail(StatusCode::BAD_REQUEST, "commentaire illisible");
            };
            if text.trim().is_empty() {
                return fail(StatusCode::BAD_REQUEST, "commentaire absent");
            }
            let created = self.write(|db| {
                db.project_mut(org, project)
                    .map(|stored| stored.add_comment(id, &text))
            });
            return match created {
                Some(Ok(note)) => {
                    crate::shapes::deliver_work_item(self, org, project, "workitem.commented", &note);
                    Json(note).into_response()
                }
                _ => fail(StatusCode::NOT_FOUND, "work item introuvable"),
            };
        }
        if method == Method::PATCH || method == Method::DELETE {
            let Some(comment_id) = comment_id else {
                return fail(StatusCode::NOT_FOUND, "commentaire introuvable");
            };
            let present = self.read(|db| {
                db.project(org, project).is_some_and(|stored| {
                    stored.visible_comments(id).is_some_and(|notes| {
                        notes.iter().any(|note| note.get("id").and_then(|value| value.as_i64()) == Some(comment_id))
                    })
                })
            });
            if !present {
                return fail(StatusCode::NOT_FOUND, "commentaire introuvable");
            }
            if method == Method::DELETE {
                let removed = self.write(|db| {
                    db.project_mut(org, project)
                        .map(|stored| stored.delete_comment(id, comment_id))
                });
                return match removed {
                    Some(Ok(())) => StatusCode::NO_CONTENT.into_response(),
                    Some(Err(message)) => fail(StatusCode::NOT_FOUND, message),
                    None => fail(StatusCode::NOT_FOUND, "work item introuvable"),
                };
            }
            let Some(text) = readable_comment(body) else {
                return fail(StatusCode::BAD_REQUEST, "commentaire illisible");
            };
            if text.trim().is_empty() {
                return fail(StatusCode::BAD_REQUEST, "commentaire absent");
            }
            let updated = self.write(|db| {
                db.project_mut(org, project)
                    .map(|stored| stored.update_comment(id, comment_id, &text))
            });
            return match updated {
                Some(Ok(note)) => {
                    crate::shapes::deliver_work_item(self, org, project, "workitem.commented", &note);
                    Json(note).into_response()
                }
                Some(Err(message)) => fail(StatusCode::NOT_FOUND, message),
                None => fail(StatusCode::NOT_FOUND, "work item introuvable"),
            };
        }
        if method == Method::GET {
            if let Some(comment_id) = comment_id {
                let found = self.read(|db| {
                    db.project(org, project).and_then(|stored| {
                        stored.visible_comments(id).and_then(|notes| {
                            notes.into_iter().find(|note| note.get("id").and_then(|value| value.as_i64()) == Some(comment_id))
                        })
                    })
                });
                return match found {
                    Some(note) => Json(note).into_response(),
                    None => fail(StatusCode::NOT_FOUND, "commentaire introuvable"),
                };
            }
            let listed = self.read(|db| db.project(org, project).and_then(|stored| stored.visible_comments(id)));
            return match listed {
                Some(notes) => {
                    let count = notes.len();
                    Json(json!({"count": count, "totalCount": count, "comments": notes})).into_response()
                }
                None => fail(StatusCode::NOT_FOUND, "work item introuvable"),
            };
        }
        fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée")
    }

    fn recycle_bin(&self, method: &Method, org: &str, project: &str, rest: &[String], body: &[u8]) -> Response {
        if rest.len() == 2 && method == Method::GET {
            let value = self.read(|db| {
                db.project(org, project)
                    .map(|stored| {
                        stored
                            .recycle_bin
                            .values()
                            .map(crate::store::recycle_reference)
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default()
            });
            return Json(json!({"count": value.len(), "value": value})).into_response();
        }
        if rest.len() != 3 {
            return fail(StatusCode::NOT_FOUND, "élément supprimé introuvable");
        }
        let Ok(id) = rest[2].parse::<i64>() else {
            return fail(StatusCode::NOT_FOUND, "élément supprimé introuvable");
        };
        let present = self.read(|db| {
            db.project(org, project)
                .and_then(|stored| stored.recycle_bin.get(&id).cloned())
        });
        if method == Method::GET {
            return match present {
                Some(item) => Json(item).into_response(),
                None => fail(StatusCode::NOT_FOUND, "élément supprimé introuvable"),
            };
        }
        if present.is_none() {
            return fail(StatusCode::NOT_FOUND, "élément supprimé introuvable");
        }
        if method == Method::PATCH {
            let payload = parse_json(body).unwrap_or(Value::Null);
            let Some(object) = payload.as_object() else {
                return fail(StatusCode::BAD_REQUEST, "restauration illisible");
            };
            if object.get("isDeleted").and_then(|value| value.as_bool()) != Some(false) {
                return fail(StatusCode::BAD_REQUEST, "restauration refusée");
            }
            let restored = self.write(|db| {
                db.project_mut(org, project)
                    .and_then(|stored| stored.restore_work_item(id).ok())
            });
            return match restored {
                Some(item) => {
                    crate::shapes::deliver_work_item(self, org, project, "workitem.restored", &item);
                    Json(item).into_response()
                }
                None => fail(StatusCode::NOT_FOUND, "élément supprimé introuvable"),
            };
        }
        if method == Method::DELETE {
            let removed = self.write(|db| {
                db.project_mut(org, project)
                    .and_then(|stored| stored.destroy_recycled(id).ok())
            });
            return match removed {
                Some(()) => StatusCode::NO_CONTENT.into_response(),
                None => fail(StatusCode::NOT_FOUND, "élément supprimé introuvable"),
            };
        }
        fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée")
    }

    fn attachments(
        &self,
        method: &Method,
        org: &str,
        project: &str,
        rest: &[String],
        query: &str,
        body: &[u8],
    ) -> Response {
        if rest.len() == 2 && method == Method::POST {
            let name = query_param(query, "fileName")
                .map(pct_decode)
                .filter(|value| !value.is_empty())
                .unwrap_or_else(|| "fichier".into());
            let created = self.write(|db| {
                db.ensure_project(org, project);
                db.project_mut(org, project).map(|stored| stored.add_attachment(&name, body))
            });
            return match created {
                Some(item) => Json(item).into_response(),
                None => fail(StatusCode::NOT_FOUND, "projet inconnu"),
            };
        }
        if rest.len() >= 3 && method == Method::GET {
            let id = &rest[2];
            let found = self.read(|db| db.project(org, project).and_then(|stored| stored.attachment(id).cloned()));
            return match found {
                Some(file) => Response::builder()
                    .header(header::CONTENT_TYPE, file.content_type)
                    .body(axum::body::Body::from(file.bytes))
                    .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response()),
                None => fail(StatusCode::NOT_FOUND, "pièce jointe introuvable"),
            };
        }
        fail(StatusCode::NOT_FOUND, "pièce jointe introuvable")
    }

    fn queries(&self, method: &Method, org: &str, project: &str, rest: &[String], query: &str, body: &[u8]) -> Response {
        self.ensure_query_folders(org, project);
        if rest.len() == 2 && method == Method::GET {
            let depth = query_param(query, "$depth")
                .or_else(|| query_param(query, "depth"))
                .and_then(|value| value.parse::<i64>().ok())
                .unwrap_or(2);
            let items = self.query_items(org, project);
            let value = nest_queries(&items, "", depth.max(0) as usize);
            return Json(json!({"count": value.len(), "value": value})).into_response();
        }
        if rest.len() == 2 && method == Method::POST {
            let payload = parse_json(body).unwrap_or(json!({}));
            return self.create_query(org, project, "Shared Queries", &payload);
        }
        if rest.len() < 3 {
            return fail(StatusCode::NOT_FOUND, "requête introuvable");
        }
        let target = rest[2..].join("/");
        if method == Method::POST {
            let items = self.query_items(org, project);
            let Some(parent) = find_query(&items, &target).filter(|item| item.get("isFolder").and_then(|value| value.as_bool()).unwrap_or(false)) else {
                return fail(StatusCode::NOT_FOUND, "dossier introuvable");
            };
            let parent_path = parent.get("path").and_then(|value| value.as_str()).unwrap_or(&target).to_string();
            let payload = parse_json(body).unwrap_or(json!({}));
            return self.create_query(org, project, &parent_path, &payload);
        }
        let items = self.query_items(org, project);
        let Some(item) = find_query(&items, &target) else {
            return fail(StatusCode::NOT_FOUND, "requête introuvable");
        };
        let id = item.get("id").and_then(|value| value.as_str()).unwrap_or("").to_string();
        if method == Method::GET {
            let depth = query_param(query, "$depth").and_then(|value| value.parse::<i64>().ok()).unwrap_or(1);
            let path = item.get("path").and_then(|value| value.as_str()).unwrap_or("").to_string();
            let mut dressed = item;
            if dressed.get("isFolder").and_then(|value| value.as_bool()).unwrap_or(false) {
                let children = nest_queries(&items, &path, depth.max(0) as usize);
                if let Some(object) = dressed.as_object_mut() {
                    object.insert("hasChildren".into(), json!(!children.is_empty()));
                    object.insert("children".into(), json!(children));
                }
            }
            return Json(dressed).into_response();
        }
        if method == Method::PATCH || method == Method::PUT {
            let payload = parse_json(body).unwrap_or(json!({}));
            let mut updated = item;
            if let Some(object) = updated.as_object_mut() {
                if let Some(name) = payload.get("name").and_then(|value| value.as_str()) {
                    let path = object.get("path").and_then(|value| value.as_str()).unwrap_or("").to_string();
                    let parent = path.rsplit_once('/').map(|(parent, _)| parent.to_string()).unwrap_or_default();
                    object.insert("name".into(), json!(name));
                    object.insert(
                        "path".into(),
                        json!(if parent.is_empty() { name.to_string() } else { format!("{parent}/{name}") }),
                    );
                }
                if let Some(wiql) = payload.get("wiql").and_then(|value| value.as_str()) {
                    if object.get("isFolder").and_then(|value| value.as_bool()).unwrap_or(false) {
                        return fail(StatusCode::BAD_REQUEST, "un dossier n'a pas de WIQL");
                    }
                    object.insert("wiql".into(), json!(wiql));
                }
            }
            self.write(|db| {
                if let Some(stored) = db.project_mut(org, project) {
                    stored.put_bucket("wit/queries", updated.clone());
                }
            });
            return Json(updated).into_response();
        }
        if method == Method::DELETE {
            if id == "shared-queries" || id == "my-queries" {
                return fail(StatusCode::BAD_REQUEST, "dossier racine");
            }
            let removed = self.write(|db| {
                db.project_mut(org, project)
                    .and_then(|stored| stored.buckets.get_mut("wit/queries"))
                    .map(|bucket| bucket.remove(&id))
                    .unwrap_or(false)
            });
            return if removed {
                StatusCode::NO_CONTENT.into_response()
            } else {
                fail(StatusCode::NOT_FOUND, "requête introuvable")
            };
        }
        fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée")
    }

    fn ensure_query_folders(&self, org: &str, project: &str) {
        self.write(|db| {
            db.ensure_project(org, project);
            let Some(stored) = db.project_mut(org, project) else {
                return;
            };
            for (id, name, public) in [("shared-queries", "Shared Queries", true), ("my-queries", "My Queries", false)] {
                if stored.get_bucket("wit/queries", id).is_none() {
                    stored.put_bucket(
                        "wit/queries",
                        json!({
                            "id": id,
                            "name": name,
                            "path": name,
                            "isFolder": true,
                            "isPublic": public,
                            "hasChildren": false,
                            "queryType": "flat"
                        }),
                    );
                }
            }
        });
    }

    fn query_items(&self, org: &str, project: &str) -> Vec<Value> {
        self.read(|db| db.project(org, project).map(|stored| stored.list_bucket("wit/queries")).unwrap_or_default())
    }

    fn create_query(&self, org: &str, project: &str, parent_path: &str, payload: &Value) -> Response {
        let name = payload.get("name").and_then(|value| value.as_str()).unwrap_or("Requête").to_string();
        let folder = payload.get("isFolder").and_then(|value| value.as_bool()).unwrap_or(false);
        let wiql = payload
            .get("wiql")
            .and_then(|value| value.as_str())
            .unwrap_or("SELECT [System.Id] FROM WorkItems")
            .to_string();
        let created = self.write(|db| {
            db.ensure_project(org, project);
            let stored = db.project_mut(org, project)?;
            let next = stored.list_bucket("wit/queries").len() as i64 + 1;
            let path = if parent_path.is_empty() { name.clone() } else { format!("{parent_path}/{name}") };
            let item = if folder {
                json!({
                    "id": format!("folder-{next}"),
                    "name": name,
                    "path": path,
                    "isFolder": true,
                    "isPublic": true,
                    "hasChildren": false,
                    "queryType": "flat"
                })
            } else {
                json!({
                    "id": format!("query-{next}"),
                    "name": name,
                    "wiql": wiql,
                    "path": path,
                    "isFolder": false,
                    "isPublic": true,
                    "queryType": "flat"
                })
            };
            Some(stored.put_bucket("wit/queries", item))
        });
        match created {
            Some(item) => Json(item).into_response(),
            None => fail(StatusCode::NOT_FOUND, "projet inconnu"),
        }
    }

    fn collection(
        &self,
        method: &Method,
        org: &str,
        project: &str,
        rest: &[String],
        body: &[u8],
    ) -> Response {
        if rest.len() < 2 {
            return fail(StatusCode::NOT_FOUND, "ressource absente");
        }
        let item_id = item_id(rest);
        let bucket_key = nested_key(rest).unwrap_or_else(|| collection_key(rest));
        if method == Method::GET && item_id.is_none() {
            let value = self.read(|db| {
                db.project(org, project)
                    .and_then(|stored| stored.buckets.get(&bucket_key))
                    .map(Bucket::list)
                    .unwrap_or(json!({"count": 0, "value": []}))
            });
            return Json(value).into_response();
        }
        if method == Method::POST && item_id.is_none() {
            let payload = parse_json(body).unwrap_or(json!({}));
            let created = self.write(|db| {
                db.ensure_project(org, project);
                db.project_mut(org, project).map(|stored| {
                    let bucket = stored
                        .buckets
                        .entry(bucket_key.clone())
                        .or_insert_with(|| Bucket { items: Default::default(), next: 1 });
                    bucket.create(payload)
                })
            });
            return match created {
                Some(item) => Json(item).into_response(),
                None => fail(StatusCode::NOT_FOUND, "projet inconnu"),
            };
        }
        let Some(item_id) = item_id else {
            return fail(StatusCode::NOT_FOUND, "ressource absente");
        };
        if method == Method::GET {
            let found = self.read(|db| {
                db.project(org, project)
                    .and_then(|stored| stored.buckets.get(&bucket_key))
                    .and_then(|bucket| bucket.get(&item_id))
            });
            return match found {
                Some(item) => Json(item).into_response(),
                None => fail(StatusCode::NOT_FOUND, "ressource introuvable"),
            };
        }
        if method == Method::PATCH || method == Method::PUT {
            let payload = parse_json(body).unwrap_or(json!({}));
            let updated = self.write(|db| {
                db.project_mut(org, project)
                    .and_then(|stored| stored.buckets.get_mut(&bucket_key))
                    .and_then(|bucket| bucket.patch(&item_id, &payload))
            });
            return match updated {
                Some(item) => Json(item).into_response(),
                None => fail(StatusCode::NOT_FOUND, "ressource introuvable"),
            };
        }
        if method == Method::DELETE {
            let removed = self.write(|db| {
                db.project_mut(org, project)
                    .and_then(|stored| stored.buckets.get_mut(&bucket_key))
                    .map(|bucket| bucket.remove(&item_id))
                    .unwrap_or(false)
            });
            return if removed {
                StatusCode::NO_CONTENT.into_response()
            } else {
                fail(StatusCode::NOT_FOUND, "ressource introuvable")
            };
        }
        fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée")
    }
}

fn collection_key(rest: &[String]) -> String {
    format!(
        "{}/{}",
        rest[0].to_ascii_lowercase(),
        rest[1].to_ascii_lowercase()
    )
}

fn item_id(rest: &[String]) -> Option<String> {
    if rest.len() >= 3 && !rest[2].starts_with('$') && rest.len() < 4 {
        Some(rest[2].clone())
    } else if rest.len() >= 5 {
        Some(rest[4..].join("/"))
    } else {
        None
    }
}

fn nested_key(rest: &[String]) -> Option<String> {
    if rest.len() >= 4 {
        Some(format!(
            "{}/{}/{}/{}",
            rest[0].to_ascii_lowercase(),
            rest[1].to_ascii_lowercase(),
            rest[2],
            rest[3].to_ascii_lowercase()
        ))
    } else {
        None
    }
}

fn find_query(items: &[Value], target: &str) -> Option<Value> {
    items.iter().find(|item| {
        item.get("id").and_then(|value| value.as_str()) == Some(target)
            || item.get("name").and_then(|value| value.as_str()) == Some(target)
            || item.get("path").and_then(|value| value.as_str()) == Some(target)
    }).cloned()
}

fn nest_queries(items: &[Value], parent: &str, depth: usize) -> Vec<Value> {
    if depth == 0 {
        return Vec::new();
    }
    items
        .iter()
        .filter(|item| is_direct_query_child(parent, item.get("path").and_then(|value| value.as_str()).unwrap_or("")))
        .map(|item| {
            let mut item = item.clone();
            let path = item.get("path").and_then(|value| value.as_str()).unwrap_or("").to_string();
            if item.get("isFolder").and_then(|value| value.as_bool()).unwrap_or(false) {
                let children = nest_queries(items, &path, depth - 1);
                if let Some(object) = item.as_object_mut() {
                    object.insert("hasChildren".into(), json!(!children.is_empty()));
                    object.insert("children".into(), json!(children));
                }
            }
            item
        })
        .collect()
}

fn is_direct_query_child(parent: &str, path: &str) -> bool {
    if path.is_empty() {
        return false;
    }
    if parent.is_empty() {
        return !path.contains('/');
    }
    path.strip_prefix(parent)
        .and_then(|rest| rest.strip_prefix('/'))
        .is_some_and(|rest| !rest.is_empty() && !rest.contains('/'))
}

fn project_json(org: &str, name: &str, project: &crate::store::ProjectData) -> Value {
    json!({
        "id": project.id,
        "name": name,
        "description": project.description,
        "state": "wellFormed",
        "visibility": "private",
        "url": format!("/{org}/_apis/projects/{name}")
    })
}

fn split_api(path: &str) -> Option<(Option<String>, Option<String>, Option<String>, Vec<String>)> {
    let parts: Vec<String> = path
        .split('/')
        .filter(|part| !part.is_empty())
        .map(pct_decode)
        .collect();
    let index = parts.iter().position(|part| part == "_apis")?;
    let rest = parts[index + 1..].to_vec();
    if index == 3 {
        Some((Some(parts[0].clone()), Some(parts[1].clone()), Some(parts[2].clone()), rest))
    } else if index == 2 {
        Some((Some(parts[0].clone()), Some(parts[1].clone()), None, rest))
    } else if index == 1 {
        Some((Some(parts[0].clone()), None, None, rest))
    } else if index == 0 {
        Some((None, None, None, rest))
    } else {
        None
    }
}

fn readable_comment(body: &[u8]) -> Option<String> {
    parse_json(body)?
        .as_object()
        .and_then(|object| object.get("text"))
        .and_then(|value| value.as_str())
        .map(str::to_string)
}

pub(crate) fn parse_json(body: &[u8]) -> Option<Value> {
    if body.is_empty() {
        return Some(Value::Null);
    }
    serde_json::from_slice(body).ok()
}

pub(crate) fn query_param<'a>(query: &'a str, key: &str) -> Option<&'a str> {
    query.split('&').find_map(|pair| {
        let (name, value) = pair.split_once('=')?;
        if pct_decode(name) == key {
            Some(value)
        } else {
            None
        }
    })
}

fn form_pairs(body: &str) -> std::collections::BTreeMap<String, String> {
    body.split('&')
        .filter_map(|pair| {
            let (name, value) = pair.split_once('=')?;
            Some((pct_decode(name), pct_decode(value).replace('+', " ")))
        })
        .collect()
}

pub fn pct_decode(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' && index + 2 < bytes.len() {
            if let Ok(value) = u8::from_str_radix(&input[index + 1..index + 3], 16) {
                out.push(value);
                index += 3;
                continue;
            }
        }
        out.push(bytes[index]);
        index += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

pub fn basic_credentials(headers: &HeaderMap) -> Option<(String, String)> {
    let header = headers.get(header::AUTHORIZATION)?.to_str().ok()?;
    let encoded = header
        .strip_prefix("Basic ")
        .or_else(|| header.strip_prefix("basic "))?
        .trim();
    let mut padded = encoded.to_string();
    while padded.len() % 4 != 0 {
        padded.push('=');
    }
    let bytes = base64::engine::general_purpose::STANDARD.decode(padded).ok()?;
    let text = String::from_utf8(bytes).ok()?;
    let (user, password) = text.split_once(':')?;
    Some((user.to_string(), password.to_string()))
}

pub fn cookie_token(headers: &HeaderMap) -> Option<String> {
    let value = headers.get(header::COOKIE)?.to_str().ok()?;
    value.split(';').find_map(|part| {
        part.trim()
            .strip_prefix("azdo_session=")
            .map(|token| token.to_string())
    })
}

pub enum Identity {
    User(String),
    Anonymous,
    Rejected,
}

impl AppState {
    pub fn identify(&self, headers: &HeaderMap) -> Identity {
        if headers.get(header::AUTHORIZATION).is_some() {
            return match basic_credentials(headers) {
                Some((user, password)) if self.password_ok(&password) => {
                    Identity::User(if user.is_empty() { self.names().0 } else { user })
                }
                _ => Identity::Rejected,
            };
        }
        if let Some(token) = cookie_token(headers) {
            if let Some(user) = self.session_user(&token) {
                return Identity::User(user);
            }
        }
        Identity::Anonymous
    }
}
