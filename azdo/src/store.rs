use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};

#[derive(Clone, Serialize, Deserialize)]
pub struct Db {
    pub orgs: BTreeMap<String, OrgData>,
    pub sessions: BTreeMap<String, String>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct OrgData {
    pub projects: BTreeMap<String, ProjectData>,
    pub buckets: BTreeMap<String, Bucket>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct ProjectData {
    pub id: String,
    pub description: String,
    pub work_items: BTreeMap<i64, Value>,
    pub comments: BTreeMap<i64, Vec<Value>>,
    pub next_work_item: i64,
    pub buckets: BTreeMap<String, Bucket>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Bucket {
    pub items: BTreeMap<String, Value>,
    pub next: i64,
}

impl Db {
    pub fn open(path: &Option<PathBuf>, org: &str, project: &str) -> Self {
        if let Some(path) = path {
            if let Ok(text) = fs::read_to_string(path) {
                if let Ok(db) = serde_json::from_str::<Db>(&text) {
                    return db;
                }
            }
        }
        let mut db = Db {
            orgs: BTreeMap::new(),
            sessions: BTreeMap::new(),
        };
        db.ensure_org(org);
        db.ensure_project(org, project);
        db
    }

    pub fn save(&self, path: &Option<PathBuf>) {
        let Some(path) = path else {
            return;
        };
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        if let Ok(text) = serde_json::to_string_pretty(self) {
            let _ = fs::write(path, text);
        }
    }

    pub fn ensure_org(&mut self, org: &str) {
        self.orgs.entry(org.to_string()).or_insert_with(|| OrgData {
            projects: BTreeMap::new(),
            buckets: BTreeMap::new(),
        });
    }

    pub fn ensure_project(&mut self, org: &str, name: &str) -> bool {
        self.ensure_org(org);
        let organization = self.orgs.get_mut(org).expect("organisation");
        if organization.projects.contains_key(name)
            || organization.projects.values().any(|project| project.id == name)
        {
            return false;
        }
        organization
            .projects
            .insert(name.to_string(), ProjectData::seed(name));
        true
    }

    pub fn project(&self, org: &str, key: &str) -> Option<&ProjectData> {
        let organization = self.orgs.get(org)?;
        if let Some(project) = organization.projects.get(key) {
            return Some(project);
        }
        organization.projects.values().find(|project| project.id == key)
    }

    pub fn project_mut(&mut self, org: &str, key: &str) -> Option<&mut ProjectData> {
        let organization = self.orgs.get_mut(org)?;
        if organization.projects.contains_key(key) {
            return organization.projects.get_mut(key);
        }
        let name = organization
            .projects
            .iter()
            .find(|(_, project)| project.id == key)
            .map(|(name, _)| name.clone())?;
        organization.projects.get_mut(&name)
    }

    pub fn project_name(&self, org: &str, key: &str) -> Option<String> {
        let organization = self.orgs.get(org)?;
        if organization.projects.contains_key(key) {
            return Some(key.to_string());
        }
        organization
            .projects
            .iter()
            .find(|(_, project)| project.id == key)
            .map(|(name, _)| name.clone())
    }
}

impl ProjectData {
    pub fn seed(name: &str) -> Self {
        let mut project = ProjectData {
            id: if name == "AzurzDemo" {
                "azurz-demo".into()
            } else {
                slug(name)
            },
            description: "Projet préconfiguré du lab Azurz".into(),
            work_items: BTreeMap::new(),
            comments: BTreeMap::new(),
            next_work_item: 1,
            buckets: BTreeMap::new(),
        };
        project.buckets.insert(
            "git/repositories".into(),
            Bucket::with(json!({
                "name": name,
                "defaultBranch": "refs/heads/main",
                "size": 0,
                "isDisabled": false
            })),
        );
        project.buckets.insert(
            "core/teams".into(),
            Bucket::with(json!({
                "name": format!("{name} Team"),
                "description": "Équipe par défaut"
            })),
        );
        project.buckets.insert(
            "work/teamsettings/iterations".into(),
            Bucket::with(json!({
                "name": "Sprint 1",
                "path": format!("{name}\\Sprint 1"),
                "attributes": {"startDate": null, "finishDate": null, "timeFrame": "current"}
            })),
        );
        project
    }

    pub fn create_work_item(&mut self, project: &str, kind: &str, fields: Map<String, Value>) -> Value {
        let id = self.next_work_item;
        self.next_work_item += 1;
        let mut stored = Map::new();
        stored.insert("System.TeamProject".into(), json!(project));
        stored.insert("System.WorkItemType".into(), json!(kind));
        stored.insert("System.State".into(), json!("New"));
        for (key, value) in fields {
            stored.insert(key, value);
        }
        let item = json!({
            "id": id,
            "rev": 1,
            "fields": stored,
            "url": format!("_apis/wit/workitems/{id}"),
            "_links": {"self": {"href": format!("_apis/wit/workitems/{id}")}},
            "commentVersionRef": {"commentId": 0}
        });
        self.work_items.insert(id, item.clone());
        self.comments.insert(id, Vec::new());
        item
    }

    pub fn patch_work_item(&mut self, id: i64, ops: &[Value]) -> Result<Value, String> {
        let item = self
            .work_items
            .get_mut(&id)
            .ok_or_else(|| format!("work item {id} introuvable"))?;
        let fields = item
            .get_mut("fields")
            .and_then(|value| value.as_object_mut())
            .ok_or_else(|| "champs absents".to_string())?;
        for op in ops {
            let operation = op.get("op").and_then(|value| value.as_str()).unwrap_or("");
            let path = op.get("path").and_then(|value| value.as_str()).unwrap_or("");
            if !path.starts_with("/fields/") {
                continue;
            }
            let key = path.trim_start_matches("/fields/");
            match operation {
                "test" => {
                    let current = fields.get(key);
                    if current != op.get("value") {
                        return Err(format!("test échoué sur {key}"));
                    }
                }
                "add" | "replace" => {
                    if let Some(value) = op.get("value") {
                        fields.insert(key.to_string(), value.clone());
                    }
                }
                "remove" => {
                    fields.remove(key);
                }
                _ => {}
            }
        }
        let rev = item.get("rev").and_then(|value| value.as_i64()).unwrap_or(1) + 1;
        item["rev"] = json!(rev);
        Ok(item.clone())
    }

    pub fn add_comment(&mut self, id: i64, text: &str) -> Result<Value, String> {
        if !self.work_items.contains_key(&id) {
            return Err(format!("work item {id} introuvable"));
        }
        let notes = self.comments.entry(id).or_default();
        let comment_id = notes.len() as i64 + 1;
        let note = json!({"id": comment_id, "text": text, "workItemId": id});
        notes.push(note.clone());
        Ok(note)
    }
}

impl Bucket {
    fn with(value: Value) -> Self {
        let mut bucket = Bucket { items: BTreeMap::new(), next: 2 };
        let mut item = value;
        if let Some(object) = item.as_object_mut() {
            object.entry("id").or_insert(json!("1"));
        }
        bucket.items.insert("1".into(), item);
        bucket
    }

    pub fn list(&self) -> Value {
        let value: Vec<Value> = self.items.values().cloned().collect();
        json!({"count": value.len(), "value": value})
    }

    pub fn create(&mut self, mut body: Value) -> Value {
        let id = self.next.to_string();
        self.next += 1;
        if let Some(object) = body.as_object_mut() {
            object.entry("id").or_insert(json!(id.parse::<i64>().unwrap_or(0)));
            object.entry("url").or_insert(json!(format!("_apis/{id}")));
        } else {
            body = json!({"id": id.parse::<i64>().unwrap_or(0), "value": body});
        }
        let key = body.get("id").map(value_key).unwrap_or(id);
        self.items.insert(key, body.clone());
        body
    }

    pub fn get(&self, id: &str) -> Option<Value> {
        self.items.get(id).cloned().or_else(|| {
            self.items
                .values()
                .find(|item| value_key(&item["id"]) == id || item.get("name").and_then(|v| v.as_str()) == Some(id))
                .cloned()
        })
    }

    pub fn patch(&mut self, id: &str, body: &Value) -> Option<Value> {
        let key = self.items.iter().find_map(|(key, item)| {
            if key == id || value_key(&item["id"]) == id {
                Some(key.clone())
            } else {
                None
            }
        })?;
        let item = self.items.get_mut(&key)?;
        if let (Some(target), Some(patch)) = (item.as_object_mut(), body.as_object()) {
            for (key, value) in patch {
                target.insert(key.clone(), value.clone());
            }
        }
        Some(item.clone())
    }

    pub fn remove(&mut self, id: &str) -> bool {
        let key = self.items.iter().find_map(|(key, item)| {
            if key == id || value_key(&item["id"]) == id {
                Some(key.clone())
            } else {
                None
            }
        });
        key.map(|key| self.items.remove(&key).is_some()).unwrap_or(false)
    }
}

pub fn value_key(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        Value::Number(number) => number.to_string(),
        _ => String::new(),
    }
}

pub fn slug(name: &str) -> String {
    name.chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch.to_ascii_lowercase() } else { '-' })
        .collect()
}

pub fn apply_wiql<'a>(items: impl Iterator<Item = &'a Value>, query: &str) -> Vec<Value> {
    items
        .filter(|item| wiql_match(item.get("fields").and_then(|value| value.as_object()), query))
        .filter_map(|item| {
            item.get("id").map(|id| {
                json!({
                    "id": id,
                    "url": format!("_apis/wit/workitems/{}", value_key(id))
                })
            })
        })
        .collect()
}

fn wiql_match(fields: Option<&Map<String, Value>>, query: &str) -> bool {
    let Some(fields) = fields else {
        return false;
    };
    if let Some(tag) = clause_value(query, "System.Tags", "CONTAINS") {
        let current = fields.get("System.Tags").and_then(|value| value.as_str()).unwrap_or("");
        if !current.to_ascii_lowercase().contains(&tag.to_ascii_lowercase()) {
            return false;
        }
    }
    if let Some(title) = clause_value(query, "System.Title", "CONTAINS") {
        let current = fields.get("System.Title").and_then(|value| value.as_str()).unwrap_or("");
        if !current.to_ascii_lowercase().contains(&title.to_ascii_lowercase()) {
            return false;
        }
    }
    for field in ["System.State", "System.WorkItemType", "System.TeamProject"] {
        if let Some(expected) = clause_value(query, field, "=") {
            let current = fields.get(field).and_then(|value| value.as_str()).unwrap_or("");
            if !current.eq_ignore_ascii_case(&expected) {
                return false;
            }
        }
    }
    true
}

fn clause_value<'a>(query: &'a str, field: &str, operator: &str) -> Option<&'a str> {
    let lower = query.to_ascii_lowercase();
    let field_lower = field.to_ascii_lowercase();
    let operator_lower = operator.to_ascii_lowercase();
    let start = lower.find(&field_lower)?;
    let after = &query[start + field.len()..];
    let after_lower = &lower[start + field.len()..];
    let op_at = after_lower.find(operator_lower.as_str())?;
    let tail = after[op_at + operator.len()..].trim_start();
    let tail = tail.trim_start_matches(']').trim_start();
    let tail = tail.trim_start_matches('\'').trim_start_matches('"');
    let end = tail.find(['\'', '"']).unwrap_or(tail.len());
    Some(tail[..end].trim())
}

pub fn fields_from_patch(ops: &[Value]) -> Result<Map<String, Value>, String> {
    let mut fields = Map::new();
    for op in ops {
        let operation = op.get("op").and_then(|value| value.as_str()).unwrap_or("");
        if operation != "add" && operation != "replace" {
            continue;
        }
        let path = op.get("path").and_then(|value| value.as_str()).unwrap_or("");
        if let Some(key) = path.strip_prefix("/fields/") {
            if let Some(value) = op.get("value") {
                fields.insert(key.to_string(), value.clone());
            }
        }
    }
    Ok(fields)
}

pub const RESOURCE_AREAS: &[&str] = &[
    "core",
    "wit",
    "workItemTracking",
    "git",
    "tfvc",
    "build",
    "release",
    "test",
    "testplan",
    "testresults",
    "wiki",
    "graph",
    "pipelines",
    "distributedtask",
    "dashboard",
    "work",
    "policy",
    "hooks",
    "servicehooks",
    "security",
    "packaging",
    "search",
    "memberentitlementmanagement",
    "identity",
    "accounts",
    "locations",
    "operations",
    "extensionmanagement",
    "notification",
    "favorites",
    "workitemtrackingprocess",
    "taskagent",
    "contribution",
    "featuremanagement",
    "settings",
    "audit",
    "management",
    "blob",
    "tokenadministration",
    "permissionsreport",
    "processes",
    "provenance",
    "customerintelligence",
    "pipelineschecks",
    "reporting",
    "status",
    "symbol",
    "upack",
    "maven",
    "npm",
    "nuget",
    "pypi",
];
