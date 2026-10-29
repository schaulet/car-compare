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
    #[serde(default)]
    pub history: BTreeMap<i64, Vec<Value>>,
    #[serde(default)]
    pub attachments: BTreeMap<String, Attachment>,
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
                if let Ok(mut db) = serde_json::from_str::<Db>(&text) {
                    db.ensure_project(org, project);
                    let project_name = project.to_string();
                    if let Some(stored) = db.project_mut(org, project) {
                        stored.seed_missing(&project_name);
                    }
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
        let project_name = project.to_string();
        if let Some(stored) = db.project_mut(org, project) {
            stored.seed_missing(&project_name);
        }
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
        if let Some(organization) = self.orgs.get_mut(org) {
            organization
                .buckets
                .entry("distributedtask/pools".into())
                .or_insert_with(|| {
                    Bucket::with(json!({
                        "id": 1,
                        "name": "Azure Pipelines",
                        "isHosted": false,
                        "size": 1
                    }))
                });
        }
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
            history: BTreeMap::new(),
            attachments: BTreeMap::new(),
            next_work_item: 1,
            buckets: BTreeMap::new(),
        };
        project.seed_missing(name);
        project
    }

    pub fn seed_missing(&mut self, name: &str) {
        let repo_id = "11111111-1111-1111-1111-111111111111";
        self.buckets.entry("git/repositories".into()).or_insert_with(|| {
            Bucket::with(json!({
                "id": repo_id,
                "name": name,
                "defaultBranch": "refs/heads/main",
                "size": 0,
                "isDisabled": false,
                "isFork": false
            }))
        });
        self.buckets.entry("core/teams".into()).or_insert_with(|| {
            Bucket::with(json!({
                "name": format!("{name} Team"),
                "description": "Équipe par défaut"
            }))
        });
        self.buckets
            .entry("work/teamsettings/iterations".into())
            .or_insert_with(|| {
                Bucket::with(json!({
                    "name": "Sprint 1",
                    "path": format!("{name}\\Sprint 1"),
                    "attributes": {"startDate": null, "finishDate": null, "timeFrame": "current"}
                }))
            });
        self.buckets.entry("build/definitions".into()).or_insert_with(|| {
            Bucket::with(json!({
                "id": 1,
                "name": "CI",
                "path": "\\",
                "type": "build",
                "queueStatus": "enabled",
                "revision": 1
            }))
        });
        self.buckets.entry("pipelines/pipelines".into()).or_insert_with(|| {
            Bucket::with(json!({
                "id": 1,
                "name": "CI",
                "folder": "\\",
                "revision": 1
            }))
        });
        self.buckets.entry("distributedtask/queues".into()).or_insert_with(|| {
            Bucket::with(json!({
                "id": 1,
                "name": "Default",
                "pool": {"id": 1, "name": "Azure Pipelines"}
            }))
        });
        let repo_ids: Vec<String> = self
            .buckets
            .get("git/repositories")
            .map(|bucket| bucket.items.values().map(|item| value_key(&item["id"])).collect())
            .unwrap_or_default();
        for id in repo_ids {
            if id.is_empty() {
                continue;
            }
            let commit = object_id(&format!("{name}:{id}:main"));
            self.buckets
                .entry(format!("git/repositories/{id}/refs"))
                .or_insert_with(|| {
                    Bucket::with(json!({
                        "id": "refs/heads/main",
                        "name": "refs/heads/main",
                        "objectId": commit,
                        "success": true
                    }))
                });
            self.buckets
                .entry(format!("git/repositories/{id}/commits"))
                .or_insert_with(|| {
                    Bucket::with(json!({
                        "id": commit,
                        "commitId": commit,
                        "comment": "Initialisation du dépôt",
                        "author": {"name": "azurz", "date": "2026-10-29T00:00:00Z"}
                    }))
                });
            let readme = format!("# {name}\n");
            self.buckets
                .entry(format!("git/repositories/{id}/items"))
                .or_insert_with(|| {
                    let mut bucket = Bucket::empty();
                    bucket.put(json!({
                        "id": "/",
                        "path": "/",
                        "isFolder": true,
                        "gitObjectType": "tree"
                    }));
                    bucket.put(json!({
                        "id": "/README.md",
                        "path": "/README.md",
                        "isFolder": false,
                        "gitObjectType": "blob",
                        "content": readme
                    }));
                    bucket
                });
            self.buckets
                .entry(format!("git/repositories/{id}/commits/{commit}/blobs"))
                .or_insert_with(|| {
                    let mut bucket = Bucket::empty();
                    bucket.put(json!({
                        "id": "/",
                        "path": "/",
                        "isFolder": true,
                        "gitObjectType": "tree"
                    }));
                    bucket.put(json!({
                        "id": "/README.md",
                        "path": "/README.md",
                        "isFolder": false,
                        "gitObjectType": "blob",
                        "content": format!("# {name}\n")
                    }));
                    bucket
                });
        }
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
        stored.entry("System.AreaPath").or_insert(json!(project));
        stored
            .entry("System.IterationPath")
            .or_insert(json!(format!("{project}\\Sprint 1")));
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
        self.history.entry(id).or_default().push(item.clone());
        item
    }

    pub fn patch_work_item(&mut self, id: i64, ops: &[Value]) -> Result<Value, String> {
        let snapshot = {
            let item = self
                .work_items
                .get_mut(&id)
                .ok_or_else(|| format!("work item {id} introuvable"))?;
            for op in ops {
                let operation = op.get("op").and_then(|value| value.as_str()).unwrap_or("");
                let path = op.get("path").and_then(|value| value.as_str()).unwrap_or("");
                if operation == "add" && (path == "/relations/-" || path == "/relations") {
                    let value = op.get("value").cloned().unwrap_or(json!({}));
                    if !item.get("relations").map(|relations| relations.is_array()).unwrap_or(false) {
                        item["relations"] = json!([]);
                    }
                    if let Some(relations) = item.get_mut("relations").and_then(|relations| relations.as_array_mut()) {
                        relations.push(value);
                    }
                    continue;
                }
                if operation == "remove" {
                    if let Some(index) = path.strip_prefix("/relations/") {
                        if let Ok(index) = index.parse::<usize>() {
                            if let Some(relations) = item.get_mut("relations").and_then(|relations| relations.as_array_mut()) {
                                if index < relations.len() {
                                    relations.remove(index);
                                }
                            }
                        }
                        continue;
                    }
                }
                let Some(key) = path.strip_prefix("/fields/") else {
                    continue;
                };
                let Some(fields) = item.get_mut("fields").and_then(|value| value.as_object_mut()) else {
                    continue;
                };
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
            item.clone()
        };
        self.history.entry(id).or_default().push(snapshot.clone());
        Ok(snapshot)
    }

    pub fn add_comment(&mut self, id: i64, text: &str) -> Result<Value, String> {
        if !self.work_items.contains_key(&id) {
            return Err(format!("work item {id} introuvable"));
        }
        let notes = self.comments.entry(id).or_default();
        let comment_id = notes.len() as i64 + 1;
        let note = json!({
            "id": comment_id,
            "text": text,
            "workItemId": id,
            "createdBy": {"displayName": "azurz", "uniqueName": "azurz"},
            "createdDate": "2026-10-29T00:00:00Z"
        });
        notes.push(note.clone());
        Ok(note)
    }

    pub fn add_attachment(&mut self, name: &str, bytes: &[u8]) -> Value {
        let id = format!("att-{:08x}", self.attachments.len() + 1);
        self.attachments.insert(
            id.clone(),
            Attachment {
                name: name.to_string(),
                content_type: "application/octet-stream".into(),
                bytes: bytes.to_vec(),
            },
        );
        json!({"id": id, "url": format!("_apis/wit/attachments/{id}?fileName={name}")})
    }

    pub fn attachment(&self, id: &str) -> Option<&Attachment> {
        self.attachments.get(id)
    }

    pub fn list_bucket(&self, key: &str) -> Vec<Value> {
        self.buckets
            .get(key)
            .map(|bucket| bucket.items.values().cloned().collect())
            .unwrap_or_default()
    }

    pub fn put_bucket(&mut self, key: &str, item: Value) -> Value {
        let bucket = self.buckets.entry(key.to_string()).or_insert_with(Bucket::empty);
        bucket.put(item)
    }

    pub fn get_bucket(&self, key: &str, id: &str) -> Option<Value> {
        self.buckets.get(key).and_then(|bucket| bucket.get(id))
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Attachment {
    pub name: String,
    pub content_type: String,
    pub bytes: Vec<u8>,
}

impl Bucket {
    pub fn empty() -> Self {
        Bucket { items: BTreeMap::new(), next: 1 }
    }

    pub fn put(&mut self, body: Value) -> Value {
        let key = value_key(&body["id"]);
        if key.is_empty() {
            return self.create(body);
        }
        if let Ok(numeric) = key.parse::<i64>() {
            if numeric >= self.next {
                self.next = numeric + 1;
            }
        }
        if let Some(previous) = self.items.iter().find_map(|(stored, item)| {
            (stored == &key || value_key(&item["id"]) == key).then(|| stored.clone())
        }) {
            if previous != key {
                self.items.remove(&previous);
            }
        }
        self.items.insert(key, body.clone());
        body
    }

    fn with(value: Value) -> Self {
        let mut bucket = Bucket { items: BTreeMap::new(), next: 2 };
        let mut item = value;
        if let Some(object) = item.as_object_mut() {
            object.entry("id").or_insert(json!("1"));
        }
        bucket.put(item);
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

pub fn object_id(seed: &str) -> String {
    let mut lanes = [
        0xcbf29ce484222325u64,
        0x84222325cbf29ce4u64,
        0x9e3779b97f4a7c15u64,
        0x6c62272e07bb0142u64,
        0x517cc1b727220a95u64,
    ];
    for (index, byte) in seed.bytes().enumerate() {
        let lane = index % lanes.len();
        lanes[lane] ^= u64::from(byte);
        lanes[lane] = lanes[lane].wrapping_mul(0x100000001b3);
        lanes[lane] ^= (index as u64).wrapping_shl(1).wrapping_add(0x9e3779b97f4a7c15);
    }
    for index in 0..lanes.len() {
        let next = lanes[(index + 1) % lanes.len()];
        lanes[index] ^= next.rotate_left(17);
        lanes[index] = lanes[index].wrapping_mul(0x100000001b3);
    }
    let mut out = String::with_capacity(40);
    for lane in lanes {
        out.push_str(&format!("{:08x}", lane as u32));
    }
    out
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
