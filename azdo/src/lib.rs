mod http;
mod shapes;
mod store;
mod tracking;
mod web;

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use axum::Router;
use store::Db;
use tower_http::cors::CorsLayer;

use crate::http::dispatch;

#[derive(Clone)]
pub struct AppState {
    inner: Arc<Mutex<Inner>>,
}

struct Inner {
    db: Db,
    pat: String,
    org: String,
    project: String,
    path: Option<PathBuf>,
}

impl AppState {
    pub fn lab() -> Self {
        Self::new("azurz-lab-pat", "azurz", "AzurzDemo", None)
    }

    pub fn from_env() -> Self {
        let pat = std::env::var("AZDO_PAT")
            .or_else(|_| std::env::var("AZURZ_AZDO_PAT"))
            .unwrap_or_else(|_| "azurz-lab-pat".into());
        let org = std::env::var("AZDO_ORG")
            .or_else(|_| std::env::var("AZURZ_AZDO_ORG"))
            .unwrap_or_else(|_| "azurz".into());
        let project = std::env::var("AZDO_PROJECT")
            .or_else(|_| std::env::var("AZURZ_AZDO_PROJECT"))
            .unwrap_or_else(|_| "AzurzDemo".into());
        let path = std::env::var("AZDO_DATA").ok().map(PathBuf::from);
        Self::new(&pat, &org, &project, path)
    }

    fn new(pat: &str, org: &str, project: &str, path: Option<PathBuf>) -> Self {
        let db = Db::open(&path, org, project);
        Self {
            inner: Arc::new(Mutex::new(Inner {
                db,
                pat: pat.to_string(),
                org: org.to_string(),
                project: project.to_string(),
                path,
            })),
        }
    }

    pub fn names(&self) -> (String, String) {
        let inner = self.inner.lock().expect("db");
        (inner.org.clone(), inner.project.clone())
    }

    pub fn password_ok(&self, password: &str) -> bool {
        self.inner.lock().expect("db").pat == password
    }

    pub fn open_session(&self, user: String) -> String {
        let mut inner = self.inner.lock().expect("db");
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or(0);
        let token = format!("{nanos:x}-{}", inner.db.sessions.len());
        inner.db.sessions.insert(token.clone(), user);
        inner.db.save(&inner.path);
        token
    }

    pub fn close_session(&self, token: &str) {
        let mut inner = self.inner.lock().expect("db");
        inner.db.sessions.remove(token);
        inner.db.save(&inner.path);
    }

    pub fn session_user(&self, token: &str) -> Option<String> {
        self.inner
            .lock()
            .expect("db")
            .db
            .sessions
            .get(token)
            .cloned()
    }

    pub fn read<R>(&self, reader: impl FnOnce(&Db) -> R) -> R {
        let inner = self.inner.lock().expect("db");
        reader(&inner.db)
    }

    pub fn write<R>(&self, writer: impl FnOnce(&mut Db) -> R) -> R {
        let mut inner = self.inner.lock().expect("db");
        let result = writer(&mut inner.db);
        let path = inner.path.clone();
        inner.db.save(&path);
        result
    }
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .fallback(dispatch)
        .layer(CorsLayer::permissive())
        .with_state(state)
}
