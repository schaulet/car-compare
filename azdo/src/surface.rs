use axum::http::{header, Method, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::{json, Map, Value};

use crate::http::{fail, parse_json, pct_decode};
use crate::AppState;

pub fn org(state: &AppState, method: &Method, org: &str, rest: &[String], body: &[u8]) -> Option<Response> {
    let head = rest.first()?.to_ascii_lowercase();
    let response = match head.as_str() {
        "search" => search(state, method, org, None, rest, body),
        "accesscontrollists" | "accesscontrolentries" => acls(state, method, org, rest, body),
        "security" => permissions(state, method, org, rest, body),
        _ => return None,
    };
    Some(response)
}

pub fn project(state: &AppState, method: &Method, org: &str, project: &str, rest: &[String], query: &str, body: &[u8]) -> Option<Response> {
    let head = rest.first()?.to_ascii_lowercase();
    let response = match head.as_str() {
        "tfvc" => tfvc(state, method, org, project, rest, query, body),
        "search" => search(state, method, org, Some(project), rest, body),
        "packaging" if rest.get(3).is_some_and(|value| value.eq_ignore_ascii_case("upack")) => {
            upack(state, method, org, project, rest, body)
        }
        "packaging"
            if rest.get(3).is_some_and(|value| value.eq_ignore_ascii_case("maven") || value.eq_ignore_ascii_case("pypi"))
                && rest.get(4).is_some_and(|value| value.eq_ignore_ascii_case("packages")) =>
        {
            protocol_packages(state, method, org, project, rest)
        }
        _ => return None,
    };
    Some(response)
}

pub fn packaging(state: &AppState, method: &Method, path: &str, body: &[u8]) -> Response {
    let parts: Vec<String> = path.split('/').filter(|part| !part.is_empty()).map(|part| pct_decode(part)).collect();
    let Some(index) = parts.iter().position(|part| part == "_packaging") else {
        return fail(StatusCode::NOT_FOUND, "flux introuvable");
    };
    if index < 2 {
        return fail(StatusCode::NOT_FOUND, "flux introuvable");
    }
    let org = &parts[index - 2];
    let project = &parts[index - 1];
    let feed_name = parts.get(index + 1).map(String::as_str).unwrap_or("");
    let Some(feed) = find_feed(state, org, project, feed_name) else {
        return fail(StatusCode::NOT_FOUND, "flux introuvable");
    };
    let protocol = parts.get(index + 2).map(|value| value.to_ascii_lowercase()).unwrap_or_default();
    let tail = if parts.len() > index + 3 { &parts[index + 3..] } else { &[] };
    let feed_id = value_text(&feed["id"]);
    if protocol == "nuget" {
        return nuget(state, method, org, project, &feed_id, feed_name, tail, body);
    }
    if protocol == "npm" && (method == Method::GET || method == Method::PUT) {
        let name = parts.get(index + 3).cloned().unwrap_or_default();
        return npm_package(state, method, org, project, &feed_id, &name, body);
    }
    if protocol == "maven" {
        return maven(state, method, org, project, &feed_id, tail, body);
    }
    if protocol == "pypi" {
        return pypi(state, method, org, project, &feed_id, tail, body);
    }
    fail(StatusCode::NOT_FOUND, "protocole inconnu")
}

fn tfvc(state: &AppState, method: &Method, org: &str, project: &str, rest: &[String], query: &str, body: &[u8]) -> Response {
    let resource = rest.get(1).map(|value| value.to_ascii_lowercase()).unwrap_or_default();
    if resource == "changesets" {
        return changesets(state, method, org, project, rest, body);
    }
    if resource == "items" && method == Method::GET {
        ensure_tfvc_root(state, org, project);
        let path = crate::http::query_param(query, "path").map(pct_decode).unwrap_or_else(|| "$/".into());
        let recursion = crate::http::query_param(query, "recursionLevel").is_some();
        let rows = list_project(state, org, project, "tfvc/items");
        if recursion || path == "$/" {
            let value = if path == "$/" {
                rows
            } else {
                rows.into_iter().filter(|item| text(item, "path").unwrap_or_default().starts_with(&path)).collect()
            };
            return Json(page(value)).into_response();
        }
        return match rows.into_iter().find(|item| text(item, "path").as_deref() == Some(path.as_str())) {
            Some(item) => Json(item).into_response(),
            None => fail(StatusCode::NOT_FOUND, "élément TFVC introuvable"),
        };
    }
    fail(StatusCode::NOT_FOUND, "ressource TFVC introuvable")
}

fn changesets(state: &AppState, method: &Method, org: &str, project: &str, rest: &[String], body: &[u8]) -> Response {
    let key = "tfvc/changesets";
    if rest.len() == 2 && method == Method::GET {
        return Json(page(list_project(state, org, project, key))).into_response();
    }
    if rest.len() == 2 && method == Method::POST {
        let payload = parse_json(body).unwrap_or(json!({}));
        let created = state.write(|db| {
            db.ensure_project(org, project);
            let stored = db.project_mut(org, project)?;
            let next = stored.list_bucket(key).len() as i64 + 1;
            let changes = payload.get("changes").cloned().unwrap_or(json!([]));
            let item = json!({
                "id": next,
                "changesetId": next,
                "comment": text(&payload, "comment").unwrap_or_default(),
                "author": {"displayName": "azurz", "uniqueName": "azurz"},
                "createdDate": "2026-10-29T00:00:00Z",
                "url": format!("/{org}/{project}/_apis/tfvc/changesets/{next}"),
                "changes": changes.clone()
            });
            if let Some(rows) = changes.as_array() {
                for change in rows {
                    let path = change.get("item").and_then(|item| text(item, "path")).unwrap_or_default();
                    if path.is_empty() {
                        continue;
                    }
                    let content = change.get("item").and_then(|item| text(item, "content")).unwrap_or_default();
                    stored.put_bucket(
                        "tfvc/items",
                        json!({
                            "id": path,
                            "path": path,
                            "isFolder": path.ends_with('/'),
                            "content": content,
                            "changeSetId": next
                        }),
                    );
                }
            }
            Some(stored.put_bucket(key, item))
        });
        return match created {
            Some(item) => Json(item).into_response(),
            None => fail(StatusCode::NOT_FOUND, "projet inconnu"),
        };
    }
    let id = rest.get(2).map(String::as_str).unwrap_or("");
    if rest.len() >= 4 && rest[3].eq_ignore_ascii_case("changes") && method == Method::GET {
        let changes = get_project(state, org, project, key, id)
            .and_then(|item| item.get("changes").and_then(|value| value.as_array()).cloned())
            .unwrap_or_default();
        return Json(page(changes)).into_response();
    }
    if method == Method::GET {
        return match get_project(state, org, project, key, id) {
            Some(item) => Json(item).into_response(),
            None => fail(StatusCode::NOT_FOUND, "changeset introuvable"),
        };
    }
    fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée")
}

fn search(state: &AppState, method: &Method, org: &str, project: Option<&str>, rest: &[String], body: &[u8]) -> Response {
    if method != Method::POST {
        return fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée");
    }
    let kind = rest.get(1).map(|value| value.to_ascii_lowercase()).unwrap_or_default();
    let payload = parse_json(body).unwrap_or(json!({}));
    let text_query = text(&payload, "searchText").unwrap_or_default();
    let top = payload.get("$top").and_then(|value| value.as_u64()).unwrap_or(25) as usize;
    let results = if kind == "codesearchresults" {
        code_results(state, org, project, &text_query, top)
    } else {
        work_results(state, org, project, &text_query, top)
    };
    Json(json!({
        "count": results.len(),
        "infoCode": 0,
        "facets": {},
        "results": results
    }))
    .into_response()
}

fn acls(state: &AppState, method: &Method, org: &str, rest: &[String], body: &[u8]) -> Response {
    let namespace = rest.get(1).cloned().unwrap_or_default();
    if namespace.is_empty() {
        return fail(StatusCode::NOT_FOUND, "espace de noms inconnu");
    }
    let key = format!("acl/{namespace}");
    if method == Method::GET {
        return Json(page(list_org(state, org, &key))).into_response();
    }
    if method == Method::POST || method == Method::PUT {
        let payload = parse_json(body).unwrap_or(json!({}));
        let token = text(&payload, "token").unwrap_or_else(|| "token".into());
        let entries = payload.get("accessControlEntries").and_then(|value| value.as_array()).cloned().unwrap_or_default();
        let mut dictionary = Map::new();
        for entry in entries {
            let descriptor = text(&entry, "descriptor").unwrap_or_else(|| "azurz".into());
            dictionary.insert(
                descriptor.clone(),
                json!({
                    "descriptor": descriptor,
                    "allow": entry.get("allow").cloned().unwrap_or(json!(0)),
                    "deny": entry.get("deny").cloned().unwrap_or(json!(0))
                }),
            );
        }
        let item = json!({
            "id": token,
            "token": token,
            "inheritPermissions": true,
            "acesDictionary": dictionary
        });
        put_org(state, org, &key, item.clone());
        return Json(item).into_response();
    }
    fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée")
}

fn permissions(state: &AppState, method: &Method, org: &str, rest: &[String], body: &[u8]) -> Response {
    let resource = rest.get(1).map(|value| value.to_ascii_lowercase()).unwrap_or_default();
    if resource != "permissionevaluationbatch" || method != Method::POST {
        return fail(StatusCode::NOT_FOUND, "évaluation introuvable");
    }
    let payload = parse_json(body).unwrap_or(json!({}));
    let evaluations = payload.get("evaluations").and_then(|value| value.as_array()).cloned().unwrap_or_default();
    let evaluations = evaluations
        .into_iter()
        .map(|mut evaluation| {
            let namespace = text(&evaluation, "securityNamespaceId").unwrap_or_default();
            let token = text(&evaluation, "token").unwrap_or_default();
            let requested = evaluation.get("permissions").and_then(|value| value.as_i64()).unwrap_or(0);
            let allowed = permission_granted(state, org, &namespace, &token, requested);
            if let Some(object) = evaluation.as_object_mut() {
                object.insert("value".into(), json!(allowed));
            }
            evaluation
        })
        .collect::<Vec<_>>();
    Json(json!({"evaluations": evaluations})).into_response()
}

fn permission_granted(state: &AppState, org: &str, namespace: &str, token: &str, requested: i64) -> bool {
    if requested == 0 {
        return false;
    }
    let key = format!("acl/{namespace}");
    let Some(acl) = list_org(state, org, &key).into_iter().find(|item| text(item, "token").as_deref() == Some(token)) else {
        return false;
    };
    let Some(entries) = acl.get("acesDictionary").and_then(|value| value.as_object()) else {
        return false;
    };
    entries.values().any(|entry| {
        let allow = entry.get("allow").and_then(|value| value.as_i64()).unwrap_or(0);
        let deny = entry.get("deny").and_then(|value| value.as_i64()).unwrap_or(0);
        (allow & requested) == requested && (deny & requested) == 0
    })
}

fn nuget(state: &AppState, method: &Method, org: &str, project: &str, feed_id: &str, feed_name: &str, tail: &[String], body: &[u8]) -> Response {
    let v3 = tail.first().is_some_and(|value| value.eq_ignore_ascii_case("v3"));
    if v3 && tail.get(1).is_some_and(|value| value.eq_ignore_ascii_case("index.json")) {
        return nuget_index(org, project, feed_name);
    }
    let flat = v3 && tail.get(1).is_some_and(|value| value.eq_ignore_ascii_case("flat2"));
    if !flat || tail.len() < 4 {
        return fail(StatusCode::NOT_FOUND, "paquet NuGet introuvable");
    }
    let id = tail[2].to_ascii_lowercase();
    let key = format!("packaging/feeds/{feed_id}/nuget");
    if tail[3].eq_ignore_ascii_case("index.json") && method == Method::GET {
        let mut versions = list_project(state, org, project, &key)
            .into_iter()
            .filter_map(|item| {
                let stored = text(&item, "id")?;
                let (package, version) = stored.split_once('/')?;
                (package == id).then(|| version.to_string())
            })
            .collect::<Vec<_>>();
        versions.sort();
        return Json(json!({"versions": versions})).into_response();
    }
    if tail.len() < 5 {
        return fail(StatusCode::NOT_FOUND, "paquet NuGet introuvable");
    }
    let version = tail[3].clone();
    let stored_id = format!("{id}/{version}");
    if method == Method::PUT {
        let content = String::from_utf8_lossy(body).into_owned();
        put_project(state, org, project, &key, json!({"id": stored_id, "version": version, "content": content}));
        return octets(&content);
    }
    if method == Method::GET {
        return match get_project(state, org, project, &key, &stored_id) {
            Some(item) => octets(&text(&item, "content").unwrap_or_default()),
            None => fail(StatusCode::NOT_FOUND, "paquet NuGet introuvable"),
        };
    }
    fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée")
}

fn upack(state: &AppState, method: &Method, org: &str, project: &str, rest: &[String], body: &[u8]) -> Response {
    if rest.len() < 8 || !rest[4].eq_ignore_ascii_case("packages") || !rest[6].eq_ignore_ascii_case("versions") {
        return fail(StatusCode::NOT_FOUND, "paquet universel introuvable");
    }
    let Some(feed) = find_feed(state, org, project, &rest[2]) else {
        return fail(StatusCode::NOT_FOUND, "flux introuvable");
    };
    let key = format!("packaging/feeds/{}/upack", value_text(&feed["id"]));
    let stored_id = format!("{}/{}", rest[5], rest[7]);
    if method == Method::PUT {
        let content = String::from_utf8_lossy(body).into_owned();
        put_project(
            state,
            org,
            project,
            &key,
            json!({"id": stored_id, "name": rest[5], "version": rest[7], "content": content}),
        );
        return octets(&content);
    }
    if method == Method::GET {
        return match get_project(state, org, project, &key, &stored_id) {
            Some(item) => octets(&text(&item, "content").unwrap_or_default()),
            None => fail(StatusCode::NOT_FOUND, "paquet universel introuvable"),
        };
    }
    fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée")
}

fn octets(content: &str) -> Response {
    typed("application/octet-stream", content)
}

fn nuget_index(org: &str, project: &str, feed: &str) -> Response {
    let base = format!("/{org}/{project}/_packaging/{feed}/nuget");
    Json(json!({
        "version": "3.0.0",
        "resources": [
            {"@id": format!("{base}/v3/flat2/"), "@type": "PackageBaseAddress/3.0.0"},
            {"@id": format!("{base}/v3/registrations2/"), "@type": "RegistrationsBaseUrl/3.0.0"},
            {"@id": format!("{base}/v2"), "@type": "PackagePublish/2.0.0"},
            {"@id": format!("{base}/v3/query"), "@type": "SearchQueryService/3.0.0"}
        ]
    }))
    .into_response()
}

fn protocol_packages(state: &AppState, method: &Method, org: &str, project: &str, rest: &[String]) -> Response {
    if method != Method::GET {
        return fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée");
    }
    let Some(feed) = find_feed(state, org, project, &rest[2]) else {
        return fail(StatusCode::NOT_FOUND, "flux introuvable");
    };
    let protocol = rest[3].to_ascii_lowercase();
    let key = format!("packaging/feeds/{}/{protocol}", value_text(&feed["id"]));
    let value = list_project(state, org, project, &key).into_iter().map(without_content).collect();
    Json(page(value)).into_response()
}

fn without_content(mut item: Value) -> Value {
    if let Some(object) = item.as_object_mut() {
        object.remove("content");
    }
    item
}

fn maven(state: &AppState, method: &Method, org: &str, project: &str, feed_id: &str, tail: &[String], body: &[u8]) -> Response {
    let segments = strip_prefix(tail, "v1");
    if segments.is_empty() {
        return fail(StatusCode::NOT_FOUND, "paquet Maven introuvable");
    }
    let key = format!("packaging/feeds/{feed_id}/maven");
    if segments.last().is_some_and(|value| value.eq_ignore_ascii_case("maven-metadata.xml")) {
        if method != Method::GET {
            return fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée");
        }
        if segments.len() < 2 {
            return fail(StatusCode::NOT_FOUND, "paquet Maven introuvable");
        }
        let artifact = segments[segments.len() - 2].clone();
        let group = segments[..segments.len() - 2].join(".");
        let prefix = maven_prefix(&group, &artifact);
        let mut versions = list_project(state, org, project, &key)
            .into_iter()
            .filter_map(|item| {
                let stored = text(&item, "id")?;
                let rest = stored.strip_prefix(&prefix)?.split('/').next()?;
                (!rest.is_empty()).then(|| rest.to_string())
            })
            .collect::<Vec<_>>();
        versions.sort();
        versions.dedup();
        let latest = versions.last().cloned().unwrap_or_default();
        let listed = versions
            .iter()
            .map(|version| format!("<version>{}</version>", xml_escape(version)))
            .collect::<String>();
        let xml = format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?><metadata><groupId>{}</groupId><artifactId>{}</artifactId><versioning><latest>{}</latest><release>{}</release><versions>{}</versions></versioning></metadata>",
            xml_escape(&group),
            xml_escape(&artifact),
            xml_escape(&latest),
            xml_escape(&latest),
            listed
        );
        return typed("application/xml", &xml);
    }
    if segments.len() < 3 {
        return fail(StatusCode::NOT_FOUND, "paquet Maven introuvable");
    }
    let filename = segments[segments.len() - 1].clone();
    let version = segments[segments.len() - 2].clone();
    let artifact = segments[segments.len() - 3].clone();
    let group = if segments.len() > 3 { segments[..segments.len() - 3].join(".") } else { String::new() };
    let stored_id = format!("{}{artifact}/{version}/{filename}", maven_prefix(&group, ""));
    if method == Method::PUT {
        let content = String::from_utf8_lossy(body).into_owned();
        put_project(
            state,
            org,
            project,
            &key,
            json!({
                "id": stored_id,
                "groupId": group,
                "artifactId": artifact,
                "version": version,
                "filename": filename,
                "content": content
            }),
        );
        return octets(&content);
    }
    if method == Method::GET {
        return match get_project(state, org, project, &key, &stored_id) {
            Some(item) => octets(&text(&item, "content").unwrap_or_default()),
            None => fail(StatusCode::NOT_FOUND, "paquet Maven introuvable"),
        };
    }
    fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée")
}

fn pypi(state: &AppState, method: &Method, org: &str, project: &str, feed_id: &str, tail: &[String], body: &[u8]) -> Response {
    let segments = strip_prefix(tail, "simple");
    if segments.is_empty() {
        return fail(StatusCode::NOT_FOUND, "paquet PyPI introuvable");
    }
    let package = normalize_pypi(&segments[0]);
    let key = format!("packaging/feeds/{feed_id}/pypi");
    if segments.len() == 1 {
        if method != Method::GET {
            return fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée");
        }
        let prefix = format!("{package}/");
        let mut files = list_project(state, org, project, &key)
            .into_iter()
            .filter(|item| text(item, "id").is_some_and(|id| id.starts_with(&prefix)))
            .filter_map(|item| text(&item, "filename"))
            .collect::<Vec<_>>();
        files.sort();
        files.dedup();
        let links = files
            .iter()
            .map(|file| format!("<a href=\"{}\">{}</a>", html_escape(file), html_escape(file)))
            .collect::<Vec<_>>()
            .join("");
        let html = format!(
            "<!DOCTYPE html><html><head><title>Links for {}</title></head><body><h1>Links for {}</h1>{}</body></html>",
            html_escape(&package),
            html_escape(&package),
            links
        );
        return typed("text/html; charset=utf-8", &html);
    }
    let (version, filename) = if segments.len() == 2 {
        let filename = segments[1].clone();
        (pypi_version(&package, &filename), filename)
    } else {
        (segments[1].clone(), segments[2].clone())
    };
    let stored_id = format!("{package}/{version}/{filename}");
    if method == Method::PUT {
        let content = String::from_utf8_lossy(body).into_owned();
        put_project(
            state,
            org,
            project,
            &key,
            json!({
                "id": stored_id,
                "name": package,
                "version": version,
                "filename": filename,
                "content": content
            }),
        );
        return octets(&content);
    }
    if method == Method::GET {
        return match get_project(state, org, project, &key, &stored_id) {
            Some(item) => octets(&text(&item, "content").unwrap_or_default()),
            None => fail(StatusCode::NOT_FOUND, "paquet PyPI introuvable"),
        };
    }
    fail(StatusCode::METHOD_NOT_ALLOWED, "méthode refusée")
}

fn strip_prefix<'a>(tail: &'a [String], prefix: &str) -> &'a [String] {
    if tail.first().is_some_and(|value| value.eq_ignore_ascii_case(prefix)) {
        &tail[1..]
    } else {
        tail
    }
}

fn maven_prefix(group: &str, artifact: &str) -> String {
    if group.is_empty() {
        return if artifact.is_empty() { String::new() } else { format!("{artifact}/") };
    }
    if artifact.is_empty() {
        format!("{group}/")
    } else {
        format!("{group}/{artifact}/")
    }
}

fn pypi_version(package: &str, filename: &str) -> String {
    let lower = filename.to_ascii_lowercase();
    let stem = lower
        .trim_end_matches(".tar.gz")
        .trim_end_matches(".zip")
        .trim_end_matches(".whl");
    let prefix = format!("{package}-");
    stem.strip_prefix(&prefix)
        .and_then(|rest| rest.split('-').next())
        .filter(|version| !version.is_empty())
        .unwrap_or("0.0.0")
        .to_string()
}

fn normalize_pypi(name: &str) -> String {
    let mut normalized = String::new();
    let mut dash = false;
    for character in name.chars() {
        if character == '-' || character == '_' || character == '.' {
            if !dash && !normalized.is_empty() {
                normalized.push('-');
                dash = true;
            }
        } else {
            normalized.push(character.to_ascii_lowercase());
            dash = false;
        }
    }
    normalized
}

fn xml_escape(value: &str) -> String {
    value.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn typed(content_type: &str, content: &str) -> Response {
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, content_type)
        .body(axum::body::Body::from(content.to_string()))
        .unwrap_or_else(|_| fail(StatusCode::INTERNAL_SERVER_ERROR, "réponse impossible"))
}

fn npm_package(state: &AppState, method: &Method, org: &str, project: &str, feed_id: &str, name: &str, body: &[u8]) -> Response {
    if name.is_empty() {
        return fail(StatusCode::NOT_FOUND, "paquet introuvable");
    }
    let key = format!("packaging/feeds/{feed_id}/npm");
    if method == Method::GET {
        return match get_project(state, org, project, &key, name) {
            Some(item) => Json(item).into_response(),
            None => fail(StatusCode::NOT_FOUND, "paquet introuvable"),
        };
    }
    let payload = parse_json(body).unwrap_or(json!({}));
    let mut item = payload;
    if let Some(object) = item.as_object_mut() {
        object.insert("id".into(), json!(name));
        object.entry("name").or_insert(json!(name));
        object.entry("versions").or_insert(json!({}));
    }
    put_project(state, org, project, &key, item.clone());
    Json(item).into_response()
}

pub fn work_items_batch(state: &AppState, org: &str, project: &str, body: &[u8]) -> Response {
    let payload = parse_json(body).unwrap_or(json!({}));
    let ids = payload.get("ids").and_then(|value| value.as_array()).cloned().unwrap_or_default();
    let fields = payload
        .get("fields")
        .and_then(|value| value.as_array())
        .map(|rows| {
            rows.iter().filter_map(|value| value.as_str().map(str::to_string)).collect::<Vec<_>>()
        });
    let value = state.read(|db| {
        let Some(stored) = db.project(org, project) else {
            return Vec::new();
        };
        ids.into_iter()
            .filter_map(|id| id.as_i64())
            .filter_map(|id| stored.work_items.get(&id).cloned())
            .map(|mut item| {
                if let Some(wanted) = &fields {
                    if let Some(object) = item.get_mut("fields").and_then(|value| value.as_object_mut()) {
                        object.retain(|key, _| wanted.iter().any(|field| field == key));
                    }
                }
                item
            })
            .collect::<Vec<_>>()
    });
    Json(page(value)).into_response()
}

fn work_results(state: &AppState, org: &str, project: Option<&str>, query: &str, top: usize) -> Vec<Value> {
    let needle = query.to_ascii_lowercase();
    state.read(|db| {
        let Some(organization) = db.orgs.get(org) else {
            return Vec::new();
        };
        let mut rows = Vec::new();
        for (name, stored) in &organization.projects {
            if project.is_some_and(|wanted| wanted != name) {
                continue;
            }
            for item in stored.work_items.values() {
                let fields = item.get("fields").cloned().unwrap_or(json!({}));
                let title = text(&fields, "System.Title").unwrap_or_default();
                let description = text(&fields, "System.Description").unwrap_or_default();
                let tags = text(&fields, "System.Tags").unwrap_or_default();
                let haystack = format!("{title} {description} {tags}").to_ascii_lowercase();
                if !needle.is_empty() && !haystack.contains(&needle) {
                    continue;
                }
                rows.push(json!({
                    "project": {"name": name},
                    "fields": {
                        "system.id": item.get("id").cloned().unwrap_or(json!(0)).to_string(),
                        "system.title": title,
                        "system.workitemtype": text(&fields, "System.WorkItemType").unwrap_or_default(),
                        "system.state": text(&fields, "System.State").unwrap_or_else(|| "New".into())
                    },
                    "url": format!("/{org}/{name}/_workitems/edit/{}", item.get("id").and_then(|value| value.as_i64()).unwrap_or(0))
                }));
                if rows.len() >= top {
                    return rows;
                }
            }
        }
        rows
    })
}

fn code_results(state: &AppState, org: &str, project: Option<&str>, query: &str, top: usize) -> Vec<Value> {
    let needle = query.to_ascii_lowercase();
    state.read(|db| {
        let Some(organization) = db.orgs.get(org) else {
            return Vec::new();
        };
        let mut rows = Vec::new();
        for (name, stored) in &organization.projects {
            if project.is_some_and(|wanted| wanted != name) {
                continue;
            }
            for (key, bucket) in &stored.buckets {
                let git_items = key.starts_with("git/repositories/") && key.ends_with("/items");
                let tfvc_items = key == "tfvc/items";
                if !git_items && !tfvc_items {
                    continue;
                }
                for item in bucket.items.values() {
                    let path = text(item, "path").unwrap_or_default();
                    let content = text(item, "content").unwrap_or_default();
                    let haystack = format!("{path} {content}").to_ascii_lowercase();
                    if !needle.is_empty() && !haystack.contains(&needle) {
                        continue;
                    }
                    if path == "/" || path == "$/" {
                        continue;
                    }
                    let file_name = path.rsplit('/').next().unwrap_or(&path).to_string();
                    rows.push(json!({
                        "fileName": file_name,
                        "path": path,
                        "content": content,
                        "repository": {"name": name},
                        "versions": [{"branchName": "main"}],
                        "matches": {"content": [{"charOffset": 0, "length": needle.len()}]}
                    }));
                    if rows.len() >= top {
                        return rows;
                    }
                }
            }
        }
        rows
    })
}

fn ensure_tfvc_root(state: &AppState, org: &str, project: &str) {
    if list_project(state, org, project, "tfvc/items").is_empty() {
        put_project(state, org, project, "tfvc/items", json!({"id": "$/", "path": "$/", "isFolder": true, "content": ""}));
    }
}

fn find_feed(state: &AppState, org: &str, project: &str, name: &str) -> Option<Value> {
    list_project(state, org, project, "packaging/feeds").into_iter().find(|item| {
        text(item, "name").as_deref() == Some(name) || text(item, "id").as_deref() == Some(name) || value_text(&item["id"]) == name
    })
}

fn list_project(state: &AppState, org: &str, project: &str, key: &str) -> Vec<Value> {
    state.read(|db| db.project(org, project).map(|stored| stored.list_bucket(key)).unwrap_or_default())
}

fn get_project(state: &AppState, org: &str, project: &str, key: &str, id: &str) -> Option<Value> {
    state.read(|db| db.project(org, project).and_then(|stored| stored.get_bucket(key, id)))
}

fn put_project(state: &AppState, org: &str, project: &str, key: &str, item: Value) {
    state.write(|db| {
        db.ensure_project(org, project);
        if let Some(stored) = db.project_mut(org, project) {
            stored.put_bucket(key, item);
        }
    });
}

fn list_org(state: &AppState, org: &str, key: &str) -> Vec<Value> {
    state.read(|db| {
        db.orgs
            .get(org)
            .and_then(|organization| organization.buckets.get(key))
            .map(|bucket| bucket.items.values().cloned().collect())
            .unwrap_or_default()
    })
}

fn put_org(state: &AppState, org: &str, key: &str, item: Value) {
    state.write(|db| {
        db.ensure_org(org);
        if let Some(organization) = db.orgs.get_mut(org) {
            let bucket = organization.buckets.entry(key.to_string()).or_insert_with(crate::store::Bucket::empty);
            bucket.put(item);
        }
    });
}

fn page(value: Vec<Value>) -> Value {
    json!({"count": value.len(), "value": value})
}

fn text(value: &Value, key: &str) -> Option<String> {
    value.get(key).and_then(|item| item.as_str()).map(str::to_string)
}

fn value_text(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        Value::Number(number) => number.to_string(),
        _ => String::new(),
    }
}
