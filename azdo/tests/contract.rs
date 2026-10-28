use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use tower::ServiceExt;

fn app() -> axum::Router {
    azdo::router(azdo::AppState::lab())
}

async fn call(
    app: &axum::Router,
    method: &str,
    uri: &str,
    auth: Option<&str>,
    cookie: Option<&str>,
    content_type: &str,
    body: Option<&str>,
) -> (StatusCode, String, Option<String>) {
    let mut builder = Request::builder().method(method).uri(uri);
    if let Some(auth) = auth {
        builder = builder.header("authorization", auth);
    }
    if let Some(cookie) = cookie {
        builder = builder.header("cookie", cookie);
    }
    if body.is_some() {
        builder = builder.header("content-type", content_type);
    }
    let request = builder
        .body(Body::from(body.unwrap_or("").to_string()))
        .unwrap();
    let response = app.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let cookie = response
        .headers()
        .get("set-cookie")
        .and_then(|value| value.to_str().ok())
        .map(|value| value.split(';').next().unwrap_or(value).to_string());
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (status, String::from_utf8(bytes.to_vec()).unwrap(), cookie)
}

fn basic(user: &str, password: &str) -> String {
    use base64::Engine;
    format!(
        "Basic {}",
        base64::engine::general_purpose::STANDARD.encode(format!("{user}:{password}"))
    )
}

#[tokio::test]
async fn health_is_public_and_projects_require_the_pat() {
    let app = app();
    let (status, body, _) = call(&app, "GET", "/health", None, None, "application/json", None).await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("AzurzDemo"));

    let (status, body, _) = call(
        &app,
        "GET",
        "/azurz/_apis/projects?api-version=7.1",
        None,
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert!(body.contains("PAT refusé"));
}

#[tokio::test]
async fn pat_is_the_password_whatever_the_username() {
    let app = app();
    let (status, body, _) = call(
        &app,
        "GET",
        "/azurz/_apis/projects",
        Some(&basic("azurz", "azurz-lab-pat")),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(body.contains("AzurzDemo"));
    assert!(body.contains("wellFormed"));
}

#[tokio::test]
async fn work_item_roundtrip_matches_the_azure_devops_client() {
    let app = app();
    let auth = basic("", "azurz-lab-pat");
    let patch = r#"[{"op":"add","path":"/fields/System.Title","value":"OEE Ligne 1"},{"op":"add","path":"/fields/System.Tags","value":"azurz; Plant"},{"op":"add","path":"/fields/System.State","value":"New"}]"#;
    let (status, body, _) = call(
        &app,
        "POST",
        "/azurz/AzurzDemo/_apis/wit/workitems/$User%20Story?api-version=7.1",
        Some(&auth),
        None,
        "application/json-patch+json",
        Some(patch),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let created: serde_json::Value = serde_json::from_str(&body).unwrap();
    let id = created["id"].as_i64().unwrap();
    assert_eq!(created["fields"]["System.Title"], "OEE Ligne 1");
    assert_eq!(created["fields"]["System.WorkItemType"], "User Story");
    assert_eq!(created["rev"], 1);

    let (status, body, _) = call(
        &app,
        "PATCH",
        &format!("/azurz/AzurzDemo/_apis/wit/workitems/{id}?api-version=7.1"),
        Some(&auth),
        None,
        "application/json-patch+json",
        Some(r#"[{"op":"replace","path":"/fields/System.State","value":"Active"}]"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let updated: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(updated["fields"]["System.State"], "Active");
    assert_eq!(updated["rev"], 2);

    let (status, body, _) = call(
        &app,
        "POST",
        &format!("/azurz/AzurzDemo/_apis/wit/workItems/{id}/comments?api-version=7.1-preview.3"),
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"text":"critère ajouté"}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");

    let (status, body, _) = call(
        &app,
        "POST",
        "/azurz/AzurzDemo/_apis/wit/wiql?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"query":"SELECT [System.Id] FROM WorkItems WHERE [System.Tags] CONTAINS 'azurz' AND [System.State] = 'Active'"}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let wiql: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(wiql["workItems"][0]["id"], id);
}

#[tokio::test]
async fn generic_resource_covers_a_product_area() {
    let app = app();
    let auth = basic("ada", "azurz-lab-pat");
    let (status, body, _) = call(
        &app,
        "POST",
        "/azurz/AzurzDemo/_apis/wiki/wikis?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"name":"Manuel"}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status, body, _) = call(
        &app,
        "GET",
        "/azurz/AzurzDemo/_apis/wiki/wikis?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(body.contains("Manuel"));

    let (status, body, _) = call(
        &app,
        "GET",
        "/_apis/resourceAreas?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    for area in ["wit", "git", "build", "pipelines", "test", "wiki", "packaging", "release"] {
        assert!(body.contains(area), "{area} absent de {body}");
    }
}

#[tokio::test]
async fn sign_in_opens_the_azure_devops_shell() {
    let app = app();
    let (status, body, _) = call(&app, "GET", "/", None, None, "text/html", None).await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("name=\"username\""));
    assert!(body.contains("name=\"password\""));
    assert!(body.contains("Azure DevOps"));

    let (status, body, _) = call(
        &app,
        "POST",
        "/_signin",
        None,
        None,
        "application/x-www-form-urlencoded",
        Some("username=ada&password=mauvais"),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert!(body.contains("refusé"));

    let (status, _body, cookie) = call(
        &app,
        "POST",
        "/_signin",
        None,
        None,
        "application/x-www-form-urlencoded",
        Some("username=ada&password=azurz-lab-pat"),
    )
    .await;
    assert_eq!(status, StatusCode::SEE_OTHER);
    let cookie = cookie.expect("cookie de session");
    let (status, body, _) = call(
        &app,
        "GET",
        "/azurz/AzurzDemo/_workitems",
        None,
        Some(&cookie),
        "text/html",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(body.contains("Éléments de travail"));
    assert!(body.contains("Demandes de tirage"));
    assert!(body.contains("Plans de test"));
    assert!(body.contains("Artefacts"));
    assert!(body.contains("Pipelines"));
}
