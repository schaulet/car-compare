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

#[tokio::test]
async fn relations_queries_and_boards_follow_the_rest_shape() {
    let app = app();
    let auth = basic("azurz", "azurz-lab-pat");
    let (status, body, _) = call(
        &app,
        "POST",
        "/azurz/AzurzDemo/_apis/wit/workitems/$User%20Story?api-version=7.1",
        Some(&auth),
        None,
        "application/json-patch+json",
        Some(r#"[{"op":"add","path":"/fields/System.Title","value":"Parent"},{"op":"add","path":"/fields/System.State","value":"Active"}]"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let parent: serde_json::Value = serde_json::from_str(&body).unwrap();
    let parent_id = parent["id"].as_i64().unwrap();
    let (status, body, _) = call(
        &app,
        "POST",
        "/azurz/AzurzDemo/_apis/wit/workitems/$Task?api-version=7.1",
        Some(&auth),
        None,
        "application/json-patch+json",
        Some(r#"[{"op":"add","path":"/fields/System.Title","value":"Enfant"}]"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let child: serde_json::Value = serde_json::from_str(&body).unwrap();
    let child_id = child["id"].as_i64().unwrap();
    let patch = format!(
        r#"[{{"op":"add","path":"/relations/-","value":{{"rel":"System.LinkTypes.Hierarchy-Forward","url":"/{child_id}","attributes":{{"comment":"enfant"}}}}}}]"#
    );
    let (status, body, _) = call(
        &app,
        "PATCH",
        &format!("/azurz/AzurzDemo/_apis/wit/workitems/{parent_id}?api-version=7.1"),
        Some(&auth),
        None,
        "application/json-patch+json",
        Some(&patch),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let linked: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(linked["relations"][0]["rel"], "System.LinkTypes.Hierarchy-Forward");
    assert!(linked["relations"][0]["url"].as_str().unwrap().contains(&child_id.to_string()));

    let (status, body, _) = call(
        &app,
        "POST",
        "/azurz/AzurzDemo/_apis/wit/attachments?fileName=preuve.txt&api-version=7.1",
        Some(&auth),
        None,
        "application/octet-stream",
        Some("preuve-lab"),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let attachment: serde_json::Value = serde_json::from_str(&body).unwrap();
    let attachment_id = attachment["id"].as_str().unwrap();
    let (status, body, _) = call(
        &app,
        "GET",
        &format!("/azurz/AzurzDemo/_apis/wit/attachments/{attachment_id}?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body, "preuve-lab");

    let (status, body, _) = call(
        &app,
        "POST",
        "/azurz/AzurzDemo/_apis/wit/queries?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"name":"Actifs","wiql":"SELECT [System.Id] FROM WorkItems WHERE [System.State] = 'Active'"}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let query: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(query["name"], "Actifs");
    let query_id = query["id"].to_string().trim_matches('"').to_string();
    let (status, body, _) = call(
        &app,
        "POST",
        &format!("/azurz/AzurzDemo/_apis/wit/wiql/{query_id}?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some("{}"),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let found: serde_json::Value = serde_json::from_str(&body).unwrap();
    let ids: Vec<i64> = found["workItems"].as_array().unwrap().iter().filter_map(|item| item["id"].as_i64()).collect();
    assert!(ids.contains(&parent_id), "{body}");

    let (status, body, _) = call(
        &app,
        "GET",
        "/azurz/AzurzDemo/AzurzDemo%20Team/_apis/work/boards?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let boards: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(boards["value"][0]["name"], "Stories");
    let (status, body, _) = call(
        &app,
        "GET",
        "/azurz/AzurzDemo/AzurzDemo%20Team/_apis/work/boards/Stories/columns?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(body.contains("Active"));
    let (status, body, _) = call(
        &app,
        "GET",
        "/azurz/AzurzDemo/AzurzDemo%20Team/_apis/work/backlogs/Microsoft.RequirementCategory/workItems?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(body.contains(&parent_id.to_string()), "{body}");
}

#[tokio::test]
async fn pull_request_threads_build_logs_and_dashboards_follow_the_rest_shape() {
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
    let repo_id = repos["value"][0]["id"].as_str().unwrap();
    let (status, body, _) = call(
        &app,
        "POST",
        &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/pullrequests?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"title":"Fil de revue","sourceRefName":"refs/heads/feature","targetRefName":"refs/heads/main"}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let pull: serde_json::Value = serde_json::from_str(&body).unwrap();
    let pull_id = pull["pullRequestId"].as_i64().unwrap();
    let (status, body, _) = call(
        &app,
        "POST",
        &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/pullrequests/{pull_id}/threads?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"comments":[{"content":"vu sur la ligne OEE"}],"status":"active"}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let thread: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(thread["comments"][0]["content"], "vu sur la ligne OEE");
    assert_eq!(thread["status"], "active");
    let (status, body, _) = call(
        &app,
        "GET",
        &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/pullrequests/{pull_id}/threads?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(body.contains("vu sur la ligne OEE"));
    let (status, body, _) = call(
        &app,
        "POST",
        &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/pullrequests/{pull_id}/statuses?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"state":"succeeded","description":"lab","context":{"name":"azurz","genre":"continuous-integration"}}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let status_doc: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(status_doc["state"], "succeeded");
    assert_eq!(status_doc["context"]["name"], "azurz");

    let (status, body, _) = call(
        &app,
        "POST",
        "/azurz/AzurzDemo/_apis/build/builds?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"definition":{"id":1},"sourceBranch":"refs/heads/main"}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let build: serde_json::Value = serde_json::from_str(&body).unwrap();
    let build_id = build["id"].as_i64().unwrap();
    let (status, body, _) = call(
        &app,
        "GET",
        &format!("/azurz/AzurzDemo/_apis/build/builds/{build_id}/logs/1?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(body.contains("Build succeeded"));
    let (status, body, _) = call(
        &app,
        "POST",
        &format!("/azurz/AzurzDemo/_apis/build/builds/{build_id}/artifacts?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"name":"drop","resource":{"type":"PipelineArtifact","data":"azurz"}}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status, body, _) = call(
        &app,
        "GET",
        &format!("/azurz/AzurzDemo/_apis/build/builds/{build_id}/artifacts?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(body.contains("drop"));

    let (status, body, _) = call(
        &app,
        "POST",
        "/azurz/AzurzDemo/_apis/dashboard/dashboards?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"name":"Vue lab","widgets":[{"name":"Work items"}]}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let dashboard: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(dashboard["name"], "Vue lab");
    assert_eq!(dashboard["widgets"][0]["name"], "Work items");

    let (status, body, _) = call(
        &app,
        "POST",
        "/azurz/AzurzDemo/_apis/serviceendpoint/endpoints?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"name":"ignition","type":"generic","url":"http://ignition:8088"}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let endpoint: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(endpoint["name"], "ignition");
    assert_eq!(endpoint["type"], "generic");
    assert_eq!(endpoint["url"], "http://ignition:8088");

    let (status, body, _) = call(
        &app,
        "GET",
        "/azurz/_apis/securitynamespaces?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(body.contains("Git"));
    assert!(body.contains("WorkItemTracking"));
}

#[tokio::test]
async fn tfvc_search_acls_and_package_protocols_follow_the_rest_shape() {
    let app = app();
    let auth = basic("azurz", "azurz-lab-pat");
    let (status, body, _) = call(
        &app,
        "POST",
        "/azurz/AzurzDemo/_apis/tfvc/changesets?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"comment":"init tfvc","changes":[{"changeType":"add","item":{"path":"$/README.txt","content":"depot AzurzDemo"}}]}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let changeset: serde_json::Value = serde_json::from_str(&body).unwrap();
    let changeset_id = changeset["changesetId"].as_i64().unwrap();
    assert_eq!(changeset["comment"], "init tfvc");
    let (status, body, _) = call(
        &app,
        "GET",
        &format!("/azurz/AzurzDemo/_apis/tfvc/changesets/{changeset_id}/changes?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(body.contains("$/README.txt"));
    let (status, body, _) = call(
        &app,
        "GET",
        "/azurz/AzurzDemo/_apis/tfvc/items?path=$/README.txt&api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let item: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(item["path"], "$/README.txt");
    assert!(item["content"].as_str().unwrap().contains("AzurzDemo"));

    let (status, body, _) = call(
        &app,
        "POST",
        "/azurz/AzurzDemo/_apis/wit/workitems/$User%20Story?api-version=7.1",
        Some(&auth),
        None,
        "application/json-patch+json",
        Some(r#"[{"op":"add","path":"/fields/System.Title","value":"Pompe recherche"}]"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let created: serde_json::Value = serde_json::from_str(&body).unwrap();
    let work_id = created["id"].as_i64().unwrap();
    let (status, body, _) = call(
        &app,
        "POST",
        "/azurz/AzurzDemo/_apis/search/workitemsearchresults?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"searchText":"Pompe","$top":10}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let found: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(found["results"][0]["fields"]["system.title"], "Pompe recherche");
    let (status, body, _) = call(
        &app,
        "POST",
        "/azurz/AzurzDemo/_apis/wit/workitemsbatch?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        Some(&format!(r#"{{"ids":[{work_id}],"fields":["System.Title"]}}"#)),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let batch: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(batch["value"][0]["id"], work_id);
    assert_eq!(batch["value"][0]["fields"]["System.Title"], "Pompe recherche");

    let (status, body, _) = call(
        &app,
        "POST",
        "/azurz/_apis/search/codesearchresults?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"searchText":"AzurzDemo"}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(body.contains("README.md") || body.contains("README.txt"), "{body}");

    let namespace = "2e9eb7ed-3c0a-47d4-87c1-0ffdd275fd87";
    let (status, body, _) = call(
        &app,
        "POST",
        &format!("/azurz/_apis/accesscontrolentries/{namespace}?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"token":"repoV2/azurz","merge":true,"accessControlEntries":[{"descriptor":"azurz","allow":4,"deny":0}]}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status, body, _) = call(
        &app,
        "GET",
        &format!("/azurz/_apis/accesscontrollists/{namespace}?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let acls: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(acls["value"][0]["token"], "repoV2/azurz");
    assert_eq!(acls["value"][0]["acesDictionary"]["azurz"]["allow"], 4);

    let (status, body, _) = call(
        &app,
        "POST",
        "/azurz/AzurzDemo/_apis/packaging/feeds?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"name":"azurz-packages"}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status, body, _) = call(
        &app,
        "GET",
        "/azurz/AzurzDemo/_packaging/azurz-packages/nuget/v3/index.json",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let index: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(index["version"], "3.0.0");
    assert!(index["resources"]
        .as_array()
        .unwrap()
        .iter()
        .any(|resource| resource["@type"].as_str().unwrap_or("").contains("PackageBaseAddress")));

    let (status, body, _) = call(
        &app,
        "PUT",
        "/azurz/AzurzDemo/_packaging/azurz-packages/npm/oee",
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"name":"oee","versions":{"1.0.0":{"version":"1.0.0"}}}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status, body, _) = call(
        &app,
        "GET",
        "/azurz/AzurzDemo/_packaging/azurz-packages/npm/oee",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let package: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(package["name"], "oee");
    assert_eq!(package["versions"]["1.0.0"]["version"], "1.0.0");
}

#[tokio::test]
async fn permissions_nuget_flat_container_and_universal_packages() {
    let app = app();
    let auth = basic("azurz", "azurz-lab-pat");
    let namespace = "2e9eb7ed-3c0a-47d4-87c1-0ffdd275fd87";
    let (status, body, _) = call(
        &app,
        "POST",
        &format!("/azurz/_apis/accesscontrolentries/{namespace}?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"token":"repoV2/azurz","merge":true,"accessControlEntries":[{"descriptor":"azurz","allow":4,"deny":0}]}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");

    let (status, body, _) = call(
        &app,
        "POST",
        "/azurz/_apis/security/permissionevaluationbatch?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        Some(&format!(
            r#"{{"alwaysAllowAdministrators":true,"evaluations":[{{"securityNamespaceId":"{namespace}","token":"repoV2/azurz","permissions":4}}]}}"#
        )),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let granted: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(granted["evaluations"][0]["value"], true);

    let (status, body, _) = call(
        &app,
        "POST",
        "/azurz/_apis/security/permissionevaluationbatch?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        Some(&format!(
            r#"{{"alwaysAllowAdministrators":true,"evaluations":[{{"securityNamespaceId":"{namespace}","token":"repoV2/azurz","permissions":8}}]}}"#
        )),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let denied: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(denied["evaluations"][0]["value"], false);

    let (status, body, _) = call(
        &app,
        "POST",
        "/azurz/AzurzDemo/_apis/packaging/feeds?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"name":"azurz-packages"}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status, body, _) = call(
        &app,
        "PUT",
        "/azurz/AzurzDemo/_packaging/azurz-packages/nuget/v3/flat2/oee/1.2.0/oee.1.2.0.nupkg",
        Some(&auth),
        None,
        "application/octet-stream",
        Some("nupkg-lab"),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status, body, _) = call(
        &app,
        "GET",
        "/azurz/AzurzDemo/_packaging/azurz-packages/nuget/v3/flat2/oee/index.json",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let versions: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert!(versions["versions"].as_array().unwrap().iter().any(|version| version == "1.2.0"), "{body}");
    let (status, body, _) = call(
        &app,
        "GET",
        "/azurz/AzurzDemo/_packaging/azurz-packages/nuget/v3/flat2/oee/1.2.0/oee.1.2.0.nupkg",
        Some(&auth),
        None,
        "application/octet-stream",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body, "nupkg-lab");

    let (status, body, _) = call(
        &app,
        "PUT",
        "/azurz/AzurzDemo/_apis/packaging/feeds/azurz-packages/upack/packages/module/versions/1.0.0",
        Some(&auth),
        None,
        "application/octet-stream",
        Some("paquet-universel"),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body, "paquet-universel");
    let (status, body, _) = call(
        &app,
        "GET",
        "/azurz/AzurzDemo/_apis/packaging/feeds/azurz-packages/upack/packages/module/versions/1.0.0",
        Some(&auth),
        None,
        "application/octet-stream",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body, "paquet-universel");
}

#[tokio::test]
async fn git_push_writes_files_and_a_pull_request_can_be_completed() {
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
    let repo_id = repos["value"][0]["id"].as_str().unwrap();
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
    let refs: serde_json::Value = serde_json::from_str(&body).unwrap();
    let old = refs["value"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["name"] == "refs/heads/main")
        .unwrap()["objectId"]
        .as_str()
        .unwrap()
        .to_string();
    let stale = "a".repeat(40);
    let (status, body, _) = call(
        &app,
        "POST",
        &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/pushes?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some(&format!(
            r#"{{"refUpdates":[{{"name":"refs/heads/main","oldObjectId":"{stale}"}}],"commits":[{{"comment":"ajout pompe","changes":[{{"changeType":"add","item":{{"path":"/src/pompe.txt"}},"newContent":{{"content":"debit 12","contentType":"rawtext"}}}}]}}]}}"#
        )),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");

    let (status, body, _) = call(
        &app,
        "POST",
        &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/pushes?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some(&format!(
            r#"{{"refUpdates":[{{"name":"refs/heads/main","oldObjectId":"{old}"}}],"commits":[{{"comment":"ajout pompe","changes":[{{"changeType":"add","item":{{"path":"/src/pompe.txt"}},"newContent":{{"content":"debit 12","contentType":"rawtext"}}}}]}}]}}"#
        )),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let push: serde_json::Value = serde_json::from_str(&body).unwrap();
    let commit_id = push["commits"][0]["commitId"].as_str().unwrap().to_string();
    assert_eq!(push["commits"][0]["comment"], "ajout pompe");
    assert_eq!(push["refUpdates"][0]["newObjectId"], commit_id);
    let (status, body, _) = call(
        &app,
        "GET",
        &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/commits/{commit_id}/changes?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(body.contains("/src/pompe.txt"), "{body}");
    let (status, body, _) = call(
        &app,
        "GET",
        &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/items?path=/src/pompe.txt&api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let file: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(file["content"], "debit 12");

    let (status, _, _) = call(
        &app,
        "POST",
        &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/refs?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some(&format!(r#"[{{"name":"refs/heads/feature","oldObjectId":"0000000000000000000000000000000000000000","newObjectId":"{commit_id}"}}]"#)),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, body, _) = call(
        &app,
        "POST",
        &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/pullrequests?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"title":"Revue pompe","sourceRefName":"refs/heads/feature","targetRefName":"refs/heads/main"}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let pull: serde_json::Value = serde_json::from_str(&body).unwrap();
    let pull_id = pull["pullRequestId"].as_i64().unwrap();
    let (status, body, _) = call(
        &app,
        "POST",
        &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/pullrequests/{pull_id}/reviewers?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"displayName":"ligne","uniqueName":"ligne","vote":10}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let reviewer: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(reviewer["displayName"], "ligne");
    assert_eq!(reviewer["vote"], 10);
    let (status, body, _) = call(
        &app,
        "GET",
        &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/pullrequests/{pull_id}/reviewers?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let reviewers: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(reviewers[0]["vote"], 10);
    let (status, body, _) = call(
        &app,
        "PATCH",
        &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/pullrequests/{pull_id}?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"status":"completed"}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let done: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(done["status"], "completed");
    let merge = done["lastMergeCommit"]["commitId"].as_str().unwrap().to_string();
    assert_ne!(merge, commit_id);
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
    let refs: serde_json::Value = serde_json::from_str(&body).unwrap();
    let main = refs["value"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["name"] == "refs/heads/main")
        .unwrap();
    assert_eq!(status, StatusCode::OK);
    assert_eq!(main["objectId"].as_str().unwrap(), merge.as_str(), "refs={body} pull={done}");
}

#[tokio::test]
async fn test_plan_suites_cases_and_points_record_a_result() {
    let app = app();
    let auth = basic("azurz", "azurz-lab-pat");
    let (status, body, _) = call(
        &app,
        "POST",
        "/azurz/AzurzDemo/_apis/testplan/plans?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"name":"Recette pompe"}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let plan: serde_json::Value = serde_json::from_str(&body).unwrap();
    let plan_id = plan["id"].as_i64().unwrap();
    let root_id = plan["rootSuite"]["id"].as_i64().unwrap();
    let (status, body, _) = call(
        &app,
        "GET",
        &format!("/azurz/AzurzDemo/_apis/testplan/plans/{plan_id}/suites?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let suites: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(suites["value"][0]["name"], "Recette pompe");
    let (status, body, _) = call(
        &app,
        "POST",
        &format!("/azurz/AzurzDemo/_apis/testplan/plans/{plan_id}/suites?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some(&format!(r#"{{"name":"Debit","parentSuite":{{"id":{root_id}}}}}"#)),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let suite: serde_json::Value = serde_json::from_str(&body).unwrap();
    let suite_id = suite["id"].as_i64().unwrap();
    assert_eq!(suite["suiteType"], "staticTestSuite");
    let (status, body, _) = call(
        &app,
        "POST",
        &format!("/azurz/AzurzDemo/_apis/testplan/plans/{plan_id}/suites/{suite_id}/testcases?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some(r#"[{"title":"Debit nominal"}]"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let cases: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(cases["value"][0]["workItem"]["name"], "Debit nominal");
    let (status, body, _) = call(
        &app,
        "GET",
        &format!("/azurz/AzurzDemo/_apis/testplan/plans/{plan_id}/suites/{suite_id}/testpoint?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let points: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(points["value"][0]["outcome"], "Unspecified");
    let point_id = points["value"][0]["id"].as_i64().unwrap();
    let (status, body, _) = call(
        &app,
        "POST",
        "/azurz/AzurzDemo/_apis/test/runs?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        Some(&format!(r#"{{"name":"Campagne pompe","plan":{{"id":{plan_id}}}}}"#)),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let run: serde_json::Value = serde_json::from_str(&body).unwrap();
    let run_id = run["id"].as_i64().unwrap();
    let (status, body, _) = call(
        &app,
        "POST",
        &format!("/azurz/AzurzDemo/_apis/test/runs/{run_id}/results?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some(&format!(r#"[{{"testPoint":{{"id":{point_id}}},"outcome":"Passed"}}]"#)),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status, body, _) = call(
        &app,
        "GET",
        &format!("/azurz/AzurzDemo/_apis/testplan/plans/{plan_id}/suites/{suite_id}/testpoint?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let points: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(points["value"][0]["outcome"], "Passed");
    let (status, body, _) = call(
        &app,
        "GET",
        &format!("/azurz/AzurzDemo/_apis/test/runs/{run_id}?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    let run: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(run["state"], "Completed");
    assert_eq!(run["passedTests"], 1);
}

#[tokio::test]
async fn release_definition_environments_wait_for_approval() {
    let app = app();
    let auth = basic("azurz", "azurz-lab-pat");
    let (status, body, _) = call(
        &app,
        "POST",
        "/azurz/AzurzDemo/_apis/release/definitions?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"name":"Deploiement ligne","environments":[{"name":"ligne-a","rank":1,"preDeployApprovals":{"approvals":[{"approver":{"displayName":"azurz"}}]}},{"name":"ligne-b","rank":2}]}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let definition: serde_json::Value = serde_json::from_str(&body).unwrap();
    let definition_id = definition["id"].as_i64().unwrap();
    assert_eq!(definition["environments"][0]["name"], "ligne-a");
    assert_eq!(definition["environments"][1]["name"], "ligne-b");
    let (status, body, _) = call(
        &app,
        "POST",
        "/azurz/AzurzDemo/_apis/release/releases?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        Some(&format!(r#"{{"definitionId":{definition_id},"description":"1.0.0"}}"#)),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let release: serde_json::Value = serde_json::from_str(&body).unwrap();
    let release_id = release["id"].as_i64().unwrap();
    assert!(release["name"].as_str().unwrap().starts_with("Release-"));
    assert_eq!(release["environments"][0]["status"], "inProgress");
    assert_eq!(release["environments"][1]["status"], "notStarted");
    let (status, body, _) = call(
        &app,
        "GET",
        "/azurz/AzurzDemo/_apis/release/approvals?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let approvals: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(approvals["value"][0]["status"], "pending");
    assert_eq!(approvals["value"][0]["releaseEnvironment"]["name"], "ligne-a");
    let approval_id = approvals["value"][0]["id"].as_i64().unwrap();
    let (status, body, _) = call(
        &app,
        "PATCH",
        &format!("/azurz/AzurzDemo/_apis/release/approvals/{approval_id}?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"status":"approved","comments":"ok"}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let approved: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(approved["status"], "approved");
    let (status, body, _) = call(
        &app,
        "GET",
        &format!("/azurz/AzurzDemo/_apis/release/releases/{release_id}?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let release: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(release["environments"][0]["status"], "succeeded");
    assert_eq!(release["environments"][1]["status"], "succeeded");
}

#[tokio::test]
async fn build_timeline_follows_status_and_accepts_agent_records() {
    let app = app();
    let auth = basic("azurz", "azurz-lab-pat");
    let (status, body, _) = call(
        &app,
        "GET",
        "/azurz/_apis/distributedtask/pools?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let pools: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(pools["value"][0]["name"], "Azure Pipelines");
    let (status, body, _) = call(
        &app,
        "GET",
        "/azurz/AzurzDemo/_apis/distributedtask/queues?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let queues: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(queues["value"][0]["name"], "Default");

    let (status, body, _) = call(
        &app,
        "POST",
        "/azurz/AzurzDemo/_apis/build/builds?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"definition":{"id":1},"sourceBranch":"refs/heads/main"}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let build: serde_json::Value = serde_json::from_str(&body).unwrap();
    let build_id = build["id"].as_i64().unwrap();
    assert_eq!(build["status"], "notStarted");
    assert_eq!(build["queue"]["name"], "Default");
    assert_eq!(build["queue"]["pool"]["name"], "Azure Pipelines");

    let (status, body, _) = call(
        &app,
        "GET",
        &format!("/azurz/AzurzDemo/_apis/build/builds/{build_id}/timeline?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let timeline: serde_json::Value = serde_json::from_str(&body).unwrap();
    let stage = timeline["records"]
        .as_array()
        .unwrap()
        .iter()
        .find(|record| record["type"] == "Stage")
        .unwrap();
    assert_eq!(stage["state"], "pending");
    assert!(stage["result"].is_null());

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
    let build: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(build["status"], "completed");
    assert_eq!(build["result"], "succeeded");

    let (status, body, _) = call(
        &app,
        "GET",
        &format!("/azurz/AzurzDemo/_apis/build/builds/{build_id}/timeline?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let timeline: serde_json::Value = serde_json::from_str(&body).unwrap();
    let records = timeline["records"].as_array().unwrap();
    assert!(records.iter().any(|record| record["type"] == "Stage" && record["result"] == "succeeded"));
    assert!(records.iter().any(|record| record["type"] == "Job" && record["result"] == "succeeded"));
    let task = records.iter().find(|record| record["name"] == "Build").unwrap();
    assert_eq!(task["type"], "Task");
    assert_eq!(task["result"], "succeeded");
    assert!(task["log"]["url"].as_str().unwrap().contains("/logs/1"));

    let (status, body, _) = call(
        &app,
        "PUT",
        &format!("/azurz/AzurzDemo/_apis/build/builds/{build_id}/timeline?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"records":[{"id":"task-compile","parentId":"job-1","type":"Task","name":"Compile","state":"completed","result":"succeeded"}]}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status, body, _) = call(
        &app,
        "GET",
        &format!("/azurz/AzurzDemo/_apis/build/builds/{build_id}/timeline?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let timeline: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert!(timeline["records"]
        .as_array()
        .unwrap()
        .iter()
        .any(|record| record["name"] == "Compile" && record["result"] == "succeeded"));
}

#[tokio::test]
async fn branch_policies_reject_then_approve_a_pull_request() {
    let app = app();
    let auth = basic("azurz", "azurz-lab-pat");
    let (status, body, _) = call(
        &app,
        "GET",
        "/azurz/AzurzDemo/_apis/policy/types?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let types: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert!(types["value"].as_array().unwrap().iter().any(|item| item["displayName"] == "Minimum number of reviewers"));
    assert!(types["value"].as_array().unwrap().iter().any(|item| item["id"] == "0609b952-1397-4640-95ec-e00a01b2c241"));

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
    let repo_id = repos["value"][0]["id"].as_str().unwrap();
    let (status, body, _) = call(
        &app,
        "POST",
        "/azurz/AzurzDemo/_apis/policy/configurations?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        Some(&format!(
            r#"{{"isEnabled":true,"isBlocking":true,"type":{{"id":"fa4e907d-c16b-4a4c-9dfa-4906e5d171dd"}},"settings":{{"minimumApproverCount":1,"scope":[{{"repositoryId":"{repo_id}","refName":"refs/heads/main","matchKind":"Exact"}}]}}}}"#
        )),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let configuration: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(configuration["type"]["displayName"], "Minimum number of reviewers");
    assert_eq!(configuration["isBlocking"], true);
    let (status, body, _) = call(
        &app,
        "POST",
        &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/pullrequests?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"title":"Strategie pompe","sourceRefName":"refs/heads/feature","targetRefName":"refs/heads/main"}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let pull: serde_json::Value = serde_json::from_str(&body).unwrap();
    let pull_id = pull["pullRequestId"].as_i64().unwrap();
    let artifact = format!("vstfs:///CodeReview/CodeReviewId/azurz-demo/{pull_id}");
    let (status, body, _) = call(
        &app,
        "POST",
        "/azurz/AzurzDemo/_apis/policy/evaluations?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        Some(&format!(r#"{{"artifactId":"{artifact}"}}"#)),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let evaluations: serde_json::Value = serde_json::from_str(&body).unwrap();
    let reviewers = evaluations["value"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["configuration"]["type"]["displayName"] == "Minimum number of reviewers")
        .unwrap();
    assert_eq!(reviewers["status"], "rejected");
    let (status, _, _) = call(
        &app,
        "POST",
        &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/pullrequests/{pull_id}/reviewers?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"displayName":"azurz","vote":10}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let encoded = artifact.replace('/', "%2F").replace(':', "%3A");
    let (status, body, _) = call(
        &app,
        "GET",
        &format!("/azurz/AzurzDemo/_apis/policy/evaluations?artifactId={encoded}&api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let evaluations: serde_json::Value = serde_json::from_str(&body).unwrap();
    let reviewers = evaluations["value"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["configuration"]["type"]["displayName"] == "Minimum number of reviewers")
        .unwrap();
    assert_eq!(reviewers["status"], "approved");

    let (status, body, _) = call(
        &app,
        "POST",
        "/azurz/AzurzDemo/_apis/policy/configurations?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        Some(&format!(
            r#"{{"isEnabled":true,"isBlocking":true,"type":{{"id":"0609b952-1397-4640-95ec-e00a01b2c241"}},"settings":{{"buildDefinitionId":1,"scope":[{{"repositoryId":"{repo_id}","refName":"refs/heads/main","matchKind":"Exact"}}]}}}}"#
        )),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status, body, _) = call(
        &app,
        "POST",
        "/azurz/AzurzDemo/_apis/policy/evaluations?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        Some(&format!(r#"{{"artifactId":"{artifact}"}}"#)),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let evaluations: serde_json::Value = serde_json::from_str(&body).unwrap();
    let build = evaluations["value"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["configuration"]["type"]["displayName"] == "Build")
        .unwrap();
    assert_eq!(build["status"], "rejected");
    let (status, body, _) = call(
        &app,
        "POST",
        "/azurz/AzurzDemo/_apis/build/builds?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"definition":{"id":1},"sourceBranch":"refs/heads/feature"}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let queued: serde_json::Value = serde_json::from_str(&body).unwrap();
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
    let (status, body, _) = call(
        &app,
        "POST",
        "/azurz/AzurzDemo/_apis/policy/evaluations?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        Some(&format!(r#"{{"artifactId":"{artifact}"}}"#)),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let evaluations: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert!(evaluations["value"].as_array().unwrap().iter().all(|item| item["status"] == "approved"));
}

#[tokio::test]
async fn git_diff_compares_two_commits_and_reads_a_file_at_that_version() {
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
    let repo_id = repos["value"][0]["id"].as_str().unwrap();
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
    let refs: serde_json::Value = serde_json::from_str(&body).unwrap();
    let base = refs["value"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["name"] == "refs/heads/main")
        .unwrap()["objectId"]
        .as_str()
        .unwrap()
        .to_string();
    let (status, body, _) = call(
        &app,
        "POST",
        &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/pushes?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some(&format!(
            r#"{{"refUpdates":[{{"name":"refs/heads/main","oldObjectId":"{base}"}}],"commits":[{{"comment":"ajout vanne","changes":[{{"changeType":"add","item":{{"path":"/src/vanne.txt"}},"newContent":{{"content":"debit 12","contentType":"rawtext"}}}}]}}]}}"#
        )),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let first: serde_json::Value = serde_json::from_str(&body).unwrap();
    let first_commit = first["commits"][0]["commitId"].as_str().unwrap().to_string();
    let (status, body, _) = call(
        &app,
        "POST",
        &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/pushes?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some(&format!(
            r#"{{"refUpdates":[{{"name":"refs/heads/main","oldObjectId":"{first_commit}"}}],"commits":[{{"comment":"reglage vanne","changes":[{{"changeType":"edit","item":{{"path":"/src/vanne.txt"}},"newContent":{{"content":"debit 18","contentType":"rawtext"}}}}]}}]}}"#
        )),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let second: serde_json::Value = serde_json::from_str(&body).unwrap();
    let second_commit = second["commits"][0]["commitId"].as_str().unwrap();
    let (status, body, _) = call(
        &app,
        "GET",
        &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/diffs/commits?baseVersion={first_commit}&targetVersion={second_commit}&baseVersionType=commit&targetVersionType=commit&api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let diff: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(diff["aheadCount"], 1);
    assert_eq!(diff["changes"][0]["changeType"], "edit");
    assert_eq!(diff["changes"][0]["item"]["path"], "/src/vanne.txt");
    assert_eq!(diff["changes"][0]["originalContent"]["content"], "debit 12");
    assert_eq!(diff["changes"][0]["newContent"]["content"], "debit 18");
    assert_eq!(diff["changeCounts"]["Edit"], 1);
    let (status, body, _) = call(
        &app,
        "GET",
        &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/diffs/commits?baseVersion={base}&targetVersion=refs/heads/main&baseVersionType=commit&targetVersionType=branch&api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let ranged: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(ranged["aheadCount"], 2);
    assert_eq!(ranged["changes"].as_array().unwrap().len(), 1);
    assert_eq!(ranged["changes"][0]["changeType"], "add");
    assert_eq!(ranged["changes"][0]["newContent"]["content"], "debit 18");
    let (status, body, _) = call(
        &app,
        "GET",
        &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/items?path=/src/vanne.txt&versionDescriptor.version={first_commit}&versionDescriptor.versionType=commit&api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let old: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(old["content"], "debit 12");
    let (status, body, _) = call(
        &app,
        "GET",
        &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/items?path=/src/vanne.txt&versionDescriptor.version={second_commit}&versionDescriptor.versionType=commit&api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let current: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(current["content"], "debit 18");
}

#[tokio::test]
async fn sprint_capacity_records_hours_and_team_days_off() {
    let app = app();
    let auth = basic("azurz", "azurz-lab-pat");
    let (status, body, _) = call(
        &app,
        "GET",
        "/azurz/AzurzDemo/_apis/work/teamsettings/iterations?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let iterations: serde_json::Value = serde_json::from_str(&body).unwrap();
    let sprint = &iterations["value"][0];
    assert_eq!(sprint["name"], "Sprint 1");
    assert_eq!(sprint["attributes"]["timeFrame"], "current");
    assert!(sprint["attributes"]["startDate"].as_str().unwrap().starts_with("2026-10-26"));
    let iteration_id = sprint["id"].as_str().unwrap_or("1");
    let (status, body, _) = call(
        &app,
        "PUT",
        &format!("/azurz/AzurzDemo/_apis/work/teamsettings/iterations/{iteration_id}/capacities/azurz?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"teamMember":{"displayName":"azurz","uniqueName":"azurz"},"activities":[{"name":"Development","capacityPerDay":6}],"daysOff":[{"start":"2026-10-27T00:00:00Z","end":"2026-10-27T00:00:00Z"}]}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let capacity: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(capacity["activities"][0]["capacityPerDay"], 6);
    assert_eq!(capacity["daysOff"][0]["start"], "2026-10-27T00:00:00Z");
    let (status, body, _) = call(
        &app,
        "PATCH",
        &format!("/azurz/AzurzDemo/_apis/work/teamsettings/iterations/{iteration_id}/capacities/azurz?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"activities":[{"name":"Development","capacityPerDay":8}]}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status, body, _) = call(
        &app,
        "GET",
        &format!("/azurz/AzurzDemo/_apis/work/teamsettings/iterations/{iteration_id}/capacities?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let capacities: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(capacities["value"][0]["teamMember"]["displayName"], "azurz");
    assert_eq!(capacities["value"][0]["activities"][0]["capacityPerDay"], 8);
    assert_eq!(capacities["value"][0]["daysOff"].as_array().unwrap().len(), 1);
    let (status, body, _) = call(
        &app,
        "PUT",
        &format!("/azurz/AzurzDemo/_apis/work/teamsettings/iterations/{iteration_id}/teamdaysoff?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"daysOff":[{"start":"2026-10-30T00:00:00Z","end":"2026-10-30T00:00:00Z"}]}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status, body, _) = call(
        &app,
        "GET",
        &format!("/azurz/AzurzDemo/_apis/work/teamsettings/iterations/{iteration_id}/teamdaysoff?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let days: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(days["daysOff"][0]["start"], "2026-10-30T00:00:00Z");
}

#[tokio::test]
async fn maven_and_pypi_publish_then_read_the_package() {
    let app = app();
    let auth = basic("azurz", "azurz-lab-pat");
    let (status, body, _) = call(
        &app,
        "POST",
        "/azurz/AzurzDemo/_apis/packaging/feeds?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"name":"ligne-packages"}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let pom = "<project><groupId>com.azurz</groupId><artifactId>oee</artifactId><version>1.0.0</version></project>";
    let (status, body, _) = call(
        &app,
        "PUT",
        "/azurz/AzurzDemo/_packaging/ligne-packages/maven/v1/com/azurz/oee/1.0.0/oee-1.0.0.pom",
        Some(&auth),
        None,
        "application/xml",
        Some(pom),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body, pom);
    let (status, body, _) = call(
        &app,
        "GET",
        "/azurz/AzurzDemo/_packaging/ligne-packages/maven/v1/com/azurz/oee/maven-metadata.xml",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(body.contains("<artifactId>oee</artifactId>"), "{body}");
    assert!(body.contains("<version>1.0.0</version>"), "{body}");
    let (status, body, _) = call(
        &app,
        "GET",
        "/azurz/AzurzDemo/_apis/packaging/feeds/ligne-packages/maven/packages?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let maven: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(maven["value"][0]["artifactId"], "oee");
    assert_eq!(maven["value"][0]["version"], "1.0.0");
    assert!(maven["value"][0].get("content").is_none());
    let archive = "paquet-pypi-lab";
    let (status, body, _) = call(
        &app,
        "PUT",
        "/azurz/AzurzDemo/_packaging/ligne-packages/pypi/simple/oee/oee-1.2.0.tar.gz",
        Some(&auth),
        None,
        "application/octet-stream",
        Some(archive),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body, archive);
    let (status, body, _) = call(
        &app,
        "GET",
        "/azurz/AzurzDemo/_packaging/ligne-packages/pypi/simple/oee/",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(body.contains("oee-1.2.0.tar.gz"), "{body}");
    assert!(body.contains("Links for oee"), "{body}");
    let (status, body, _) = call(
        &app,
        "GET",
        "/azurz/AzurzDemo/_packaging/ligne-packages/pypi/simple/oee/oee-1.2.0.tar.gz",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body, archive);
    let (status, body, _) = call(
        &app,
        "GET",
        "/azurz/AzurzDemo/_apis/packaging/feeds/ligne-packages/pypi/packages?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let pypi: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(pypi["value"][0]["name"], "oee");
    assert_eq!(pypi["value"][0]["version"], "1.2.0");
    assert_eq!(pypi["value"][0]["filename"], "oee-1.2.0.tar.gz");
}
