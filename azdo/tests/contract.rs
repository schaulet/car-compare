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

#[tokio::test]
async fn git_refs_and_pull_requests_follow_the_rest_shape() {
    let app = app();
    let auth = basic("azurz", "azurz-lab-pat");
    let (status, body, _) = call(
        &app,
        "GET",
        "/azurz/AzurzDemo/_apis/git/repositories?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let repos: serde_json::Value = serde_json::from_str(&body).unwrap();
    let repo = &repos["value"][0];
    assert_eq!(repo["name"], "AzurzDemo");
    assert_eq!(repo["defaultBranch"], "refs/heads/main");
    assert_eq!(repo["project"]["name"], "AzurzDemo");
    assert!(repo["remoteUrl"].as_str().unwrap().contains("AzurzDemo"));
    let repo_id = repo["id"].as_str().unwrap();

    let (status, body, _) = call(
        &app,
        "GET",
        &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/refs?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(body.contains("refs/heads/main"));
    let refs: serde_json::Value = serde_json::from_str(&body).unwrap();
    let object_id = refs["value"][0]["objectId"].as_str().unwrap();
    assert_eq!(object_id.len(), 40);

    let (status, body, _) = call(
        &app,
        "POST",
        &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/refs?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some(
            r#"[{"name":"refs/heads/feature","oldObjectId":"0000000000000000000000000000000000000000"}]"#,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let created_refs: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(created_refs["value"][0]["name"], "refs/heads/feature");
    assert_eq!(created_refs["value"][0]["success"], true);

    let (status, body, _) = call(
        &app,
        "POST",
        &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/pullrequests?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some(
            r#"{"title":"Revue OEE","description":"lab","sourceRefName":"refs/heads/feature","targetRefName":"refs/heads/main"}"#,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let pull: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert!(pull["pullRequestId"].as_i64().unwrap() > 0);
    assert_eq!(pull["status"], "active");
    assert_eq!(pull["title"], "Revue OEE");
    assert_eq!(pull["sourceRefName"], "refs/heads/feature");
    assert_eq!(pull["targetRefName"], "refs/heads/main");
    assert_eq!(pull["repository"]["name"], "AzurzDemo");
    assert!(pull["createdBy"]["displayName"].as_str().is_some());

    let (status, body, _) = call(
        &app,
        "GET",
        "/azurz/AzurzDemo/_apis/git/pullrequests?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(body.contains("Revue OEE"));

    let (status, body, _) = call(
        &app,
        "GET",
        &format!(
            "/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/items?path=/README.md&api-version=7.1"
        ),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let item: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(item["path"], "/README.md");
    assert_eq!(item["gitObjectType"], "blob");
    assert!(item["content"].as_str().unwrap().contains("AzurzDemo"));
}

#[tokio::test]
async fn builds_test_plans_and_feeds_follow_the_rest_shape() {
    let app = app();
    let auth = basic("azurz", "azurz-lab-pat");
    let (status, body, _) = call(
        &app,
        "GET",
        "/azurz/AzurzDemo/_apis/build/definitions?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let definitions: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(definitions["value"][0]["name"], "CI");
    assert_eq!(definitions["value"][0]["queueStatus"], "enabled");
    assert_eq!(definitions["value"][0]["type"], "build");
    let definition_id = definitions["value"][0]["id"].as_i64().unwrap();

    let (status, body, _) = call(
        &app,
        "POST",
        "/azurz/AzurzDemo/_apis/build/builds?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        Some(&format!(
            r#"{{"definition":{{"id":{definition_id}}},"sourceBranch":"refs/heads/main"}}"#
        )),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let queued: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(queued["status"], "notStarted");
    assert_eq!(queued["project"]["name"], "AzurzDemo");
    assert!(queued["buildNumber"].as_str().unwrap().contains('.'));
    let build_id = queued["id"].as_i64().unwrap();

    let (status, body, _) = call(
        &app,
        "GET",
        &format!("/azurz/AzurzDemo/_apis/build/builds/{build_id}?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let finished: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(finished["status"], "completed");
    assert_eq!(finished["result"], "succeeded");

    let (status, body, _) = call(
        &app,
        "POST",
        "/azurz/AzurzDemo/_apis/pipelines/pipelines/1/runs?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"resources":{"repositories":{"self":{"refName":"refs/heads/main"}}}}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let pipeline_run: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(pipeline_run["state"], "inProgress");
    let pipeline_run_id = pipeline_run["id"].as_i64().unwrap();
    let (status, body, _) = call(
        &app,
        "GET",
        &format!("/azurz/AzurzDemo/_apis/pipelines/runs/{pipeline_run_id}?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let pipeline_done: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(pipeline_done["state"], "completed");
    assert_eq!(pipeline_done["result"], "succeeded");

    let (status, body, _) = call(
        &app,
        "POST",
        "/azurz/AzurzDemo/_apis/testplan/plans?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"name":"Recette ligne 1"}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let plan: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(plan["name"], "Recette ligne 1");
    assert_eq!(plan["state"], "Active");
    assert_eq!(plan["areaPath"], "AzurzDemo");
    assert!(plan["rootSuite"]["id"].as_i64().unwrap() > 0);
    let plan_id = plan["id"].as_i64().unwrap();

    let (status, body, _) = call(
        &app,
        "POST",
        "/azurz/AzurzDemo/_apis/test/runs?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        Some(&format!(r#"{{"name":"Campagne 1","plan":{{"id":{plan_id}}}}}"#)),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let run: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(run["name"], "Campagne 1");
    assert_eq!(run["state"], "NotStarted");
    let run_id = run["id"].as_i64().unwrap();

    let (status, body, _) = call(
        &app,
        "POST",
        &format!("/azurz/AzurzDemo/_apis/test/runs/{run_id}/results?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some(r#"[{"testCaseTitle":"OEE","outcome":"Passed"}]"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status, body, _) = call(
        &app,
        "GET",
        &format!("/azurz/AzurzDemo/_apis/test/runs/{run_id}/results?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(body.contains("Passed"));
    assert!(body.contains("OEE"));

    let (status, body, _) = call(
        &app,
        "POST",
        "/azurz/AzurzDemo/_apis/packaging/feeds?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"name":"azurz"}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let feed: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(feed["name"], "azurz");
    assert!(feed["fullyQualifiedName"].as_str().unwrap().contains("azurz"));
}

#[tokio::test]
async fn work_item_catalog_wiki_pages_and_service_hooks_are_addressable() {
    let app = app();
    let auth = basic("azurz", "azurz-lab-pat");
    let (status, body, _) = call(
        &app,
        "GET",
        "/azurz/AzurzDemo/_apis/wit/workitemtypes?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    for kind in ["User Story", "Bug", "Task", "Feature", "Epic"] {
        assert!(body.contains(kind), "{kind} absent");
    }
    assert!(body.contains("Microsoft.VSTS.WorkItemTypes.UserStory"));

    let (status, body, _) = call(
        &app,
        "GET",
        "/azurz/AzurzDemo/_apis/wit/fields?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(body.contains("System.Title"));
    assert!(body.contains("System.State"));

    let (status, body, _) = call(
        &app,
        "GET",
        "/azurz/AzurzDemo/_apis/wit/classificationnodes/areas?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let areas: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(areas["name"], "AzurzDemo");
    assert_eq!(areas["structureType"], "area");

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
    let wiki: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(wiki["type"], "projectWiki");
    let wiki_id = wiki["id"].as_str().unwrap_or_else(|| wiki["name"].as_str().unwrap());

    let (status, body, _) = call(
        &app,
        "PUT",
        &format!("/azurz/AzurzDemo/_apis/wiki/wikis/{wiki_id}/pages?path=/Accueil&api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some("{\"content\":\"# Accueil du lab\"}"),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status, body, _) = call(
        &app,
        "GET",
        &format!("/azurz/AzurzDemo/_apis/wiki/wikis/{wiki_id}/pages?path=/Accueil&api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(body.contains("Accueil du lab"));

    let (status, body, _) = call(
        &app,
        "POST",
        "/azurz/_apis/hooks/subscriptions?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        Some(
            r#"{"publisherId":"tfs","eventType":"workitem.updated","consumerId":"webHooks","consumerActionId":"httpRequest","consumerInputs":{"url":"http://lab/hooks"}}"#,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let hook: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(hook["status"], "enabled");
    assert_eq!(hook["eventType"], "workitem.updated");
    let (status, body, _) = call(
        &app,
        "GET",
        "/azurz/_apis/hooks/subscriptions?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(body.contains("workitem.updated"));
}
