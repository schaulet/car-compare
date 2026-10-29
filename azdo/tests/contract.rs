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

#[tokio::test]
async fn pool_agent_comes_online_and_finishes_a_job() {
    let app = app();
    let auth = basic("azurz", "azurz-lab-pat");
    let (status, body, _) = call(
        &app,
        "POST",
        "/azurz/_apis/distributedtask/pools/1/agents?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"name":"azdo-linux","version":"3.248.0","osDescription":"Ubuntu 24.04","status":"offline"}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let created: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(created["name"], "azdo-linux");
    assert_eq!(created["status"], "offline");
    assert_eq!(created["osDescription"], "Ubuntu 24.04");
    let agent_id = created["id"].as_i64().unwrap();
    let (status, body, _) = call(
        &app,
        "GET",
        "/azurz/_apis/distributedtask/pools/1/agents?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let agents: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(agents["count"], 1);
    let (status, body, _) = call(
        &app,
        "PATCH",
        &format!("/azurz/_apis/distributedtask/pools/1/agents/{agent_id}?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"status":"online","systemCapabilities":{"Agent.OS":"Linux","Agent.Version":"3.248.0"}}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let online: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(online["status"], "online");
    assert_eq!(online["systemCapabilities"]["Agent.OS"], "Linux");
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
    assert_eq!(pools["value"][0]["size"], 1);
    let (status, body, _) = call(
        &app,
        "POST",
        "/azurz/_apis/distributedtask/pools/1/jobrequests?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        Some(&format!(r#"{{"agentId":{agent_id},"planType":"Build","definition":{{"id":1,"name":"CI"}}}}"#)),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let request: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(request["status"], "queued");
    assert_eq!(request["reservedAgent"]["name"], "azdo-linux");
    assert!(request["result"].is_null());
    let request_id = request["requestId"].as_i64().unwrap();
    let (status, body, _) = call(
        &app,
        "GET",
        &format!("/azurz/_apis/distributedtask/pools/1/jobrequests?agentId={agent_id}&api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let queued: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(queued["value"][0]["requestId"], request_id);
    let (status, body, _) = call(
        &app,
        "PATCH",
        &format!("/azurz/_apis/distributedtask/pools/1/jobrequests/{request_id}?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"result":"succeeded"}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let finished: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(finished["status"], "completed");
    assert_eq!(finished["result"], "succeeded");
}

#[tokio::test]
async fn service_hook_delivers_a_work_item_to_the_consumer() {
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::time::Duration;
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let (sender, receiver) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let (mut socket, _) = listener.accept().unwrap();
        socket.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
        let mut total = Vec::new();
        let mut buffer = [0u8; 2048];
        loop {
            match socket.read(&mut buffer) {
                Ok(0) => break,
                Ok(count) => {
                    total.extend_from_slice(&buffer[..count]);
                    let text = String::from_utf8_lossy(&total);
                    let Some(header_end) = text.find("\r\n\r\n") else {
                        continue;
                    };
                    let length = text
                        .lines()
                        .find_map(|line| line.split_once(':').and_then(|(name, value)| name.eq_ignore_ascii_case("content-length").then(|| value.trim().parse::<usize>().ok())))
                        .flatten()
                        .unwrap_or(0);
                    if total.len() >= header_end + 4 + length {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
        let _ = socket.write_all(b"HTTP/1.1 204 No Content\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
        let _ = sender.send(String::from_utf8_lossy(&total).into_owned());
    });

    let app = app();
    let auth = basic("azurz", "azurz-lab-pat");
    let (status, body, _) = call(
        &app,
        "POST",
        "/azurz/_apis/hooks/subscriptions?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        Some(&format!(
            r#"{{"publisherId":"tfs","eventType":"workitem.created","consumerId":"webHooks","consumerActionId":"httpRequest","consumerInputs":{{"url":"http://127.0.0.1:{port}/consumer"}},"publisherInputs":{{"projectId":"AzurzDemo"}}}}"#
        )),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let created_hook: serde_json::Value = serde_json::from_str(&body).unwrap();
    let created_id = created_hook["id"].as_str().unwrap();
    let (status, body, _) = call(
        &app,
        "POST",
        "/azurz/_apis/hooks/subscriptions?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"publisherId":"tfs","eventType":"workitem.updated","consumerId":"webHooks","consumerActionId":"httpRequest","consumerInputs":{"url":"/_hooks/inbox"}}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let updated_hook: serde_json::Value = serde_json::from_str(&body).unwrap();
    let updated_id = updated_hook["id"].as_str().unwrap();
    let (status, body, _) = call(
        &app,
        "POST",
        "/azurz/_apis/hooks/subscriptions?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"eventType":"workitem.created","consumerInputs":{"url":"/_hooks/inbox"},"publisherInputs":{"projectId":"Autre"}}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let other: serde_json::Value = serde_json::from_str(&body).unwrap();
    let other_id = other["id"].as_str().unwrap();
    let (status, body, _) = call(
        &app,
        "POST",
        "/azurz/AzurzDemo/_apis/wit/workitems/$Task?api-version=7.1",
        Some(&auth),
        None,
        "application/json-patch+json",
        Some(r#"[{"op":"add","path":"/fields/System.Title","value":"Pompe hook"}]"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let item: serde_json::Value = serde_json::from_str(&body).unwrap();
    let item_id = item["id"].as_i64().unwrap();
    let delivered = receiver.recv_timeout(Duration::from_secs(2)).expect("consommateur");
    assert!(delivered.contains("workitem.created"), "{delivered}");
    assert!(delivered.contains("Pompe hook"), "{delivered}");
    let (status, body, _) = call(
        &app,
        "GET",
        &format!("/azurz/_apis/hooks/subscriptions/{created_id}/notifications?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let notes: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(notes["value"][0]["status"], "succeeded");
    assert_eq!(notes["value"][0]["details"], "HTTP 204");
    let (status, body, _) = call(
        &app,
        "GET",
        &format!("/azurz/_apis/hooks/subscriptions/{other_id}/notifications?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let skipped: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(skipped["count"], 0);
    let (status, body, _) = call(
        &app,
        "PATCH",
        &format!("/azurz/AzurzDemo/_apis/wit/workitems/{item_id}?api-version=7.1"),
        Some(&auth),
        None,
        "application/json-patch+json",
        Some(r#"[{"op":"replace","path":"/fields/System.Title","value":"Pompe mise a jour"}]"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status, body, _) = call(
        &app,
        "GET",
        "/azurz/_apis/hooks/inbox?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let inbox: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(inbox["value"][0]["eventType"], "workitem.updated");
    assert_eq!(inbox["value"][0]["resource"]["fields"]["System.Title"], "Pompe mise a jour");
    let (status, body, _) = call(
        &app,
        "GET",
        &format!("/azurz/_apis/hooks/subscriptions/{updated_id}/notifications?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let updated_notes: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(updated_notes["value"][0]["status"], "succeeded");
    assert_eq!(updated_notes["value"][0]["details"], "livré");
}

#[tokio::test]
async fn dashboard_widgets_render_markdown_query_and_build() {
    let app = app();
    let auth = basic("azurz", "azurz-lab-pat");
    let (status, body, _) = call(
        &app,
        "POST",
        "/azurz/AzurzDemo/_apis/dashboard/dashboards?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"name":"Vue ligne"}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let dashboard: serde_json::Value = serde_json::from_str(&body).unwrap();
    let dashboard_id = dashboard["id"].as_str().unwrap();
    let (status, body, _) = call(
        &app,
        "POST",
        &format!("/azurz/AzurzDemo/_apis/dashboard/dashboards/{dashboard_id}/widgets?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"name":"Note","contributionId":"ms.vss-dashboards.MarkdownWidget","settings":{"content":"Debit nominal"},"size":{"rowSpan":1,"columnSpan":2}}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let markdown: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(markdown["rendered"]["kind"], "markdown");
    assert_eq!(markdown["rendered"]["text"], "Debit nominal");
    let widget_id = markdown["id"].as_str().unwrap();
    let (status, _, _) = call(
        &app,
        "POST",
        "/azurz/AzurzDemo/_apis/wit/workitems/$Task?api-version=7.1",
        Some(&auth),
        None,
        "application/json-patch+json",
        Some(r#"[{"op":"add","path":"/fields/System.Title","value":"Tache tableau"},{"op":"add","path":"/fields/System.State","value":"Active"}]"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, body, _) = call(
        &app,
        "POST",
        &format!("/azurz/AzurzDemo/_apis/dashboard/dashboards/{dashboard_id}/widgets?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"name":"Work items actifs","contributionId":"ms.vss-dashboards.QueryScalarWidget","settings":{"state":"Active"}}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let query: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(query["rendered"]["kind"], "query");
    assert_eq!(query["rendered"]["value"], 1);
    let (status, body, _) = call(
        &app,
        "POST",
        "/azurz/AzurzDemo/_apis/build/builds?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"definition":{"id":1}}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let build: serde_json::Value = serde_json::from_str(&body).unwrap();
    let build_id = build["id"].as_i64().unwrap();
    let (status, _, _) = call(
        &app,
        "GET",
        &format!("/azurz/AzurzDemo/_apis/build/builds/{build_id}?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, body, _) = call(
        &app,
        "POST",
        &format!("/azurz/AzurzDemo/_apis/dashboard/dashboards/{dashboard_id}/widgets?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"name":"Build CI","contributionId":"ms.vss-dashboards.BuildWidget","settings":{"definitionId":1}}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let build_widget: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(build_widget["rendered"]["kind"], "build");
    assert_eq!(build_widget["rendered"]["value"], "succeeded");
    let (status, body, _) = call(
        &app,
        "PATCH",
        &format!("/azurz/AzurzDemo/_apis/dashboard/dashboards/{dashboard_id}/widgets/{widget_id}?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"settings":{"content":"Debit 18"}}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let edited: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(edited["rendered"]["text"], "Debit 18");
    let (status, body, _) = call(
        &app,
        "GET",
        &format!("/azurz/AzurzDemo/_apis/dashboard/dashboards/{dashboard_id}/widgets?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let widgets: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(widgets["count"], 3);
    let (status, _, _) = call(
        &app,
        "DELETE",
        &format!("/azurz/AzurzDemo/_apis/dashboard/dashboards/{dashboard_id}/widgets/{widget_id}?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (status, body, _) = call(
        &app,
        "GET",
        &format!("/azurz/AzurzDemo/_apis/dashboard/dashboards/{dashboard_id}?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let stored: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(stored["widgets"].as_array().unwrap().len(), 2);
    assert_eq!(stored["widgets"][0]["rendered"]["value"], 1);
}

#[tokio::test]
async fn saved_queries_live_in_folders_and_run_the_updated_wiql() {
    let app = app();
    let auth = basic("azurz", "azurz-lab-pat");
    let (status, body, _) = call(
        &app,
        "GET",
        "/azurz/AzurzDemo/_apis/wit/queries?$depth=2&api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let tree: serde_json::Value = serde_json::from_str(&body).unwrap();
    let names: Vec<&str> = tree["value"].as_array().unwrap().iter().filter_map(|item| item["name"].as_str()).collect();
    assert!(names.contains(&"Shared Queries"), "{body}");
    assert!(names.contains(&"My Queries"), "{body}");
    let (status, _, _) = call(
        &app,
        "POST",
        "/azurz/AzurzDemo/_apis/wit/workitems/$Task?api-version=7.1",
        Some(&auth),
        None,
        "application/json-patch+json",
        Some(r#"[{"op":"add","path":"/fields/System.Title","value":"Brouillon"},{"op":"add","path":"/fields/System.State","value":"New"}]"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, body, _) = call(
        &app,
        "POST",
        "/azurz/AzurzDemo/_apis/wit/workitems/$Task?api-version=7.1",
        Some(&auth),
        None,
        "application/json-patch+json",
        Some(r#"[{"op":"add","path":"/fields/System.Title","value":"Pompe active"},{"op":"add","path":"/fields/System.State","value":"Active"}]"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let active: serde_json::Value = serde_json::from_str(&body).unwrap();
    let active_id = active["id"].as_i64().unwrap();
    let (status, body, _) = call(
        &app,
        "POST",
        "/azurz/AzurzDemo/_apis/wit/queries/shared-queries?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"name":"Actifs ligne","wiql":"SELECT [System.Id] FROM WorkItems WHERE [System.State] = 'Active'"}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let query: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(query["path"], "Shared Queries/Actifs ligne");
    let query_id = query["id"].as_str().unwrap();
    let (status, body, _) = call(
        &app,
        "POST",
        "/azurz/AzurzDemo/_apis/wit/queries/shared-queries?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"name":"Ligne","isFolder":true}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let folder: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(folder["isFolder"], true);
    assert_eq!(folder["path"], "Shared Queries/Ligne");
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
    assert_eq!(ids, vec![active_id]);
    let (status, body, _) = call(
        &app,
        "PATCH",
        &format!("/azurz/AzurzDemo/_apis/wit/queries/{query_id}?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"wiql":"SELECT [System.Id] FROM WorkItems WHERE [System.State] = 'New'"}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
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
    let revised: serde_json::Value = serde_json::from_str(&body).unwrap();
    let titles_ids: Vec<i64> = revised["workItems"].as_array().unwrap().iter().filter_map(|item| item["id"].as_i64()).collect();
    assert!(!titles_ids.contains(&active_id), "{body}");
    assert_eq!(titles_ids.len(), 1);
    let (status, _, _) = call(
        &app,
        "DELETE",
        "/azurz/AzurzDemo/_apis/wit/queries/shared-queries?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (status, _, _) = call(
        &app,
        "DELETE",
        &format!("/azurz/AzurzDemo/_apis/wit/queries/{query_id}?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (status, _, _) = call(
        &app,
        "GET",
        &format!("/azurz/AzurzDemo/_apis/wit/queries/{query_id}?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn commit_statuses_keep_history_and_the_latest_per_context() {
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
        &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/commits?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let commits: serde_json::Value = serde_json::from_str(&body).unwrap();
    let commit_id = commits["value"][0]["commitId"].as_str().unwrap();
    let list_path = format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/commits/{commit_id}/statuses?api-version=7.1");
    let (status, body, _) = call(&app, "GET", &list_path, Some(&auth), None, "application/json", None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let empty: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(empty["count"], 0);
    let (status, body, _) = call(
        &app,
        "POST",
        &list_path,
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"state":"pending","description":"en cours","targetUrl":"http://lab/build/1","context":{"name":"ci","genre":"continuous-integration"}}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let pending: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(pending["state"], "pending");
    assert_eq!(pending["context"]["name"], "ci");
    assert_eq!(pending["targetUrl"], "http://lab/build/1");
    let (status, body, _) = call(
        &app,
        "POST",
        &list_path,
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"state":"succeeded","description":"verte","context":{"name":"ci","genre":"continuous-integration"}}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status, body, _) = call(
        &app,
        "POST",
        &list_path,
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"state":"failed","description":"alerte","context":{"name":"audit","genre":"security"}}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status, body, _) = call(&app, "GET", &list_path, Some(&auth), None, "application/json", None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let history: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(history["count"], 3);
    assert_eq!(history["value"][0]["state"], "failed");
    assert_eq!(history["value"][0]["description"], "alerte");
    let latest_path = format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/commits/{commit_id}/statuses?latestOnly=true&api-version=7.1");
    let (status, body, _) = call(&app, "GET", &latest_path, Some(&auth), None, "application/json", None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let latest: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(latest["count"], 2, "{body}");
    let mut pairs: Vec<(String, String)> = latest["value"]
        .as_array()
        .unwrap()
        .iter()
        .map(|item| {
            (
                item["context"]["name"].as_str().unwrap().to_string(),
                item["state"].as_str().unwrap().to_string(),
            )
        })
        .collect();
    pairs.sort();
    assert_eq!(pairs, vec![("audit".to_string(), "failed".to_string()), ("ci".to_string(), "succeeded".to_string())]);
    let (status, _, _) = call(
        &app,
        "POST",
        &list_path,
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"state":"running","context":{"name":"ci"}}"#),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (status, _, _) = call(
        &app,
        "POST",
        &list_path,
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"state":"succeeded"}"#),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (status, _, _) = call(
        &app,
        "POST",
        &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/commits/0000000000000000000000000000000000000000/statuses?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"state":"succeeded","context":{"name":"ci"}}"#),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn teams_keep_members_and_their_own_backlog_settings() {
    let app = app();
    let auth = basic("azurz", "azurz-lab-pat");
    let (status, body, _) = call(
        &app,
        "GET",
        "/azurz/_apis/projects/AzurzDemo/teams?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let listed: serde_json::Value = serde_json::from_str(&body).unwrap();
    let default_team = listed["value"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["name"] == "AzurzDemo Team")
        .unwrap();
    assert_eq!(default_team["projectName"], "AzurzDemo");
    assert_eq!(default_team["projectId"], "azurz-demo");
    let (status, body, _) = call(
        &app,
        "POST",
        "/azurz/_apis/projects/AzurzDemo/teams?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"name":"Ligne A","description":"Atelier"}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let created: serde_json::Value = serde_json::from_str(&body).unwrap();
    let team_id = created["id"].as_str().unwrap().to_string();
    assert_eq!(created["description"], "Atelier");
    let (status, body, _) = call(
        &app,
        "PATCH",
        &format!("/azurz/_apis/projects/AzurzDemo/teams/{team_id}?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"description":"Atelier nord"}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let patched: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(patched["name"], "Ligne A");
    assert_eq!(patched["description"], "Atelier nord");
    let (status, body, _) = call(
        &app,
        "GET",
        &format!("/azurz/_apis/projects/AzurzDemo/teams/{team_id}/members?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let members: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(members["value"][0]["identity"]["uniqueName"], "azurz");
    assert_eq!(members["value"][0]["isTeamAdmin"], true);
    let (status, body, _) = call(
        &app,
        "POST",
        &format!("/azurz/_apis/projects/AzurzDemo/teams/{team_id}/members?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"displayName":"Nadia","uniqueName":"nadia@azurz.lab","isTeamAdmin":false}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status, body, _) = call(
        &app,
        "GET",
        &format!("/azurz/_apis/projects/AzurzDemo/teams/{team_id}/members?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let members: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(members["count"], 2);
    let (status, body, _) = call(
        &app,
        "GET",
        "/azurz/AzurzDemo/Ligne%20A/_apis/work/teamsettings?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let settings: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(settings["bugsBehavior"], "off");
    assert_eq!(settings["defaultIterationMacro"], "@currentIteration");
    let (status, body, _) = call(
        &app,
        "PATCH",
        "/azurz/AzurzDemo/Ligne%20A/_apis/work/teamsettings?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"bugsBehavior":"asTasks","workingDays":["monday","tuesday","wednesday"]}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let updated: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(updated["bugsBehavior"], "asTasks");
    assert_eq!(updated["workingDays"].as_array().unwrap().len(), 3);
    let (status, body, _) = call(
        &app,
        "GET",
        "/azurz/AzurzDemo/_apis/work/teamsettings?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let defaults: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(defaults["bugsBehavior"], "off", "{body}");
    let (status, _, _) = call(
        &app,
        "PATCH",
        "/azurz/AzurzDemo/Ligne%20A/_apis/work/teamsettings?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"bugsBehavior":"asBugs"}"#),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (status, _, _) = call(
        &app,
        "DELETE",
        "/azurz/_apis/projects/AzurzDemo/teams/1?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (status, _, _) = call(
        &app,
        "DELETE",
        &format!("/azurz/_apis/projects/AzurzDemo/teams/{team_id}?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (status, _, _) = call(
        &app,
        "GET",
        &format!("/azurz/_apis/projects/AzurzDemo/teams/{team_id}?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn pull_request_work_items_follow_artifact_links() {
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
        Some(r#"{"title":"Lier la pompe","sourceRefName":"refs/heads/feature","targetRefName":"refs/heads/main"}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let pull: serde_json::Value = serde_json::from_str(&body).unwrap();
    let pull_id = pull["pullRequestId"].as_i64().unwrap();
    let (status, body, _) = call(
        &app,
        "POST",
        "/azurz/AzurzDemo/_apis/wit/workitems/$Task?api-version=7.1",
        Some(&auth),
        None,
        "application/json-patch+json",
        Some(r#"[{"op":"add","path":"/fields/System.Title","value":"Pompe liee"}]"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let linked: serde_json::Value = serde_json::from_str(&body).unwrap();
    let linked_id = linked["id"].as_i64().unwrap();
    let (status, _, _) = call(
        &app,
        "POST",
        "/azurz/AzurzDemo/_apis/wit/workitems/$Task?api-version=7.1",
        Some(&auth),
        None,
        "application/json-patch+json",
        Some(r#"[{"op":"add","path":"/fields/System.Title","value":"Hors demande"}]"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let list_path = format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/pullrequests/{pull_id}/workitems?api-version=7.1");
    let (status, body, _) = call(&app, "GET", &list_path, Some(&auth), None, "application/json", None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let empty: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(empty["count"], 0);
    let artifact = format!("vstfs:///Git/PullRequestId/azurz-demo%2F{repo_id}%2F{pull_id}");
    let patch = format!(
        r#"[{{"op":"add","path":"/relations/-","value":{{"rel":"ArtifactLink","url":"{artifact}","attributes":{{"name":"Pull Request"}}}}}}]"#
    );
    let (status, body, _) = call(
        &app,
        "PATCH",
        &format!("/azurz/AzurzDemo/_apis/wit/workitems/{linked_id}?api-version=7.1"),
        Some(&auth),
        None,
        "application/json-patch+json",
        Some(&patch),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let other = format!("vstfs:///Git/PullRequestId/azurz-demo%2F{repo_id}%2F999");
    let other_patch = format!(
        r#"[{{"op":"add","path":"/relations/-","value":{{"rel":"ArtifactLink","url":"{other}","attributes":{{"name":"Pull Request"}}}}}}]"#
    );
    let (status, body, _) = call(
        &app,
        "GET",
        "/azurz/AzurzDemo/_apis/wit/workitems?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let items: serde_json::Value = serde_json::from_str(&body).unwrap();
    let other_id = items["value"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["fields"]["System.Title"] == "Hors demande")
        .unwrap()["id"]
        .as_i64()
        .unwrap();
    let (status, _, _) = call(
        &app,
        "PATCH",
        &format!("/azurz/AzurzDemo/_apis/wit/workitems/{other_id}?api-version=7.1"),
        Some(&auth),
        None,
        "application/json-patch+json",
        Some(&other_patch),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, body, _) = call(&app, "GET", &list_path, Some(&auth), None, "application/json", None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let found: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(found["count"], 1, "{body}");
    assert_eq!(found["value"][0]["id"], linked_id.to_string());
    let (status, _, _) = call(
        &app,
        "PATCH",
        &format!("/azurz/AzurzDemo/_apis/wit/workitems/{linked_id}?api-version=7.1"),
        Some(&auth),
        None,
        "application/json-patch+json",
        Some(r#"[{"op":"remove","path":"/relations/0"}]"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, body, _) = call(&app, "GET", &list_path, Some(&auth), None, "application/json", None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let cleared: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(cleared["count"], 0, "{body}");
    let (status, _, _) = call(
        &app,
        "GET",
        &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/pullrequests/99999/workitems?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn classification_nodes_keep_children_when_a_parent_is_renamed() {
    let app = app();
    let auth = basic("azurz", "azurz-lab-pat");
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
    let root: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(root["name"], "AzurzDemo");
    assert_eq!(root["structureType"], "area");
    let (status, body, _) = call(
        &app,
        "POST",
        "/azurz/AzurzDemo/_apis/wit/classificationnodes/areas?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"name":"Ligne A"}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let area: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(area["path"], "\\AzurzDemo\\Area\\Ligne A");
    let (status, body, _) = call(
        &app,
        "POST",
        "/azurz/AzurzDemo/_apis/wit/classificationnodes/areas/Ligne%20A?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"name":"Machine"}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let child: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(child["path"], "\\AzurzDemo\\Area\\Ligne A\\Machine");
    let (status, body, _) = call(
        &app,
        "PATCH",
        "/azurz/AzurzDemo/_apis/wit/classificationnodes/areas/Ligne%20A?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"name":"Ligne nord"}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status, body, _) = call(
        &app,
        "GET",
        "/azurz/AzurzDemo/_apis/wit/classificationnodes/areas/Ligne%20nord/Machine?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let moved: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(moved["path"], "\\AzurzDemo\\Area\\Ligne nord\\Machine");
    let (status, body, _) = call(
        &app,
        "POST",
        "/azurz/AzurzDemo/_apis/wit/classificationnodes/iterations?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"name":"Sprint 2","attributes":{"startDate":"2026-11-09T00:00:00Z","finishDate":"2026-11-20T00:00:00Z"}}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let sprint: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(sprint["attributes"]["startDate"], "2026-11-09T00:00:00Z");
    let (status, body, _) = call(
        &app,
        "GET",
        "/azurz/AzurzDemo/_apis/wit/classificationnodes/iterations?$depth=2&api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(body.contains("Sprint 1"), "{body}");
    assert!(body.contains("Sprint 2"), "{body}");
    let (status, _, _) = call(
        &app,
        "DELETE",
        "/azurz/AzurzDemo/_apis/wit/classificationnodes/areas?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (status, _, _) = call(
        &app,
        "POST",
        "/azurz/AzurzDemo/_apis/wit/classificationnodes/areas?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"name":"Ligne nord"}"#),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (status, _, _) = call(
        &app,
        "DELETE",
        "/azurz/AzurzDemo/_apis/wit/classificationnodes/areas/Ligne%20nord/Machine?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (status, _, _) = call(
        &app,
        "GET",
        "/azurz/AzurzDemo/_apis/wit/classificationnodes/areas/Ligne%20nord/Machine?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn annotated_tag_points_at_a_commit_and_can_be_deleted() {
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
        &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/refs?filter=heads&api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let heads: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert!(body.contains("refs/heads/main"), "{body}");
    assert!(!body.contains("refs/tags/"), "{body}");
    let commit_id = heads["value"]
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
        &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/annotatedtags?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some(&format!(
            r#"{{"name":"v1.0.0","message":"Première version","taggedObject":{{"objectId":"{commit_id}"}}}}"#
        )),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let tag: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(tag["name"], "v1.0.0");
    assert_eq!(tag["message"], "Première version");
    assert_eq!(tag["taggedObject"]["objectId"], commit_id);
    assert_eq!(tag["taggedObject"]["objectType"], "commit");
    assert_eq!(tag["taggedBy"]["name"], "azurz");
    let tag_id = tag["objectId"].as_str().unwrap();
    assert_eq!(tag_id.len(), 40);
    assert_ne!(tag_id, commit_id);

    let (status, body, _) = call(
        &app,
        "GET",
        &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/annotatedtags/{tag_id}?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let read: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(read["message"], "Première version");
    assert_eq!(read["taggedObject"]["objectId"], commit_id);

    let (status, body, _) = call(
        &app,
        "GET",
        &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/refs?filter=tags&api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let tags: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert!(!body.contains("refs/heads/"), "{body}");
    let tag_ref = tags["value"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["name"] == "refs/tags/v1.0.0")
        .unwrap();
    assert_eq!(tag_ref["objectId"], tag_id);
    assert_eq!(tag_ref["peeledObjectId"], commit_id);

    let (status, _, _) = call(
        &app,
        "POST",
        &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/annotatedtags?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some(&format!(
            r#"{{"name":"v1.0.0","taggedObject":{{"objectId":"{commit_id}"}}}}"#
        )),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (status, _, _) = call(
        &app,
        "POST",
        &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/annotatedtags?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"name":"v9","taggedObject":{"objectId":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"}}"#),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (status, _, _) = call(
        &app,
        "POST",
        &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/annotatedtags?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"name":" ","message":"vide"}"#),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    let (status, body, _) = call(
        &app,
        "POST",
        &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/refs?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some(&format!(
            r#"[{{"name":"refs/tags/v0.9","oldObjectId":"0000000000000000000000000000000000000000","newObjectId":"{commit_id}"}}]"#
        )),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status, body, _) = call(
        &app,
        "GET",
        &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/refs?filter=tags&api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let listed: serde_json::Value = serde_json::from_str(&body).unwrap();
    let light = listed["value"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["name"] == "refs/tags/v0.9")
        .unwrap();
    assert_eq!(light["objectId"], commit_id);
    assert!(light.get("peeledObjectId").is_none() || light["peeledObjectId"].is_null());

    let (status, body, _) = call(
        &app,
        "POST",
        &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/refs?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some(&format!(
            r#"[{{"name":"refs/tags/v1.0.0","oldObjectId":"{tag_id}","newObjectId":"0000000000000000000000000000000000000000"}}]"#
        )),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let deleted: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(deleted["value"][0]["success"], true);
    let (status, body, _) = call(
        &app,
        "GET",
        &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/refs?filter=tags&api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(!body.contains("refs/tags/v1.0.0"), "{body}");
    assert!(body.contains("refs/tags/v0.9"), "{body}");
    let (status, _, _) = call(
        &app,
        "GET",
        &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/annotatedtags/{tag_id}?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _, _) = call(
        &app,
        "GET",
        &format!(        "/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/annotatedtags/bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn pull_request_commits_are_the_source_commits_missing_from_the_target() {
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
        &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/refs?filter=heads&api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let heads: serde_json::Value = serde_json::from_str(&body).unwrap();
    let main = heads["value"]
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
        &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/refs?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some(&format!(
            r#"[{{"name":"refs/heads/feature","oldObjectId":"0000000000000000000000000000000000000000","newObjectId":"{main}"}}]"#
        )),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status, body, _) = call(
        &app,
        "POST",
        &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/pushes?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some(&format!(
            r#"{{"refUpdates":[{{"name":"refs/heads/feature","oldObjectId":"{main}"}}],"commits":[{{"comment":"Pompe","changes":[{{"changeType":"add","item":{{"path":"/src/pompe.txt"}},"newContent":{{"content":"debit","contentType":"rawtext"}}}}]}}]}}"#
        )),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let push: serde_json::Value = serde_json::from_str(&body).unwrap();
    let pompe = push["commits"][0]["commitId"].as_str().unwrap().to_string();
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
        "GET",
        &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/pullrequests/{pull_id}/commits?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let listed: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(listed["count"], 1);
    assert_eq!(listed["value"][0]["comment"], "Pompe");
    assert_eq!(listed["value"][0]["commitId"], pompe);
    assert_eq!(listed["value"][0]["parents"][0], main);

    let (status, body, _) = call(
        &app,
        "POST",
        &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/pushes?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some(&format!(
            r#"{{"refUpdates":[{{"name":"refs/heads/main","oldObjectId":"{main}"}}],"commits":[{{"comment":"Bruit","changes":[{{"changeType":"add","item":{{"path":"/src/bruit.txt"}},"newContent":{{"content":"son","contentType":"rawtext"}}}}]}}]}}"#
        )),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status, body, _) = call(
        &app,
        "POST",
        &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/pushes?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some(&format!(
            r#"{{"refUpdates":[{{"name":"refs/heads/feature","oldObjectId":"{pompe}"}}],"commits":[{{"comment":"Capteur","changes":[{{"changeType":"add","item":{{"path":"/src/capteur.txt"}},"newContent":{{"content":"ok","contentType":"rawtext"}}}}]}}]}}"#
        )),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status, body, _) = call(
        &app,
        "GET",
        &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/pullrequests/{pull_id}/commits?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let listed: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(listed["count"], 2);
    assert_eq!(listed["value"][0]["comment"], "Capteur");
    assert_eq!(listed["value"][1]["comment"], "Pompe");
    assert!(!body.contains("Bruit"), "{body}");

    let (status, _, _) = call(
        &app,
        "POST",
        &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/pullrequests/{pull_id}/commits?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"comment":"interdit"}"#),
    )
    .await;
    assert_eq!(status, StatusCode::METHOD_NOT_ALLOWED);
    let (status, _, _) = call(
        &app,
        "GET",
        &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/pullrequests/999/commits?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, body, _) = call(
        &app,
        "POST",
        &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/pullrequests?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"title":"Vide","sourceRefName":"refs/heads/main","targetRefName":"refs/heads/main"}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let same: serde_json::Value = serde_json::from_str(&body).unwrap();
    let same_id = same["pullRequestId"].as_i64().unwrap();
    let (status, body, _) = call(
        &app,
        "GET",
        &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/pullrequests/{same_id}/commits?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let empty: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(empty["count"], 0);
}

#[tokio::test]
async fn git_push_and_build_completion_deliver_service_hooks() {
    let app = app();
    let auth = basic("azurz", "azurz-lab-pat");
    let (status, body, _) = call(
        &app,
        "POST",
        "/azurz/_apis/hooks/subscriptions?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"publisherId":"tfs","eventType":"git.push","consumerId":"webHooks","consumerActionId":"httpRequest","consumerInputs":{"url":"/_hooks/inbox"},"publisherInputs":{"projectId":"AzurzDemo","repository":"AzurzDemo"}}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let push_hook: serde_json::Value = serde_json::from_str(&body).unwrap();
    let push_id = push_hook["id"].as_str().unwrap();
    let (status, body, _) = call(
        &app,
        "POST",
        "/azurz/_apis/hooks/subscriptions?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"publisherId":"tfs","eventType":"git.push","consumerInputs":{"url":"/_hooks/inbox"},"publisherInputs":{"repository":"autre-repo"}}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let other: serde_json::Value = serde_json::from_str(&body).unwrap();
    let other_id = other["id"].as_str().unwrap();
    let (status, body, _) = call(
        &app,
        "POST",
        "/azurz/_apis/hooks/subscriptions?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"publisherId":"tfs","eventType":"build.complete","consumerInputs":{"url":"/_hooks/inbox"},"publisherInputs":{"projectId":"AzurzDemo"}}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let build_hook: serde_json::Value = serde_json::from_str(&body).unwrap();
    let build_hook_id = build_hook["id"].as_str().unwrap();
    let (status, body, _) = call(
        &app,
        "POST",
        "/azurz/_apis/hooks/subscriptions?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"eventType":"build.complete","consumerInputs":{"url":"/_hooks/inbox"},"publisherInputs":{"definitionName":"Autre"}}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let foreign: serde_json::Value = serde_json::from_str(&body).unwrap();
    let foreign_id = foreign["id"].as_str().unwrap();

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
        &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/refs?filter=heads&api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let heads: serde_json::Value = serde_json::from_str(&body).unwrap();
    let main = heads["value"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["name"] == "refs/heads/main")
        .unwrap()["objectId"]
        .as_str()
        .unwrap();
    let (status, body, _) = call(
        &app,
        "POST",
        &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/pushes?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some(&format!(
            r#"{{"refUpdates":[{{"name":"refs/heads/main","oldObjectId":"{main}"}}],"commits":[{{"comment":"Sonde poussée","changes":[{{"changeType":"add","item":{{"path":"/hooks.txt"}},"newContent":{{"content":"ok","contentType":"rawtext"}}}}]}}]}}"#
        )),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status, body, _) = call(
        &app,
        "GET",
        &format!("/azurz/_apis/hooks/subscriptions/{push_id}/notifications?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let notes: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(notes["count"], 1);
    assert_eq!(notes["value"][0]["eventType"], "git.push");
    assert_eq!(notes["value"][0]["status"], "succeeded");
    assert_eq!(notes["value"][0]["resource"]["commits"][0]["comment"], "Sonde poussée");
    let (status, body, _) = call(
        &app,
        "GET",
        &format!("/azurz/_apis/hooks/subscriptions/{other_id}/notifications?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let skipped: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(skipped["count"], 0);

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
    let queued: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(queued["status"], "notStarted");
    let build_id = queued["id"].as_i64().unwrap();
    let (status, body, _) = call(
        &app,
        "GET",
        &format!("/azurz/_apis/hooks/subscriptions/{build_hook_id}/notifications?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let waiting: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(waiting["count"], 0, "{body}");
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
        "GET",
        &format!("/azurz/_apis/hooks/subscriptions/{build_hook_id}/notifications?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let built: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(built["count"], 1);
    assert_eq!(built["value"][0]["eventType"], "build.complete");
    assert_eq!(built["value"][0]["resource"]["result"], "succeeded");
    let (status, _, _) = call(
        &app,
        "GET",
        &format!("/azurz/AzurzDemo/_apis/build/builds/{build_id}?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, body, _) = call(
        &app,
        "GET",
        &format!("/azurz/_apis/hooks/subscriptions/{build_hook_id}/notifications?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let once: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(once["count"], 1, "{body}");
    let (status, body, _) = call(
        &app,
        "GET",
        &format!("/azurz/_apis/hooks/subscriptions/{foreign_id}/notifications?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let ignored: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(ignored["count"], 0, "{body}");
    let (status, body, _) = call(
        &app,
        "GET",
        "/azurz/_apis/hooks/inbox?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(body.contains("git.push"), "{body}");
    assert!(body.contains("Sonde poussée"), "{body}");
    assert!(body.contains("build.complete"), "{body}");
}

#[tokio::test]
async fn pull_request_iteration_changes_are_the_net_diff() {
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
        &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/refs?filter=heads&api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let heads: serde_json::Value = serde_json::from_str(&body).unwrap();
    let main = heads["value"]
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
        &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/refs?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some(&format!(
            r#"[{{"name":"refs/heads/feature","oldObjectId":"0000000000000000000000000000000000000000","newObjectId":"{main}"}}]"#
        )),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status, body, _) = call(
        &app,
        "POST",
        &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/pushes?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some(&format!(
            r#"{{"refUpdates":[{{"name":"refs/heads/feature","oldObjectId":"{main}"}}],"commits":[{{"comment":"Pompe","changes":[{{"changeType":"add","item":{{"path":"/src/pompe.txt"}},"newContent":{{"content":"debit","contentType":"rawtext"}}}}]}}]}}"#
        )),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let push: serde_json::Value = serde_json::from_str(&body).unwrap();
    let pompe = push["commits"][0]["commitId"].as_str().unwrap().to_string();
    let (status, body, _) = call(
        &app,
        "POST",
        &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/pushes?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some(&format!(
            r#"{{"refUpdates":[{{"name":"refs/heads/feature","oldObjectId":"{pompe}"}}],"commits":[{{"comment":"Capteur","changes":[{{"changeType":"edit","item":{{"path":"/src/pompe.txt"}},"newContent":{{"content":"debit-2","contentType":"rawtext"}}}},{{"changeType":"add","item":{{"path":"/src/capteur.txt"}},"newContent":{{"content":"ok","contentType":"rawtext"}}}}]}}]}}"#
        )),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let push: serde_json::Value = serde_json::from_str(&body).unwrap();
    let capteur = push["commits"][0]["commitId"].as_str().unwrap().to_string();
    let (status, body, _) = call(
        &app,
        "POST",
        &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/pushes?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some(&format!(
            r#"{{"refUpdates":[{{"name":"refs/heads/main","oldObjectId":"{main}"}}],"commits":[{{"comment":"Bruit","changes":[{{"changeType":"add","item":{{"path":"/src/bruit.txt"}},"newContent":{{"content":"son","contentType":"rawtext"}}}}]}}]}}"#
        )),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let push: serde_json::Value = serde_json::from_str(&body).unwrap();
    let bruit = push["commits"][0]["commitId"].as_str().unwrap().to_string();
    let (status, body, _) = call(
        &app,
        "POST",
        &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/pullrequests?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"title":"Revue fichiers","sourceRefName":"refs/heads/feature","targetRefName":"refs/heads/main"}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let pull: serde_json::Value = serde_json::from_str(&body).unwrap();
    let pull_id = pull["pullRequestId"].as_i64().unwrap();
    let (status, body, _) = call(
        &app,
        "GET",
        &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/pullrequests/{pull_id}/iterations?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let listed: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(listed["count"], 1);
    assert_eq!(listed["value"][0]["description"], "Iteration 1");
    assert_eq!(listed["value"][0]["sourceRefCommit"]["commitId"], capteur);
    assert_eq!(listed["value"][0]["targetRefCommit"]["commitId"], bruit);
    assert_eq!(listed["value"][0]["commonRefCommit"]["commitId"], main);
    let (status, body, _) = call(
        &app,
        "GET",
        &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/pullrequests/{pull_id}/iterations/1?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let iteration: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(iteration["id"], 1);
    assert_eq!(iteration["sourceRefCommit"]["commitId"], capteur);
    assert_eq!(iteration["commonRefCommit"]["commitId"], main);
    assert!(iteration.get("value").is_none());
    let (status, body, _) = call(
        &app,
        "GET",
        &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/pullrequests/{pull_id}/iterations/1/changes?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let changes: serde_json::Value = serde_json::from_str(&body).unwrap();
    let entries = changes["changeEntries"].as_array().unwrap();
    assert_eq!(entries.len(), 2, "{body}");
    let kind_of = |path: &str| {
        entries
            .iter()
            .find(|entry| entry["item"]["path"] == path)
            .unwrap()["changeType"]
            .as_str()
            .unwrap()
            .to_string()
    };
    assert_eq!(kind_of("/src/pompe.txt"), "add");
    assert_eq!(kind_of("/src/capteur.txt"), "add");
    assert!(!body.contains("bruit"), "{body}");
    let (status, body, _) = call(
        &app,
        "POST",
        &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/refs?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some(&format!(
            r#"[{{"name":"refs/heads/autre","oldObjectId":"0000000000000000000000000000000000000000","newObjectId":"{main}"}}]"#
        )),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status, body, _) = call(
        &app,
        "GET",
        &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/pullrequests/{pull_id}/iterations?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let listed: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(listed["count"], 1);
    let (status, body, _) = call(
        &app,
        "POST",
        &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/pushes?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some(&format!(
            r#"{{"refUpdates":[{{"name":"refs/heads/main","oldObjectId":"{bruit}"}}],"commits":[{{"comment":"Bruit 2","changes":[{{"changeType":"add","item":{{"path":"/src/cible2.txt"}},"newContent":{{"content":"son","contentType":"rawtext"}}}}]}}]}}"#
        )),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let push: serde_json::Value = serde_json::from_str(&body).unwrap();
    let bruit2 = push["commits"][0]["commitId"].as_str().unwrap().to_string();
    let (status, body, _) = call(
        &app,
        "GET",
        &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/pullrequests/{pull_id}/iterations?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let listed: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(listed["count"], 2);
    assert_eq!(listed["value"][1]["id"], 2);
    assert_eq!(listed["value"][1]["description"], "Iteration 2");
    assert_eq!(listed["value"][1]["sourceRefCommit"]["commitId"], capteur);
    assert_eq!(listed["value"][1]["targetRefCommit"]["commitId"], bruit2);
    assert_eq!(listed["value"][0]["sourceRefCommit"]["commitId"], capteur);
    assert_eq!(listed["value"][0]["targetRefCommit"]["commitId"], bruit);
    let (status, body, _) = call(
        &app,
        "GET",
        &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/pullrequests/{pull_id}/iterations/2/changes?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(body.contains("/src/pompe.txt"), "{body}");
    assert!(body.contains("/src/capteur.txt"), "{body}");
    assert!(!body.contains("bruit"), "{body}");
    let (status, body, _) = call(
        &app,
        "POST",
        &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/pushes?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some(&format!(
            r#"{{"refUpdates":[{{"name":"refs/heads/feature","oldObjectId":"{capteur}"}}],"commits":[{{"comment":"Retrait","changes":[{{"changeType":"delete","item":{{"path":"/src/pompe.txt"}}}}]}}]}}"#
        )),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let push: serde_json::Value = serde_json::from_str(&body).unwrap();
    let retrait = push["commits"][0]["commitId"].as_str().unwrap().to_string();
    let (status, body, _) = call(
        &app,
        "GET",
        &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/pullrequests/{pull_id}/iterations/1/changes?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(body.contains("/src/pompe.txt"), "{body}");
    assert!(body.contains("/src/capteur.txt"), "{body}");
    let (status, body, _) = call(
        &app,
        "GET",
        &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/pullrequests/{pull_id}/iterations?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let listed: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(listed["count"], 3);
    assert_eq!(listed["value"][2]["sourceRefCommit"]["commitId"], retrait);
    assert_eq!(listed["value"][2]["targetRefCommit"]["commitId"], bruit2);
    let (status, body, _) = call(
        &app,
        "GET",
        &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/pullrequests/{pull_id}/iterations/3/changes?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let changes: serde_json::Value = serde_json::from_str(&body).unwrap();
    let entries = changes["changeEntries"].as_array().unwrap();
    assert_eq!(entries.len(), 1, "{body}");
    assert_eq!(entries[0]["item"]["path"], "/src/capteur.txt");
    assert_eq!(entries[0]["changeType"], "add");
    assert!(!body.contains("pompe"), "{body}");
    let (status, body, _) = call(
        &app,
        "POST",
        &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/pullrequests?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"title":"Vide","sourceRefName":"refs/heads/main","targetRefName":"refs/heads/main"}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let same: serde_json::Value = serde_json::from_str(&body).unwrap();
    let same_id = same["pullRequestId"].as_i64().unwrap();
    let (status, body, _) = call(
        &app,
        "GET",
        &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/pullrequests/{same_id}/iterations/1/changes?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let empty: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(empty["changeEntries"].as_array().unwrap().len(), 0);
    let (status, _, _) = call(
        &app,
        "GET",
        &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/pullrequests/999/iterations?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _, _) = call(
        &app,
        "GET",
        &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/pullrequests/{pull_id}/iterations/9?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _, _) = call(
        &app,
        "POST",
        &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/pullrequests/{pull_id}/iterations?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some(r#"{}"#),
    )
    .await;
    assert_eq!(status, StatusCode::METHOD_NOT_ALLOWED);
}

#[tokio::test]
async fn commit_ids_stay_distinct_when_comments_share_a_prefix() {
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
        &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/refs?filter=heads&api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let heads: serde_json::Value = serde_json::from_str(&body).unwrap();
    let mut tip = heads["value"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["name"] == "refs/heads/main")
        .unwrap()["objectId"]
        .as_str()
        .unwrap()
        .to_string();
    let root = tip.clone();
    let mut ids = Vec::new();
    for (comment, path) in [
        ("Sonde", "/src/sonde.txt"),
        ("Sonde poussée", "/src/sonde-poussee.txt"),
        ("sonde lab", "/src/sonde-lab.txt"),
    ] {
        let (status, body, _) = call(
            &app,
            "POST",
            &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/pushes?api-version=7.1"),
            Some(&auth),
            None,
            "application/json",
            Some(&format!(
                r#"{{"refUpdates":[{{"name":"refs/heads/main","oldObjectId":"{tip}"}}],"commits":[{{"comment":"{comment}","changes":[{{"changeType":"add","item":{{"path":"{path}"}},"newContent":{{"content":"ok","contentType":"rawtext"}}}}]}}]}}"#
            )),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{body}");
        let push: serde_json::Value = serde_json::from_str(&body).unwrap();
        let commit_id = push["commits"][0]["commitId"].as_str().unwrap().to_string();
        assert_eq!(commit_id.len(), 40, "{commit_id}");
        assert!(commit_id.chars().all(|ch| ch.is_ascii_hexdigit()));
        assert_ne!(commit_id, tip);
        ids.push(commit_id.clone());
        tip = commit_id;
    }
    assert_ne!(ids[0], ids[1]);
    assert_ne!(ids[1], ids[2]);
    assert_ne!(ids[0], ids[2]);
    let parents = [root.as_str(), ids[0].as_str(), ids[1].as_str()];
    let comments = ["Sonde", "Sonde poussée", "sonde lab"];
    for index in 0..3 {
        let (status, body, _) = call(
            &app,
            "GET",
            &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/commits/{}?api-version=7.1", ids[index]),
            Some(&auth),
            None,
            "application/json",
            None,
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{body}");
        let commit: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(commit["comment"], comments[index]);
        assert_eq!(commit["parents"][0], parents[index]);
    }
    let mut tag_ids = Vec::new();
    for name in ["v1.0.0", "v1.0.1"] {
        let (status, body, _) = call(
            &app,
            "POST",
            &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/annotatedtags?api-version=7.1"),
            Some(&auth),
            None,
            "application/json",
            Some(&format!(
                r#"{{"name":"{name}","message":"{name}","taggedObject":{{"objectId":"{}"}}}}"#,
                ids[2]
            )),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{body}");
        let tag: serde_json::Value = serde_json::from_str(&body).unwrap();
        tag_ids.push(tag["objectId"].as_str().unwrap().to_string());
    }
    assert_ne!(tag_ids[0], tag_ids[1]);
    assert_ne!(tag_ids[0], ids[2]);
    let (status, body, _) = call(
        &app,
        "GET",
        &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/annotatedtags/{}?api-version=7.1", tag_ids[0]),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let first_tag: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(first_tag["name"], "v1.0.0");
    assert_eq!(first_tag["taggedObject"]["objectId"], ids[2]);
}

#[tokio::test]
async fn pull_request_lifecycle_delivers_service_hooks() {
    let app = app();
    let auth = basic("azurz", "azurz-lab-pat");
    let subscribe = |event: &str, repository: &str| {
        format!(
            r#"{{"publisherId":"tfs","eventType":"{event}","consumerId":"webHooks","consumerActionId":"httpRequest","consumerInputs":{{"url":"/_hooks/inbox"}},"publisherInputs":{{"projectId":"AzurzDemo","repository":"{repository}"}}}}"#
        )
    };
    let mut ids = std::collections::BTreeMap::new();
    for (event, repository) in [
        ("git.pullrequest.created", "AzurzDemo"),
        ("git.pullrequest.created", "autre-repo"),
        ("git.pullrequest.updated", "AzurzDemo"),
        ("git.pullrequest.merged", "AzurzDemo"),
        ("workitem.created", "AzurzDemo"),
    ] {
        let (status, body, _) = call(
            &app,
            "POST",
            "/azurz/_apis/hooks/subscriptions?api-version=7.1",
            Some(&auth),
            None,
            "application/json",
            Some(&subscribe(event, repository)),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{body}");
        let hook: serde_json::Value = serde_json::from_str(&body).unwrap();
        ids.insert(format!("{event}:{repository}"), hook["id"].as_str().unwrap().to_string());
    }
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
        Some(r#"{"title":"Revue sonde","sourceRefName":"refs/heads/feature","targetRefName":"refs/heads/main"}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let pull: serde_json::Value = serde_json::from_str(&body).unwrap();
    let pull_id = pull["pullRequestId"].as_i64().unwrap();
    let notes = |app: &axum::Router, id: &str| {
        let app = app.clone();
        let id = id.to_string();
        let auth = auth.clone();
        async move {
            let (status, body, _) = call(
                &app,
                "GET",
                &format!("/azurz/_apis/hooks/subscriptions/{id}/notifications?api-version=7.1"),
                Some(&auth),
                None,
                "application/json",
                None,
            )
            .await;
            assert_eq!(status, StatusCode::OK, "{body}");
            serde_json::from_str::<serde_json::Value>(&body).unwrap()
        }
    };
    let created = notes(&app, &ids["git.pullrequest.created:AzurzDemo"]).await;
    assert_eq!(created["count"], 1);
    assert_eq!(created["value"][0]["eventType"], "git.pullrequest.created");
    assert_eq!(created["value"][0]["resource"]["title"], "Revue sonde");
    let foreign = notes(&app, &ids["git.pullrequest.created:autre-repo"]).await;
    assert_eq!(foreign["count"], 0);
    let updated = notes(&app, &ids["git.pullrequest.updated:AzurzDemo"]).await;
    assert_eq!(updated["count"], 0);
    let merged = notes(&app, &ids["git.pullrequest.merged:AzurzDemo"]).await;
    assert_eq!(merged["count"], 0);
    let (status, body, _) = call(
        &app,
        "PATCH",
        &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/pullrequests/{pull_id}?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"title":"Revue sonde bis"}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let updated = notes(&app, &ids["git.pullrequest.updated:AzurzDemo"]).await;
    assert_eq!(updated["count"], 1);
    assert_eq!(updated["value"][0]["resource"]["title"], "Revue sonde bis");
    let (status, body, _) = call(
        &app,
        "POST",
        &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/pullrequests/{pull_id}/reviewers?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"displayName":"Nadia","vote":10}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let updated = notes(&app, &ids["git.pullrequest.updated:AzurzDemo"]).await;
    assert_eq!(updated["count"], 2);
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
    let merged = notes(&app, &ids["git.pullrequest.merged:AzurzDemo"]).await;
    assert_eq!(merged["count"], 1);
    assert_eq!(merged["value"][0]["eventType"], "git.pullrequest.merged");
    assert_eq!(merged["value"][0]["resource"]["status"], "completed");
    assert!(merged["value"][0]["resource"]["lastMergeCommit"]["commitId"].as_str().unwrap().len() == 40);
    let updated = notes(&app, &ids["git.pullrequest.updated:AzurzDemo"]).await;
    assert_eq!(updated["count"], 2);
    let created = notes(&app, &ids["git.pullrequest.created:AzurzDemo"]).await;
    assert_eq!(created["count"], 1);
    let (status, _, _) = call(
        &app,
        "PATCH",
        &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/pullrequests/{pull_id}?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"title":"Encore"}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let merged = notes(&app, &ids["git.pullrequest.merged:AzurzDemo"]).await;
    assert_eq!(merged["count"], 1);
    let updated = notes(&app, &ids["git.pullrequest.updated:AzurzDemo"]).await;
    assert_eq!(updated["count"], 2);
    let silent = notes(&app, &ids["workitem.created:AzurzDemo"]).await;
    assert_eq!(silent["count"], 0);
    let (status, body, _) = call(
        &app,
        "GET",
        "/azurz/_apis/hooks/inbox?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(body.contains("git.pullrequest.created"), "{body}");
    assert!(body.contains("Revue sonde"), "{body}");
    assert!(body.contains("git.pullrequest.updated"), "{body}");
    assert!(body.contains("Revue sonde bis"), "{body}");
    assert!(body.contains("git.pullrequest.merged"), "{body}");
}

#[tokio::test]
async fn squash_merge_records_the_net_diff_and_can_delete_the_source() {
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
        &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/refs?filter=heads&api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let heads: serde_json::Value = serde_json::from_str(&body).unwrap();
    let main = heads["value"]
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
        &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/refs?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some(&format!(
            r#"[{{"name":"refs/heads/squash-src","oldObjectId":"0000000000000000000000000000000000000000","newObjectId":"{main}"}}]"#
        )),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status, body, _) = call(
        &app,
        "POST",
        &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/pushes?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some(&format!(
            r#"{{"refUpdates":[{{"name":"refs/heads/squash-src","oldObjectId":"{main}"}}],"commits":[{{"comment":"S01 Pompe","changes":[{{"changeType":"add","item":{{"path":"/src/squash-pompe.txt"}},"newContent":{{"content":"debit","contentType":"rawtext"}}}}]}}]}}"#
        )),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let push: serde_json::Value = serde_json::from_str(&body).unwrap();
    let first = push["commits"][0]["commitId"].as_str().unwrap();
    let (status, body, _) = call(
        &app,
        "POST",
        &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/pushes?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some(&format!(
            r#"{{"refUpdates":[{{"name":"refs/heads/squash-src","oldObjectId":"{first}"}}],"commits":[{{"comment":"S02 Pompe","changes":[{{"changeType":"edit","item":{{"path":"/src/squash-pompe.txt"}},"newContent":{{"content":"debit-2","contentType":"rawtext"}}}}]}}]}}"#
        )),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let push: serde_json::Value = serde_json::from_str(&body).unwrap();
    let source_tip = push["commits"][0]["commitId"].as_str().unwrap().to_string();
    let (status, body, _) = call(
        &app,
        "POST",
        &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/pullrequests?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"title":"Revue squash","sourceRefName":"refs/heads/squash-src","targetRefName":"refs/heads/main"}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let pull: serde_json::Value = serde_json::from_str(&body).unwrap();
    let pull_id = pull["pullRequestId"].as_i64().unwrap();
    let (status, _, _) = call(
        &app,
        "PATCH",
        &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/pullrequests/{pull_id}?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"status":"completed","completionOptions":{"mergeStrategy":"octopus"}}"#),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (status, body, _) = call(
        &app,
        "GET",
        &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/pullrequests/{pull_id}?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let still: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(still["status"], "active");
    let (status, body, _) = call(
        &app,
        "PATCH",
        &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/pullrequests/{pull_id}?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"status":"completed","completionOptions":{"mergeStrategy":"squash","mergeCommitMessage":"Squash pompe","deleteSourceBranch":true}}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let done: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(done["status"], "completed");
    assert_eq!(done["completionOptions"]["mergeStrategy"], "squash");
    assert_eq!(done["completionOptions"]["deleteSourceBranch"], true);
    assert_eq!(done["lastMergeSourceCommit"]["commitId"], source_tip);
    assert_eq!(done["lastMergeTargetCommit"]["commitId"], main);
    let merge = done["lastMergeCommit"]["commitId"].as_str().unwrap();
    let (status, body, _) = call(
        &app,
        "GET",
        &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/commits/{merge}?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let commit: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(commit["comment"], "Squash pompe");
    assert_eq!(commit["parents"].as_array().unwrap().len(), 1);
    assert_eq!(commit["parents"][0], main);
    let (status, body, _) = call(
        &app,
        "GET",
        &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/commits/{merge}/changes?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let changes: serde_json::Value = serde_json::from_str(&body).unwrap();
    let entries = changes["changes"].as_array().unwrap();
    assert_eq!(entries.len(), 1, "{body}");
    assert_eq!(entries[0]["item"]["path"], "/src/squash-pompe.txt");
    assert_eq!(entries[0]["changeType"], "add");
    assert_eq!(entries[0]["newContent"]["content"], "debit-2");
    let (status, body, _) = call(
        &app,
        "GET",
        &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/refs?filter=heads&api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let refs: serde_json::Value = serde_json::from_str(&body).unwrap();
    let names: Vec<&str> = refs["value"].as_array().unwrap().iter().filter_map(|item| item["name"].as_str()).collect();
    assert!(!names.contains(&"refs/heads/squash-src"), "{body}");
    let main_ref = refs["value"].as_array().unwrap().iter().find(|item| item["name"] == "refs/heads/main").unwrap();
    assert_eq!(main_ref["objectId"], merge);
    let (status, body, _) = call(
        &app,
        "GET",
        &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/pullrequests/{pull_id}/commits?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(body.contains("S02 Pompe"), "{body}");
    assert!(body.contains("S01 Pompe"), "{body}");
    let (status, body, _) = call(
        &app,
        "POST",
        &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/refs?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some(&format!(
            r#"[{{"name":"refs/heads/fusion-src","oldObjectId":"0000000000000000000000000000000000000000","newObjectId":"{merge}"}}]"#
        )),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status, body, _) = call(
        &app,
        "POST",
        &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/pullrequests?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"title":"Revue fusion","sourceRefName":"refs/heads/fusion-src","targetRefName":"refs/heads/main"}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let other: serde_json::Value = serde_json::from_str(&body).unwrap();
    let other_id = other["pullRequestId"].as_i64().unwrap();
    let (status, body, _) = call(
        &app,
        "PATCH",
        &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/pullrequests/{other_id}?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"status":"completed","completionOptions":{"mergeStrategy":"noFastForward","deleteSourceBranch":false}}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let fused: serde_json::Value = serde_json::from_str(&body).unwrap();
    let fused_id = fused["lastMergeCommit"]["commitId"].as_str().unwrap();
    let (status, body, _) = call(
        &app,
        "GET",
        &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/commits/{fused_id}?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(body.contains("Fusion de refs/heads/fusion-src dans refs/heads/main"), "{body}");
    let (status, body, _) = call(
        &app,
        "GET",
        &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/refs?filter=heads&api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(body.contains("refs/heads/fusion-src"), "{body}");
}

#[tokio::test]
async fn rebase_replays_source_commits_onto_the_target() {
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
        &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/refs?filter=heads&api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let heads: serde_json::Value = serde_json::from_str(&body).unwrap();
    let main = heads["value"]
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
        &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/refs?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some(&format!(
            r#"[{{"name":"refs/heads/rebase-src","oldObjectId":"0000000000000000000000000000000000000000","newObjectId":"{main}"}}]"#
        )),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status, body, _) = call(
        &app,
        "POST",
        &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/pushes?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some(&format!(
            r#"{{"refUpdates":[{{"name":"refs/heads/rebase-src","oldObjectId":"{main}"}}],"commits":[{{"comment":"R01 Pompe","changes":[{{"changeType":"add","item":{{"path":"/src/rebase-pompe.txt"}},"newContent":{{"content":"debit","contentType":"rawtext"}}}}]}}]}}"#
        )),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let push: serde_json::Value = serde_json::from_str(&body).unwrap();
    let first = push["commits"][0]["commitId"].as_str().unwrap().to_string();
    let (status, body, _) = call(
        &app,
        "POST",
        &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/pushes?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some(&format!(
            r#"{{"refUpdates":[{{"name":"refs/heads/rebase-src","oldObjectId":"{first}"}}],"commits":[{{"comment":"R02 Pompe","changes":[{{"changeType":"edit","item":{{"path":"/src/rebase-pompe.txt"}},"newContent":{{"content":"debit-2","contentType":"rawtext"}}}}]}}]}}"#
        )),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let push: serde_json::Value = serde_json::from_str(&body).unwrap();
    let second = push["commits"][0]["commitId"].as_str().unwrap().to_string();
    let (status, body, _) = call(
        &app,
        "POST",
        &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/pushes?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some(&format!(
            r#"{{"refUpdates":[{{"name":"refs/heads/main","oldObjectId":"{main}"}}],"commits":[{{"comment":"R03 Cible","changes":[{{"changeType":"add","item":{{"path":"/src/rebase-cible.txt"}},"newContent":{{"content":"base","contentType":"rawtext"}}}}]}}]}}"#
        )),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let push: serde_json::Value = serde_json::from_str(&body).unwrap();
    let cible = push["commits"][0]["commitId"].as_str().unwrap().to_string();
    let (status, body, _) = call(
        &app,
        "POST",
        &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/pullrequests?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"title":"Revue rebase","sourceRefName":"refs/heads/rebase-src","targetRefName":"refs/heads/main"}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let pull: serde_json::Value = serde_json::from_str(&body).unwrap();
    let pull_id = pull["pullRequestId"].as_i64().unwrap();
    let (status, body, _) = call(
        &app,
        "PATCH",
        &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/pullrequests/{pull_id}?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"status":"completed","completionOptions":{"mergeStrategy":"rebase","deleteSourceBranch":false}}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let done: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(done["status"], "completed");
    assert_eq!(done["completionOptions"]["mergeStrategy"], "rebase");
    assert_eq!(done["lastMergeTargetCommit"]["commitId"], cible);
    let tip = done["lastMergeCommit"]["commitId"].as_str().unwrap().to_string();
    assert_ne!(tip, first);
    assert_ne!(tip, second);
    assert_eq!(done["lastMergeSourceCommit"]["commitId"], tip);
    let (status, body, _) = call(
        &app,
        "GET",
        &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/commits/{tip}?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let replayed: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(replayed["comment"], "R02 Pompe");
    let parent = replayed["parents"][0].as_str().unwrap().to_string();
    assert_ne!(parent, cible);
    let (status, body, _) = call(
        &app,
        "GET",
        &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/commits/{parent}?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let older: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(older["comment"], "R01 Pompe");
    assert_eq!(older["parents"][0], cible);
    let (status, body, _) = call(
        &app,
        "GET",
        &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/commits/{tip}/changes?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let changes: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(changes["changes"][0]["changeType"], "edit");
    assert_eq!(changes["changes"][0]["newContent"]["content"], "debit-2");
    let (status, body, _) = call(
        &app,
        "GET",
        &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/commits/{parent}/changes?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(body.contains("debit") && body.contains("add"), "{body}");
    let (status, body, _) = call(
        &app,
        "GET",
        &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/refs?filter=heads&api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let refs: serde_json::Value = serde_json::from_str(&body).unwrap();
    let object_of = |name: &str| {
        refs["value"]
            .as_array()
            .unwrap()
            .iter()
            .find(|item| item["name"] == name)
            .unwrap()["objectId"]
            .as_str()
            .unwrap()
            .to_string()
    };
    assert_eq!(object_of("refs/heads/main"), tip);
    assert_eq!(object_of("refs/heads/rebase-src"), tip);
    let (status, body, _) = call(
        &app,
        "GET",
        &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/pullrequests/{pull_id}/commits?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let listed: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(listed["value"][0]["comment"], "R02 Pompe");
    assert_eq!(listed["value"][1]["comment"], "R01 Pompe");
    assert_eq!(listed["value"][0]["commitId"], tip);
    assert!(!body.contains(&first), "{body}");
    let (status, body, _) = call(
        &app,
        "POST",
        &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/refs?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some(&format!(
            r#"[{{"name":"refs/heads/semi-src","oldObjectId":"0000000000000000000000000000000000000000","newObjectId":"{tip}"}}]"#
        )),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status, body, _) = call(
        &app,
        "POST",
        &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/pushes?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some(&format!(
            r#"{{"refUpdates":[{{"name":"refs/heads/semi-src","oldObjectId":"{tip}"}}],"commits":[{{"comment":"R04 Semi","changes":[{{"changeType":"add","item":{{"path":"/src/rebase-semi.txt"}},"newContent":{{"content":"semi","contentType":"rawtext"}}}}]}}]}}"#
        )),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status, body, _) = call(
        &app,
        "POST",
        &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/pullrequests?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"title":"Revue semi","sourceRefName":"refs/heads/semi-src","targetRefName":"refs/heads/main"}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let semi: serde_json::Value = serde_json::from_str(&body).unwrap();
    let semi_id = semi["pullRequestId"].as_i64().unwrap();
    let (status, body, _) = call(
        &app,
        "PATCH",
        &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/pullrequests/{semi_id}?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"status":"completed","completionOptions":{"mergeStrategy":"rebaseMerge","mergeCommitMessage":"Semi pompe","deleteSourceBranch":true}}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let semi_done: serde_json::Value = serde_json::from_str(&body).unwrap();
    let merge = semi_done["lastMergeCommit"]["commitId"].as_str().unwrap();
    let rebased = semi_done["lastMergeSourceCommit"]["commitId"].as_str().unwrap().to_string();
    assert_ne!(merge, rebased);
    let (status, body, _) = call(
        &app,
        "GET",
        &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/commits/{merge}?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let merge_commit: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(merge_commit["comment"], "Semi pompe");
    assert_eq!(merge_commit["parents"].as_array().unwrap().len(), 2);
    assert_eq!(merge_commit["parents"][0], tip);
    assert_eq!(merge_commit["parents"][1], rebased);
    let (status, body, _) = call(
        &app,
        "GET",
        &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/refs?filter=heads&api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let refs: serde_json::Value = serde_json::from_str(&body).unwrap();
    let names: Vec<&str> = refs["value"].as_array().unwrap().iter().filter_map(|item| item["name"].as_str()).collect();
    assert!(!names.contains(&"refs/heads/semi-src"), "{body}");
    let main_ref = refs["value"].as_array().unwrap().iter().find(|item| item["name"] == "refs/heads/main").unwrap();
    assert_eq!(main_ref["objectId"], merge);
    let (status, body, _) = call(
        &app,
        "GET",
        &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/pullrequests/{semi_id}/commits?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(body.contains("R04 Semi"), "{body}");
}

#[tokio::test]
async fn cherry_pick_replays_commits_and_revert_undoes_them() {
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
        &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/refs?filter=heads&api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let heads: serde_json::Value = serde_json::from_str(&body).unwrap();
    let main = heads["value"]
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
        &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/refs?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some(&format!(
            r#"[{{"name":"refs/heads/cherry-src","oldObjectId":"0000000000000000000000000000000000000000","newObjectId":"{main}"}}]"#
        )),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status, body, _) = call(
        &app,
        "POST",
        &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/pushes?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some(&format!(
            r#"{{"refUpdates":[{{"name":"refs/heads/cherry-src","oldObjectId":"{main}"}}],"commits":[{{"comment":"C01 Pompe","changes":[{{"changeType":"add","item":{{"path":"/src/cherry-pompe.txt"}},"newContent":{{"content":"debit","contentType":"rawtext"}}}}]}}]}}"#
        )),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let push: serde_json::Value = serde_json::from_str(&body).unwrap();
    let first = push["commits"][0]["commitId"].as_str().unwrap().to_string();
    let (status, body, _) = call(
        &app,
        "POST",
        &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/pushes?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some(&format!(
            r#"{{"refUpdates":[{{"name":"refs/heads/cherry-src","oldObjectId":"{first}"}}],"commits":[{{"comment":"C02 Pompe","changes":[{{"changeType":"edit","item":{{"path":"/src/cherry-pompe.txt"}},"newContent":{{"content":"debit-2","contentType":"rawtext"}}}}]}}]}}"#
        )),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let push: serde_json::Value = serde_json::from_str(&body).unwrap();
    let second = push["commits"][0]["commitId"].as_str().unwrap().to_string();
    let (status, body, _) = call(
        &app,
        "POST",
        &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/cherryPicks?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"generatedRefName":"heads/cherry-out","ontoRefName":"refs/heads/main","source":{"commitList":[{"commitId":"0123456789abcdef0123456789abcdef01234567"}]}}"#),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert!(body.contains("référence absente"), "{body}");
    let (status, body, _) = call(
        &app,
        "POST",
        &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/cherryPicks?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"generatedRefName":"refs/heads/missing-out","ontoRefName":"refs/heads/main","source":{"commitList":[{"commitId":"0123456789abcdef0123456789abcdef01234567"}]}}"#),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert!(body.contains("commit introuvable"), "{body}");
    let (status, body, _) = call(
        &app,
        "POST",
        &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/cherryPicks?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some(&format!(
            r#"{{"generatedRefName":"refs/heads/cherry-out","ontoRefName":"refs/heads/main","source":{{"commitList":[{{"commitId":"{first}"}},{{"commitId":"{second}"}}]}}}}"#
        )),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let operation: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(operation["status"], "completed");
    assert_eq!(operation["cherryPickId"], 1);
    assert_eq!(operation["detailedStatus"]["status"], "succeeded");
    let tip = operation["detailedStatus"]["currentCommitId"].as_str().unwrap().to_string();
    assert_ne!(tip, second);
    let (status, body, _) = call(
        &app,
        "GET",
        &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/cherryPicks/1?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(body.contains(&tip), "{body}");
    let (status, body, _) = call(
        &app,
        "GET",
        &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/commits/{tip}?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let replayed: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(replayed["comment"], "C02 Pompe");
    let replayed_parent = replayed["parents"][0].as_str().unwrap().to_string();
    assert_ne!(replayed_parent, main);
    assert_ne!(replayed_parent, first);
    let (status, body, _) = call(
        &app,
        "GET",
        &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/commits/{replayed_parent}?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let parent_commit: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(parent_commit["comment"], "C01 Pompe");
    assert_eq!(parent_commit["parents"][0], main);
    let (status, body, _) = call(
        &app,
        "GET",
        &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/commits/{tip}/changes?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(body.contains("debit-2"), "{body}");
    assert!(body.contains("edit"), "{body}");
    let (status, body, _) = call(
        &app,
        "GET",
        &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/items?path=/src/cherry-pompe.txt&versionDescriptor.version={tip}&versionDescriptor.versionType=commit&api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(body.contains("debit-2"), "{body}");
    let (status, body, _) = call(
        &app,
        "GET",
        &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/items?path=/src/cherry-pompe.txt&versionDescriptor.version={main}&versionDescriptor.versionType=commit&api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    let (status, body, _) = call(
        &app,
        "GET",
        &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/refs?filter=heads&api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let refs: serde_json::Value = serde_json::from_str(&body).unwrap();
    let cherry = refs["value"].as_array().unwrap().iter().find(|item| item["name"] == "refs/heads/cherry-out").unwrap();
    assert_eq!(cherry["objectId"], tip);
    let main_ref = refs["value"].as_array().unwrap().iter().find(|item| item["name"] == "refs/heads/main").unwrap();
    assert_eq!(main_ref["objectId"], main);
    let (status, body, _) = call(
        &app,
        "POST",
        &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/cherryPicks?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some(&format!(
            r#"{{"generatedRefName":"refs/heads/cherry-out","ontoRefName":"refs/heads/main","source":{{"commitList":[{{"commitId":"{first}"}}]}}}}"#
        )),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert!(body.contains("référence existante"), "{body}");
    let (status, body, _) = call(
        &app,
        "GET",
        &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/cherryPicks/9?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    let (status, body, _) = call(
        &app,
        "POST",
        &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/reverts?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some(&format!(
            r#"{{"generatedRefName":"refs/heads/revert-out","ontoRefName":"refs/heads/cherry-out","source":{{"commitList":[{{"commitId":"{tip}"}},{{"commitId":"{replayed_parent}"}}]}}}}"#
        )),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let revert: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(revert["status"], "completed");
    assert_eq!(revert["revertId"], 1);
    let revert_tip = revert["detailedStatus"]["currentCommitId"].as_str().unwrap().to_string();
    let (status, body, _) = call(
        &app,
        "GET",
        &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/commits/{revert_tip}?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let reverted: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(reverted["comment"], "Revert \"C01 Pompe\"");
    assert_ne!(reverted["parents"][0], tip);
    let (status, body, _) = call(
        &app,
        "GET",
        &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/commits/{revert_tip}/changes?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(body.contains("delete"), "{body}");
    let (status, body, _) = call(
        &app,
        "GET",
        &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/items?path=/src/cherry-pompe.txt&versionDescriptor.version={revert_tip}&versionDescriptor.versionType=commit&api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    let (status, body, _) = call(
        &app,
        "GET",
        &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/items?path=/src/cherry-pompe.txt&versionDescriptor.version={tip}&versionDescriptor.versionType=commit&api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(body.contains("debit-2"), "{body}");
    let (status, body, _) = call(
        &app,
        "GET",
        &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/pushes?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(body.contains("refs/heads/cherry-out"), "{body}");
    assert!(body.contains("refs/heads/revert-out"), "{body}");
}

#[tokio::test]
async fn pull_request_labels_are_shared_and_can_be_removed() {
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
        Some(r#"{"title":"Revue etiquette","sourceRefName":"refs/heads/etiquette","targetRefName":"refs/heads/main"}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let pull: serde_json::Value = serde_json::from_str(&body).unwrap();
    let pull_id = pull["pullRequestId"].as_i64().unwrap();
    let (status, body, _) = call(
        &app,
        "GET",
        &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/pullrequests/{pull_id}/labels?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let labels: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(labels["count"], 0);
    let (status, body, _) = call(
        &app,
        "POST",
        &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/pullrequests/{pull_id}/labels?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"name":" "}"#),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert!(body.contains("nom d"), "{body}");
    let (status, body, _) = call(
        &app,
        "POST",
        &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/pullrequests/{pull_id}/labels?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"name":"Bug"}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let bug: serde_json::Value = serde_json::from_str(&body).unwrap();
    let bug_id = bug["id"].as_str().unwrap().to_string();
    assert_eq!(bug_id.len(), 40);
    assert_eq!(bug["name"], "Bug");
    assert_eq!(bug["active"], true);
    let (status, body, _) = call(
        &app,
        "POST",
        &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/pullrequests/{pull_id}/labels?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"name":"bug"}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let again: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(again["id"], bug_id);
    assert_eq!(again["name"], "Bug");
    let (status, body, _) = call(
        &app,
        "POST",
        &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/pullrequests/{pull_id}/labels?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"name":"Revue"}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status, body, _) = call(
        &app,
        "GET",
        &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/pullrequests/{pull_id}/labels?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let labels: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(labels["count"], 2);
    let (status, body, _) = call(
        &app,
        "GET",
        &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/labels?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(body.contains("Bug"), "{body}");
    assert!(body.contains("Revue"), "{body}");
    let (status, body, _) = call(
        &app,
        "POST",
        &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/pullrequests?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"title":"Seconde revue","sourceRefName":"refs/heads/autre","targetRefName":"refs/heads/main"}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let second: serde_json::Value = serde_json::from_str(&body).unwrap();
    let second_id = second["pullRequestId"].as_i64().unwrap();
    let (status, body, _) = call(
        &app,
        "POST",
        &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/pullrequests/{second_id}/labels?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"name":"Bug"}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let shared: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(shared["id"], bug_id);
    let (status, body, _) = call(
        &app,
        "DELETE",
        &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/pullrequests/{pull_id}/labels/{bug_id}?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT, "{body}");
    let (status, body, _) = call(
        &app,
        "GET",
        &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/pullrequests/{pull_id}/labels?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let labels: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(labels["count"], 1);
    assert_eq!(labels["value"][0]["name"], "Revue");
    let (status, body, _) = call(
        &app,
        "GET",
        &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/pullrequests/{second_id}/labels?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(body.contains(&bug_id), "{body}");
    let (status, body, _) = call(
        &app,
        "GET",
        &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/labels?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(body.contains("Bug"), "{body}");
    let (status, body, _) = call(
        &app,
        "DELETE",
        &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/pullrequests/{pull_id}/labels/absente?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    let (status, body, _) = call(
        &app,
        "GET",
        &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/pullrequests/9/labels?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
}

#[tokio::test]
async fn work_item_updates_record_field_and_relation_changes() {
    let app = app();
    let auth = basic("azurz", "azurz-lab-pat");
    let (status, body, _) = call(
        &app,
        "POST",
        "/azurz/AzurzDemo/_apis/wit/workitems/$User%20Story?api-version=7.1",
        Some(&auth),
        None,
        "application/json-patch+json",
        Some(r#"[{"op":"add","path":"/fields/System.Title","value":"Pompe historique"}]"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let created: serde_json::Value = serde_json::from_str(&body).unwrap();
    let id = created["id"].as_i64().unwrap();
    let (status, body, _) = call(
        &app,
        "GET",
        &format!("/azurz/AzurzDemo/_apis/wit/workItems/{id}/updates?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let updates: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(updates["count"], 1);
    assert_eq!(updates["value"][0]["rev"], 1);
    assert_eq!(updates["value"][0]["workItemId"], id);
    assert_eq!(updates["value"][0]["fields"]["System.Title"]["newValue"], "Pompe historique");
    assert!(updates["value"][0]["fields"]["System.Title"].get("oldValue").is_none());
    assert!(updates["value"][0].get("relations").is_none());
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
    let (status, body, _) = call(
        &app,
        "GET",
        &format!("/azurz/AzurzDemo/_apis/wit/workItems/{id}/updates/2?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let update: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(update["rev"], 2);
    assert_eq!(update["fields"]["System.State"]["oldValue"], "New");
    assert_eq!(update["fields"]["System.State"]["newValue"], "Active");
    assert!(update["fields"].get("System.Title").is_none(), "{body}");
    let artifact = "vstfs:///Git/PullRequestId/azurz%2Fdemo%2F1";
    let (status, body, _) = call(
        &app,
        "PATCH",
        &format!("/azurz/AzurzDemo/_apis/wit/workitems/{id}?api-version=7.1"),
        Some(&auth),
        None,
        "application/json-patch+json",
        Some(&format!(
            r#"[{{"op":"add","path":"/relations/-","value":{{"rel":"ArtifactLink","url":"{artifact}"}}}}]"#
        )),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status, body, _) = call(
        &app,
        "GET",
        &format!("/azurz/AzurzDemo/_apis/wit/workItems/{id}/updates/3?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let update: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(update["relations"]["added"][0]["rel"], "ArtifactLink");
    assert_eq!(update["relations"]["added"][0]["url"], artifact);
    assert!(update["relations"]["removed"].as_array().unwrap().is_empty());
    let (status, body, _) = call(
        &app,
        "PATCH",
        &format!("/azurz/AzurzDemo/_apis/wit/workitems/{id}?api-version=7.1"),
        Some(&auth),
        None,
        "application/json-patch+json",
        Some(r#"[{"op":"remove","path":"/relations/0"}]"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status, body, _) = call(
        &app,
        "GET",
        &format!("/azurz/AzurzDemo/_apis/wit/workItems/{id}/updates/4?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let update: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(update["relations"]["removed"][0]["url"], artifact);
    assert!(update["relations"]["added"].as_array().unwrap().is_empty());
    let (status, body, _) = call(
        &app,
        "GET",
        &format!("/azurz/AzurzDemo/_apis/wit/workItems/{id}/revisions?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let revisions: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(revisions["count"], 4);
    assert_eq!(revisions["value"][3]["rev"], 4);
    assert_eq!(revisions["value"][3]["fields"]["System.State"], "Active");
    assert!(revisions["value"][3].get("revisedBy").is_none());
    let (status, body, _) = call(
        &app,
        "GET",
        &format!("/azurz/AzurzDemo/_apis/wit/workItems/{id}/updates/9?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    let (status, body, _) = call(
        &app,
        "GET",
        "/azurz/AzurzDemo/_apis/wit/workItems/99/updates?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
}

#[tokio::test]
async fn environment_checks_require_a_known_type_and_can_be_removed() {
    let app = app();
    let auth = basic("azurz", "azurz-lab-pat");
    let (status, body, _) = call(
        &app,
        "POST",
        "/azurz/AzurzDemo/_apis/distributedtask/environments?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"name":"Production","description":"Ligne A"}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let environment: serde_json::Value = serde_json::from_str(&body).unwrap();
    let env_id = environment["id"].as_i64().unwrap().to_string();
    let (status, body, _) = call(
        &app,
        "POST",
        "/azurz/AzurzDemo/_apis/pipelines/checks/configurations?api-version=7.1-preview.1",
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"type":{"id":"00000000-0000-0000-0000-000000000000"},"resource":{"type":"environment","id":"1"}}"#),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert!(body.contains("type de contr"));
    let (status, body, _) = call(
        &app,
        "POST",
        "/azurz/AzurzDemo/_apis/pipelines/checks/configurations?api-version=7.1-preview.1",
        Some(&auth),
        None,
        "application/json",
        Some(&format!(
            r#"{{"type":{{"id":"8c6f20a7-a545-4486-9777-f762fafe0d4d"}},"resource":{{"type":"environment","id":"{env_id}"}},"settings":{{"approvers":[]}}}}"#
        )),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert!(body.contains("approbateur"));
    let (status, body, _) = call(
        &app,
        "POST",
        "/azurz/AzurzDemo/_apis/pipelines/checks/configurations?api-version=7.1-preview.1",
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"type":{"id":"8c6f20a7-a545-4486-9777-f762fafe0d4d"},"resource":{"type":"environment","id":"9"},"settings":{"approvers":[{"displayName":"azurz"}]}}"#),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert!(body.contains("environnement"));
    let (status, body, _) = call(
        &app,
        "POST",
        "/azurz/AzurzDemo/_apis/pipelines/checks/configurations?api-version=7.1-preview.1",
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"type":{"id":"8c6f20a7-a545-4486-9777-f762fafe0d4d"},"resource":{"type":"widget","id":"1"}}"#),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert!(body.contains("type de ressource"));
    let approval = format!(
        r#"{{"type":{{"id":"8C6F20A7-A545-4486-9777-F762FAFE0D4D","name":"Approval"}},"settings":{{"approvers":[{{"displayName":"azurz"}}]}},"resource":{{"type":"environment","id":"{env_id}","name":"Production"}},"timeout":1440}}"#
    );
    let (status, body, _) = call(
        &app,
        "POST",
        "/azurz/AzurzDemo/_apis/pipelines/checks/configurations?api-version=7.1-preview.1",
        Some(&auth),
        None,
        "application/json",
        Some(&approval),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let check: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(check["id"], 1);
    assert_eq!(check["type"]["name"], "Approval");
    assert_eq!(check["type"]["id"], "8c6f20a7-a545-4486-9777-f762fafe0d4d");
    assert_eq!(check["resource"]["name"], "Production");
    assert_eq!(check["resource"]["id"], env_id);
    assert_eq!(check["settings"]["approvers"][0]["displayName"], "azurz");
    assert_eq!(check["settings"]["minRequiredApprovers"], 1);
    assert_eq!(check["timeout"], 1440);
    let (status, body, _) = call(
        &app,
        "POST",
        "/azurz/AzurzDemo/_apis/pipelines/checks/configurations?api-version=7.1-preview.1",
        Some(&auth),
        None,
        "application/json",
        Some(&approval),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert!(body.contains("déjà"));
    let (status, body, _) = call(
        &app,
        "POST",
        "/azurz/AzurzDemo/_apis/pipelines/checks/configurations?api-version=7.1-preview.1",
        Some(&auth),
        None,
        "application/json",
        Some(&format!(
            r#"{{"type":{{"id":"fe1de3ee-a436-41b4-bb20-f6eb4cb879a7"}},"settings":{{"displayName":"Branch control","inputs":{{"allowedBranches":" "}}}},"resource":{{"type":"environment","id":"{env_id}"}}}}"#
        )),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert!(body.contains("branches"));
    let (status, body, _) = call(
        &app,
        "POST",
        "/azurz/AzurzDemo/_apis/pipelines/checks/configurations?api-version=7.1-preview.1",
        Some(&auth),
        None,
        "application/json",
        Some(&format!(
            r#"{{"type":{{"id":"fe1de3ee-a436-41b4-bb20-f6eb4cb879a7","name":"Task Check"}},"settings":{{"displayName":"Branch control","inputs":{{"allowedBranches":"refs/heads/main"}}}},"resource":{{"type":"environment","id":"{env_id}"}}}}"#
        )),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let branch: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(branch["id"], 2);
    assert_eq!(branch["type"]["name"], "Task Check");
    assert_eq!(branch["settings"]["inputs"]["allowedBranches"], "refs/heads/main");
    let (status, body, _) = call(
        &app,
        "POST",
        "/azurz/AzurzDemo/_apis/pipelines/checks/configurations?api-version=7.1-preview.1",
        Some(&auth),
        None,
        "application/json",
        Some(&format!(
            r#"{{"type":{{"id":"2ef31ad6-baa0-403a-8b45-2cbc9b4e5563"}},"settings":{{}},"resource":{{"type":"environment","id":"{env_id}"}}}}"#
        )),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let lock: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(lock["type"]["name"], "ExclusiveLock");
    assert_eq!(lock["id"], 3);
    let (status, body, _) = call(
        &app,
        "GET",
        &format!("/azurz/AzurzDemo/_apis/pipelines/checks/configurations?resourceType=environment&resourceId={env_id}&api-version=7.1-preview.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let page: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(page["count"], 3);
    let (status, body, _) = call(
        &app,
        "GET",
        "/azurz/AzurzDemo/_apis/pipelines/checks/configurations?resourceType=environment&resourceId=2&api-version=7.1-preview.1",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let empty: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(empty["count"], 0);
    let (status, body, _) = call(
        &app,
        "GET",
        "/azurz/AzurzDemo/_apis/pipelines/checks/configurations/2?api-version=7.1-preview.1",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let one: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(one["settings"]["inputs"]["allowedBranches"], "refs/heads/main");
    let (status, body, _) = call(
        &app,
        "GET",
        "/azurz/AzurzDemo/_apis/pipelines/checks/configurations/9?api-version=7.1-preview.1",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    let (status, body, _) = call(
        &app,
        "DELETE",
        "/azurz/AzurzDemo/_apis/pipelines/checks/configurations/1?api-version=7.1-preview.1",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT, "{body}");
    assert!(body.is_empty());
    let (status, body, _) = call(
        &app,
        "GET",
        &format!("/azurz/AzurzDemo/_apis/pipelines/checks/configurations?resourceType=environment&resourceId={env_id}&api-version=7.1-preview.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let left: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(left["count"], 2);
    let names: Vec<&str> = left["value"]
        .as_array()
        .unwrap()
        .iter()
        .map(|item| item["type"]["name"].as_str().unwrap())
        .collect();
    assert!(!names.contains(&"Approval"));
    let (status, body, _) = call(
        &app,
        "GET",
        "/azurz/AzurzDemo/_apis/pipelines/checks/checktypes?api-version=7.1-preview.1",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(body.contains("8c6f20a7-a545-4486-9777-f762fafe0d4d"));
    assert!(body.contains("ExclusiveLock"));
}

#[tokio::test]
async fn pipeline_run_waits_for_an_environment_approval() {
    let app = app();
    let auth = basic("azurz", "azurz-lab-pat");
    let (status, body, _) = call(
        &app,
        "POST",
        "/azurz/AzurzDemo/_apis/distributedtask/environments?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"name":"Production","description":"Ligne A"}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let environment: serde_json::Value = serde_json::from_str(&body).unwrap();
    let env_id = environment["id"].as_i64().unwrap().to_string();
    let (status, body, _) = call(
        &app,
        "POST",
        "/azurz/AzurzDemo/_apis/pipelines/checks/configurations?api-version=7.1-preview.1",
        Some(&auth),
        None,
        "application/json",
        Some(&format!(
            r#"{{"type":{{"id":"8c6f20a7-a545-4486-9777-f762fafe0d4d"}},"settings":{{"approvers":[{{"displayName":"azurz"}}],"minRequiredApprovers":1,"instructions":"Valider"}},"resource":{{"type":"environment","id":"{env_id}"}}}}"#
        )),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status, body, _) = call(
        &app,
        "POST",
        "/azurz/AzurzDemo/_apis/pipelines/pipelines?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"name":"CI"}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status, body, _) = call(
        &app,
        "POST",
        "/azurz/AzurzDemo/_apis/pipelines/pipelines/1/runs?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"resources":{"environments":[{"id":"9"}]}}"#),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert!(body.contains("environnement"));
    let (status, body, _) = call(
        &app,
        "POST",
        "/azurz/AzurzDemo/_apis/pipelines/pipelines/1/runs?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        Some(&format!(r#"{{"resources":{{"environments":[{{"id":"{env_id}","name":"Production"}}]}}}}"#)),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let run: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(run["state"], "inProgress");
    let run_id = run["id"].as_i64().unwrap();
    let (status, body, _) = call(
        &app,
        "GET",
        &format!("/azurz/AzurzDemo/_apis/pipelines/runs/{run_id}?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let waiting: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(waiting["state"], "inProgress");
    assert!(waiting.get("result").is_none());
    let (status, body, _) = call(
        &app,
        "GET",
        "/azurz/AzurzDemo/_apis/pipelines/approvals?state=pending&api-version=7.1-preview.1",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let pending: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(pending["count"], 1);
    assert_eq!(pending["value"][0]["status"], "pending");
    assert_eq!(pending["value"][0]["resource"]["name"], "Production");
    assert_eq!(pending["value"][0]["approvers"][0]["displayName"], "azurz");
    assert_eq!(pending["value"][0]["instructions"], "Valider");
    assert_eq!(pending["value"][0]["pipeline"]["name"], "CI-1");
    let approval_id = pending["value"][0]["id"].as_i64().unwrap();
    let (status, body, _) = call(
        &app,
        "GET",
        "/azurz/AzurzDemo/_apis/pipelines/approvals/9?api-version=7.1-preview.1",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    let (status, body, _) = call(
        &app,
        "PATCH",
        "/azurz/AzurzDemo/_apis/pipelines/approvals?api-version=7.1-preview.1",
        Some(&auth),
        None,
        "application/json",
        Some(r#"[{"approvalId":"1","status":"skipped"}]"#),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert!(body.contains("statut"));
    let (status, body, _) = call(
        &app,
        "PATCH",
        "/azurz/AzurzDemo/_apis/pipelines/approvals?api-version=7.1-preview.1",
        Some(&auth),
        None,
        "application/json",
        Some(r#"[{"approvalId":"9","status":"approved"}]"#),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    let (status, body, _) = call(
        &app,
        "PATCH",
        "/azurz/AzurzDemo/_apis/pipelines/approvals?api-version=7.1-preview.1",
        Some(&auth),
        None,
        "application/json",
        Some(&format!(r#"[{{"approvalId":{approval_id},"status":"approved","comment":"ok"}}]"#)),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let updated: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(updated[0]["status"], "approved");
    assert_eq!(updated[0]["comment"], "ok");
    let (status, body, _) = call(
        &app,
        "GET",
        &format!("/azurz/AzurzDemo/_apis/pipelines/runs/{run_id}?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let done: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(done["state"], "completed");
    assert_eq!(done["result"], "succeeded");
    let (status, body, _) = call(
        &app,
        "PATCH",
        "/azurz/AzurzDemo/_apis/pipelines/approvals?api-version=7.1-preview.1",
        Some(&auth),
        None,
        "application/json",
        Some(&format!(r#"[{{"approvalId":{approval_id},"status":"approved"}}]"#)),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert!(body.contains("déjà"));
    let (status, body, _) = call(
        &app,
        "POST",
        "/azurz/AzurzDemo/_apis/pipelines/pipelines/1/runs?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        Some(&format!(r#"{{"resources":{{"environments":[{{"id":"{env_id}"}}]}}}}"#)),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let second: serde_json::Value = serde_json::from_str(&body).unwrap();
    let second_id = second["id"].as_i64().unwrap();
    let (status, body, _) = call(
        &app,
        "GET",
        "/azurz/AzurzDemo/_apis/pipelines/approvals?state=pending&api-version=7.1-preview.1",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let pending: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(pending["count"], 1);
    let second_approval = pending["value"][0]["id"].as_i64().unwrap();
    let (status, body, _) = call(
        &app,
        "PATCH",
        "/azurz/AzurzDemo/_apis/pipelines/approvals?api-version=7.1-preview.1",
        Some(&auth),
        None,
        "application/json",
        Some(&format!(r#"[{{"approvalId":"{second_approval}","status":"rejected","comment":"non"}}]"#)),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status, body, _) = call(
        &app,
        "GET",
        &format!("/azurz/AzurzDemo/_apis/pipelines/runs/{second_id}?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let failed: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(failed["state"], "completed");
    assert_eq!(failed["result"], "failed");
    let (status, body, _) = call(
        &app,
        "POST",
        "/azurz/AzurzDemo/_apis/distributedtask/environments?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"name":"Staging"}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let staging: serde_json::Value = serde_json::from_str(&body).unwrap();
    let staging_id = staging["id"].as_i64().unwrap();
    let (status, body, _) = call(
        &app,
        "POST",
        "/azurz/AzurzDemo/_apis/pipelines/pipelines/1/runs?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        Some(&format!(r#"{{"resources":{{"environments":[{{"id":{staging_id}}}]}}}}"#)),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let open: serde_json::Value = serde_json::from_str(&body).unwrap();
    let open_id = open["id"].as_i64().unwrap();
    let (status, body, _) = call(
        &app,
        "GET",
        &format!("/azurz/AzurzDemo/_apis/pipelines/runs/{open_id}?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let free: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(free["state"], "completed");
    assert_eq!(free["result"], "succeeded");
}

#[tokio::test]
async fn secure_file_hides_its_bytes_until_download() {
    let app = app();
    let auth = basic("azurz", "azurz-lab-pat");
    let (status, body, _) = call(
        &app,
        "GET",
        "/azurz/AzurzDemo/_apis/distributedtask/securefiles?api-version=7.1-preview.1",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let empty: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(empty["count"], 0);
    let (status, body, _) = call(
        &app,
        "POST",
        "/azurz/AzurzDemo/_apis/distributedtask/securefiles?name=%20&api-version=7.1-preview.1",
        Some(&auth),
        None,
        "application/octet-stream",
        Some("secret"),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert!(body.contains("nom de fichier"));
    let (status, body, _) = call(
        &app,
        "POST",
        "/azurz/AzurzDemo/_apis/distributedtask/securefiles?name=kubeconfig&api-version=7.1-preview.1",
        Some(&auth),
        None,
        "application/octet-stream",
        Some("cluster: ligne-a"),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(!body.contains("cluster: ligne-a"));
    assert!(!body.contains("content"));
    let created: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(created["id"], 1);
    assert_eq!(created["name"], "kubeconfig");
    assert_eq!(created["createdBy"]["displayName"], "azurz");
    let (status, body, _) = call(
        &app,
        "POST",
        "/azurz/AzurzDemo/_apis/distributedtask/securefiles?name=Kubeconfig&api-version=7.1-preview.1",
        Some(&auth),
        None,
        "application/octet-stream",
        Some("autre"),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert!(body.contains("déjà"));
    let (status, body, _) = call(
        &app,
        "GET",
        "/azurz/AzurzDemo/_apis/distributedtask/securefiles/1?api-version=7.1-preview.1",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(!body.contains("cluster: ligne-a"));
    let (status, body, _) = call(
        &app,
        "GET",
        "/azurz/AzurzDemo/_apis/distributedtask/securefiles/1/download?api-version=7.1-preview.1",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body, "cluster: ligne-a");
    let (status, body, _) = call(
        &app,
        "GET",
        "/azurz/AzurzDemo/_apis/distributedtask/securefiles/1?download=true&api-version=7.1-preview.1",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body, "cluster: ligne-a");
    let (status, body, _) = call(
        &app,
        "GET",
        "/azurz/AzurzDemo/_apis/distributedtask/securefiles/9/download?api-version=7.1-preview.1",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    let (status, body, _) = call(
        &app,
        "POST",
        "/azurz/AzurzDemo/_apis/distributedtask/securefiles?api-version=7.1-preview.1",
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"name":"jeton","content":"secret-lab"}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let json_file: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(json_file["name"], "jeton");
    assert!(!body.contains("secret-lab"));
    let file_id = json_file["id"].as_i64().unwrap();
    let (status, body, _) = call(
        &app,
        "GET",
        &format!("/azurz/AzurzDemo/_apis/distributedtask/securefiles/{file_id}/download?api-version=7.1-preview.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, "secret-lab");
    let (status, body, _) = call(
        &app,
        "GET",
        "/azurz/AzurzDemo/_apis/distributedtask/securefiles?api-version=7.1-preview.1",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let page: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(page["count"], 2);
    assert!(!body.contains("secret-lab"));
    assert!(!body.contains("cluster: ligne-a"));
    let (status, body, _) = call(
        &app,
        "DELETE",
        "/azurz/AzurzDemo/_apis/distributedtask/securefiles/1?api-version=7.1-preview.1",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT, "{body}");
    assert!(body.is_empty());
    let (status, body, _) = call(
        &app,
        "GET",
        "/azurz/AzurzDemo/_apis/distributedtask/securefiles/1?api-version=7.1-preview.1",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    let (status, body, _) = call(
        &app,
        "GET",
        "/azurz/AzurzDemo/_apis/distributedtask/securefiles?api-version=7.1-preview.1",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let left: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(left["count"], 1);
    assert_eq!(left["value"][0]["name"], "jeton");
}

#[tokio::test]
async fn build_artifact_hides_its_bytes_until_download() {
    let app = app();
    let auth = basic("azurz", "azurz-lab-pat");
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
    let artifacts = format!("/azurz/AzurzDemo/_apis/build/builds/{build_id}/artifacts");
    let (status, body, _) = call(
        &app,
        "GET",
        &format!("{artifacts}?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let empty: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(empty["count"], 0);
    let (status, body, _) = call(
        &app,
        "POST",
        &format!("{artifacts}?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"name":"  ","resource":{"type":"PipelineArtifact","content":"secret"}}"#),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert!(body.contains("nom d'artefact absent"));
    let (status, body, _) = call(
        &app,
        "POST",
        &format!("{artifacts}?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"name":"drop","resource":{"type":"PipelineArtifact","data":"container/drop","content":"pipeline-drop"}}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let created: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(created["name"], "drop");
    assert_eq!(created["source"], "build");
    assert_eq!(created["resource"]["type"], "PipelineArtifact");
    assert_eq!(created["resource"]["data"], "container/drop");
    assert!(created.get("content").is_none());
    assert!(created["resource"].get("content").is_none());
    assert!(!body.contains("pipeline-drop"));
    let download_url = created["resource"]["downloadUrl"].as_str().unwrap();
    assert!(download_url.contains("artifactName=drop"));
    assert!(download_url.contains("%24format=zip"));
    let (status, body, _) = call(
        &app,
        "POST",
        &format!("{artifacts}?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"name":"Drop","resource":{"type":"PipelineArtifact","content":"autre"}}"#),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert!(body.contains("artefact déjà présent"));
    let (status, body, _) = call(
        &app,
        "GET",
        &format!("{artifacts}?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let listed: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(listed["count"], 1);
    assert_eq!(listed["value"][0]["name"], "drop");
    assert!(!body.contains("pipeline-drop"));
    let (status, body, _) = call(
        &app,
        "GET",
        &format!("{artifacts}?artifactName=drop&api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let named: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(named["name"], "drop");
    assert!(named.get("count").is_none());
    assert!(!body.contains("pipeline-drop"));
    let (status, body, _) = call(
        &app,
        "GET",
        &format!("{artifacts}/drop?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(body.contains("drop"));
    assert!(!body.contains("pipeline-drop"));
    let (status, body, _) = call(
        &app,
        "GET",
        &format!("{artifacts}?artifactName=absent&api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    assert!(body.contains("artefact introuvable"));
    let (status, body, _) = call(
        &app,
        "GET",
        &format!("{artifacts}?artifactName=drop&$format=zip&api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body, "pipeline-drop");
    let (status, body, _) = call(
        &app,
        "GET",
        &format!("{artifacts}?artifactName=drop&%24format=zip&api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body, "pipeline-drop");
    let (status, body, _) = call(
        &app,
        "GET",
        &format!("{artifacts}/drop/download?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body, "pipeline-drop");
    let (status, body, _) = call(
        &app,
        "DELETE",
        &format!("{artifacts}/drop?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT, "{body}");
    assert!(body.is_empty());
    let (status, body, _) = call(
        &app,
        "GET",
        &format!("{artifacts}/drop?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    let (status, body, _) = call(
        &app,
        "GET",
        &format!("{artifacts}?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let left: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(left["count"], 0);
}

#[tokio::test]
async fn pull_request_autocomplete_waits_for_a_required_reviewer() {
    let app = app();
    let auth = basic("azurz", "azurz-lab-pat");
    let (status, body, _) = call(
        &app,
        "POST",
        "/azurz/AzurzDemo/_apis/git/pullrequests?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"title":"Pompe auto","sourceRefName":"refs/heads/feature","targetRefName":"refs/heads/main"}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let pull: serde_json::Value = serde_json::from_str(&body).unwrap();
    let pull_id = pull["pullRequestId"].as_i64().unwrap();
    let path = format!("/azurz/AzurzDemo/_apis/git/pullrequests/{pull_id}");
    assert!(pull.get("autoCompleteSetBy").is_none());
    let (status, body, _) = call(
        &app,
        "POST",
        &format!("{path}/reviewers?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"displayName":"ligne","isRequired":true,"vote":0}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status, body, _) = call(
        &app,
        "PATCH",
        &format!("{path}?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"autoCompleteSetBy":{"id":"azurz"},"completionOptions":{"mergeStrategy":"squash","deleteSourceBranch":false}}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let waiting: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(waiting["status"], "active");
    assert_eq!(waiting["autoCompleteSetBy"]["displayName"], "azurz");
    assert_eq!(waiting["completionOptions"]["mergeStrategy"], "squash");
    let (status, body, _) = call(
        &app,
        "POST",
        &format!("{path}/reviewers?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"displayName":"ligne","isRequired":true,"vote":10}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status, body, _) = call(
        &app,
        "GET",
        &format!("{path}?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let merged: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(merged["status"], "completed");
    assert_eq!(merged["completionOptions"]["mergeStrategy"], "squash");
    assert!(!merged["lastMergeCommit"]["commitId"].as_str().unwrap_or("").is_empty());

    let (status, body, _) = call(
        &app,
        "POST",
        "/azurz/AzurzDemo/_apis/git/pullrequests?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"title":"Attente","sourceRefName":"refs/heads/feature","targetRefName":"refs/heads/main"}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let second: serde_json::Value = serde_json::from_str(&body).unwrap();
    let second_id = second["pullRequestId"].as_i64().unwrap();
    let second_path = format!("/azurz/AzurzDemo/_apis/git/pullrequests/{second_id}");
    let (status, body, _) = call(
        &app,
        "POST",
        &format!("{second_path}/reviewers?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"displayName":"ligne","isRequired":true,"vote":0}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status, body, _) = call(
        &app,
        "PATCH",
        &format!("{second_path}?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"autoCompleteSetBy":{"id":"azurz"},"completionOptions":{"mergeStrategy":"ff"}}"#),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert!(body.contains("stratégie de fusion inconnue"));
    let (status, body, _) = call(
        &app,
        "PATCH",
        &format!("{second_path}?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"autoCompleteSetBy":{"id":"azurz"},"completionOptions":{"mergeStrategy":"noFastForward"}}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let armed: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(armed["status"], "active");
    let (status, body, _) = call(
        &app,
        "PATCH",
        &format!("{second_path}?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"autoCompleteSetBy":null}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let cleared: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(cleared["status"], "active");
    assert!(cleared.get("autoCompleteSetBy").is_none() || cleared["autoCompleteSetBy"].is_null());
    let (status, body, _) = call(
        &app,
        "POST",
        &format!("{second_path}/reviewers?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"displayName":"ligne","isRequired":true,"vote":10}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status, body, _) = call(
        &app,
        "GET",
        &format!("{second_path}?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let still: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(still["status"], "active");

    let (status, body, _) = call(
        &app,
        "POST",
        "/azurz/AzurzDemo/_apis/git/pullrequests?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"title":"Direct","sourceRefName":"refs/heads/feature","targetRefName":"refs/heads/main"}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let third: serde_json::Value = serde_json::from_str(&body).unwrap();
    let third_id = third["pullRequestId"].as_i64().unwrap();
    let (status, body, _) = call(
        &app,
        "PATCH",
        &format!("/azurz/AzurzDemo/_apis/git/pullrequests/{third_id}?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"autoCompleteSetBy":{},"completionOptions":{"mergeStrategy":"noFastForward"}}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let immediate: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(immediate["status"], "completed");
    assert_eq!(immediate["autoCompleteSetBy"]["displayName"], "azurz");
    assert_eq!(immediate["completionOptions"]["mergeStrategy"], "noFastForward");

    let (status, body, _) = call(
        &app,
        "POST",
        "/azurz/AzurzDemo/_apis/git/pullrequests?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"title":"Refus","sourceRefName":"refs/heads/feature","targetRefName":"refs/heads/main"}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let fourth: serde_json::Value = serde_json::from_str(&body).unwrap();
    let fourth_id = fourth["pullRequestId"].as_i64().unwrap();
    let fourth_path = format!("/azurz/AzurzDemo/_apis/git/pullrequests/{fourth_id}");
    let (status, body, _) = call(
        &app,
        "POST",
        &format!("{fourth_path}/reviewers?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"displayName":"ligne","vote":-10}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status, body, _) = call(
        &app,
        "PATCH",
        &format!("{fourth_path}?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"autoCompleteSetBy":{"displayName":"azurz"},"completionOptions":{"mergeStrategy":"noFastForward"}}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let rejected: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(rejected["status"], "active");
    assert_eq!(rejected["autoCompleteSetBy"]["displayName"], "azurz");
}

#[tokio::test]
async fn service_connection_hides_its_secret_and_can_be_updated() {
    let app = app();
    let auth = basic("azurz", "azurz-lab-pat");
    let (status, body, _) = call(
        &app,
        "GET",
        "/azurz/AzurzDemo/_apis/serviceendpoint/endpoints?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let empty: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(empty["count"], 0);
    let (status, body, _) = call(
        &app,
        "POST",
        "/azurz/AzurzDemo/_apis/serviceendpoint/endpoints?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"name":" ","type":"generic","url":"http://ignition:8088"}"#),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert!(body.contains("nom de connexion absent"));
    let (status, body, _) = call(
        &app,
        "POST",
        "/azurz/AzurzDemo/_apis/serviceendpoint/endpoints?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"name":"smtp","type":"smtp","url":"http://mail"}"#),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert!(body.contains("type de connexion inconnu"));
    let (status, body, _) = call(
        &app,
        "POST",
        "/azurz/AzurzDemo/_apis/serviceendpoint/endpoints?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"name":"ignition","type":"generic","url":"http://ignition:8088","description":"Ligne A","authorization":{"scheme":"UsernamePassword","parameters":{"username":"admin","password":"secret"}}}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let created: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(created["name"], "ignition");
    assert_eq!(created["type"], "generic");
    assert_eq!(created["url"], "http://ignition:8088");
    assert_eq!(created["authorization"]["scheme"], "UsernamePassword");
    assert_eq!(created["authorization"]["parameters"]["username"], "admin");
    assert_eq!(created["authorization"]["parameters"]["password"], "");
    assert!(!body.contains("secret"));
    let id = created["id"].as_str().unwrap();
    let path = format!("/azurz/AzurzDemo/_apis/serviceendpoint/endpoints/{id}");
    let (status, body, _) = call(
        &app,
        "POST",
        "/azurz/AzurzDemo/_apis/serviceendpoint/endpoints?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"name":"Ignition","type":"generic","url":"http://autre"}"#),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert!(body.contains("connexion déjà présente"));
    let (status, body, _) = call(
        &app,
        "GET",
        &format!("{path}?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let fetched: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(fetched["authorization"]["parameters"]["password"], "");
    assert_eq!(fetched["description"], "Ligne A");
    assert!(!body.contains("secret"));
    let (status, body, _) = call(
        &app,
        "PUT",
        &format!("{path}?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"name":"ignition","type":"kubernetes","url":"http://ignition:8089","authorization":{"scheme":"Token","parameters":{"username":"admin","password":""}}}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let updated: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(updated["url"], "http://ignition:8089");
    assert_eq!(updated["type"], "kubernetes");
    assert_eq!(updated["authorization"]["scheme"], "Token");
    assert_eq!(updated["authorization"]["parameters"]["password"], "");
    assert!(!body.contains("secret"));
    let (status, body, _) = call(
        &app,
        "PUT",
        "/azurz/AzurzDemo/_apis/serviceendpoint/endpoints/endpoint-missing?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"name":"ignition","type":"generic","url":"http://ignition:8088"}"#),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    let (status, body, _) = call(
        &app,
        "GET",
        "/azurz/AzurzDemo/_apis/serviceendpoint/types?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(body.contains("generic"));
    assert!(body.contains("github"));
    let (status, body, _) = call(
        &app,
        "GET",
        "/azurz/_apis/serviceendpoint/types?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(body.contains("kubernetes"));
    let (status, _, _) = call(
        &app,
        "DELETE",
        &format!("{path}?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (status, body, _) = call(
        &app,
        "GET",
        &format!("{path}?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    let (status, body, _) = call(
        &app,
        "GET",
        "/azurz/AzurzDemo/_apis/serviceendpoint/endpoints?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let left: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(left["count"], 0);
}

#[tokio::test]
async fn variable_group_hides_a_secret_until_it_is_cleared() {
    let app = app();
    let auth = basic("azurz", "azurz-lab-pat");
    let (status, body, _) = call(
        &app,
        "GET",
        "/azurz/AzurzDemo/_apis/distributedtask/variablegroups?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let empty: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(empty["count"], 0);
    let (status, body, _) = call(
        &app,
        "POST",
        "/azurz/AzurzDemo/_apis/distributedtask/variablegroups?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"name":" ","type":"Vsts","variables":{}}"#),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert!(body.contains("nom de groupe absent"));
    let (status, body, _) = call(
        &app,
        "POST",
        "/azurz/AzurzDemo/_apis/distributedtask/variablegroups?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"name":"autre","type":"vault","variables":{}}"#),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert!(body.contains("type de groupe inconnu"));
    let (status, body, _) = call(
        &app,
        "POST",
        "/azurz/AzurzDemo/_apis/distributedtask/variablegroups?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"name":"ligne","description":"Capteurs","type":"Vsts","variables":{" ":{"value":"x"}}}"#),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert!(body.contains("nom de variable absent"));
    let (status, body, _) = call(
        &app,
        "POST",
        "/azurz/AzurzDemo/_apis/distributedtask/variablegroups?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"name":"ligne","description":"Capteurs","type":"Vsts","variables":{"host":{"value":"ignition","isSecret":false},"token":{"value":"ligne-clef","isSecret":true}}}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let created: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(created["name"], "ligne");
    assert_eq!(created["type"], "Vsts");
    assert_eq!(created["description"], "Capteurs");
    assert_eq!(created["variables"]["host"]["value"], "ignition");
    assert_eq!(created["variables"]["token"]["value"], "");
    assert_eq!(created["variables"]["token"]["isSecret"], true);
    assert!(!body.contains("ligne-clef"));
    let id = created["id"].as_i64().unwrap();
    let path = format!("/azurz/AzurzDemo/_apis/distributedtask/variablegroups/{id}");
    let (status, body, _) = call(
        &app,
        "POST",
        "/azurz/AzurzDemo/_apis/distributedtask/variablegroups?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"name":"Ligne","type":"Vsts","variables":{}}"#),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert!(body.contains("groupe déjà présent"));
    let (status, body, _) = call(
        &app,
        "GET",
        &format!("{path}?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(!body.contains("ligne-clef"));
    let (status, body, _) = call(
        &app,
        "PUT",
        &format!("{path}?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"name":"ligne","type":"Vsts","variables":{"host":{"value":"ignition.lab","isSecret":false},"token":{"value":"","isSecret":true},"port":{"value":"8088","isSecret":false}}}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let updated: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(updated["variables"]["host"]["value"], "ignition.lab");
    assert_eq!(updated["variables"]["port"]["value"], "8088");
    assert_eq!(updated["variables"]["token"]["value"], "");
    assert!(!body.contains("ligne-clef"));
    let (status, body, _) = call(
        &app,
        "PUT",
        &format!("{path}?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"name":"ligne","type":"Vsts","variables":{"host":{"value":"ignition.lab","isSecret":false},"token":{"value":"","isSecret":false},"port":{"value":"8088","isSecret":false}}}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let revealed: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(revealed["variables"]["token"]["isSecret"], false);
    assert_eq!(revealed["variables"]["token"]["value"], "ligne-clef");
    let (status, _, _) = call(
        &app,
        "DELETE",
        &format!("{path}?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (status, body, _) = call(
        &app,
        "GET",
        &format!("{path}?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    let (status, body, _) = call(
        &app,
        "GET",
        "/azurz/AzurzDemo/_apis/distributedtask/variablegroups?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let left: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(left["count"], 0);
}

#[tokio::test]
async fn build_receives_variables_from_its_group() {
    let app = app();
    let auth = basic("azurz", "azurz-lab-pat");
    let (status, body, _) = call(
        &app,
        "GET",
        "/azurz/AzurzDemo/_apis/build/definitions/1?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let seeded: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(seeded["name"], "CI");
    assert_eq!(seeded["variableGroups"], serde_json::json!([]));
    let (status, body, _) = call(
        &app,
        "PUT",
        "/azurz/AzurzDemo/_apis/build/definitions/1?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"variableGroups":[{"id":999}]}"#),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert!(body.contains("groupe introuvable"));
    let (status, body, _) = call(
        &app,
        "GET",
        "/azurz/AzurzDemo/_apis/build/definitions/1?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let untouched: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(untouched["variableGroups"], serde_json::json!([]));
    let (status, body, _) = call(
        &app,
        "POST",
        "/azurz/AzurzDemo/_apis/distributedtask/variablegroups?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"name":"ligne","type":"Vsts","variables":{"host":{"value":"ignition","isSecret":false},"token":{"value":"ligne-clef","isSecret":true}}}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let created: serde_json::Value = serde_json::from_str(&body).unwrap();
    let group_id = created["id"].as_i64().unwrap();
    let (status, body, _) = call(
        &app,
        "PUT",
        "/azurz/AzurzDemo/_apis/build/definitions/1?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        Some(&format!(r#"{{"variableGroups":[{{"id":{group_id}}}]}}"#)),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let linked: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(linked["variableGroups"][0]["name"], "ligne");
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
    let queued: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(queued["variables"]["host"]["value"], "ignition");
    assert_eq!(queued["variables"]["token"]["value"], "");
    assert_eq!(queued["variables"]["token"]["isSecret"], true);
    assert_eq!(queued["variableGroups"][0]["name"], "ligne");
    assert!(!body.contains("ligne-clef"));
    let build_id = queued["id"].as_i64().unwrap();
    let (status, body, _) = call(
        &app,
        "PUT",
        &format!("/azurz/AzurzDemo/_apis/distributedtask/variablegroups/{group_id}?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"name":"ligne","type":"Vsts","variables":{"host":{"value":"autre","isSecret":false},"token":{"value":"","isSecret":true}}}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
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
    assert_eq!(finished["variables"]["host"]["value"], "ignition");
    assert_eq!(finished["variables"]["token"]["value"], "");
    assert_eq!(finished["variables"]["token"]["isSecret"], true);
    assert!(!body.contains("ligne-clef"));
    let (status, body, _) = call(
        &app,
        "POST",
        "/azurz/AzurzDemo/_apis/build/definitions?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"name":"Nuit","variableGroups":[{"id":999}]}"#),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert!(body.contains("groupe introuvable"));
    let (status, body, _) = call(
        &app,
        "POST",
        "/azurz/AzurzDemo/_apis/build/definitions?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        Some(&format!(r#"{{"name":"Nuit","variableGroups":[{{"id":{group_id}}}]}}"#)),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let night: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(night["name"], "Nuit");
    assert_eq!(night["variableGroups"][0]["name"], "ligne");
    let (status, _, _) = call(
        &app,
        "DELETE",
        &format!("/azurz/AzurzDemo/_apis/distributedtask/variablegroups/{group_id}?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
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
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert!(body.contains("groupe introuvable"));
}

#[tokio::test]
async fn merge_commit_keeps_source_files_after_the_target_moves() {
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
        &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/refs?filter=heads&api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let heads: serde_json::Value = serde_json::from_str(&body).unwrap();
    let main = heads["value"]
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
        &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/refs?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some(&format!(
            r#"[{{"name":"refs/heads/fusion-file","oldObjectId":"0000000000000000000000000000000000000000","newObjectId":"{main}"}}]"#
        )),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status, body, _) = call(
        &app,
        "POST",
        &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/pushes?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some(&format!(
            r#"{{"refUpdates":[{{"name":"refs/heads/fusion-file","oldObjectId":"{main}"}}],"commits":[{{"comment":"F01 Pompe","changes":[{{"changeType":"add","item":{{"path":"/src/fusion-pompe.txt"}},"newContent":{{"content":"debit-fusion","contentType":"rawtext"}}}}]}}]}}"#
        )),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let push: serde_json::Value = serde_json::from_str(&body).unwrap();
    let source_tip = push["commits"][0]["commitId"].as_str().unwrap().to_string();
    let (status, body, _) = call(
        &app,
        "POST",
        &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/pushes?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some(&format!(
            r#"{{"refUpdates":[{{"name":"refs/heads/main","oldObjectId":"{main}"}}],"commits":[{{"comment":"Cible vanne","changes":[{{"changeType":"add","item":{{"path":"/src/fusion-cible.txt"}},"newContent":{{"content":"cible","contentType":"rawtext"}}}}]}}]}}"#
        )),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let target_push: serde_json::Value = serde_json::from_str(&body).unwrap();
    let target_tip = target_push["commits"][0]["commitId"].as_str().unwrap().to_string();
    let (status, body, _) = call(
        &app,
        "POST",
        &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/pullrequests?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"title":"Revue fusion fichiers","sourceRefName":"refs/heads/fusion-file","targetRefName":"refs/heads/main"}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let pull: serde_json::Value = serde_json::from_str(&body).unwrap();
    let pull_id = pull["pullRequestId"].as_i64().unwrap();
    let (status, body, _) = call(
        &app,
        "PATCH",
        &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/pullrequests/{pull_id}?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"status":"completed","completionOptions":{"mergeStrategy":"noFastForward","deleteSourceBranch":false}}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let done: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(done["status"], "completed");
    assert_eq!(done["lastMergeSourceCommit"]["commitId"], source_tip);
    assert_eq!(done["lastMergeTargetCommit"]["commitId"], target_tip);
    let merge = done["lastMergeCommit"]["commitId"].as_str().unwrap();
    let (status, body, _) = call(
        &app,
        "GET",
        &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/commits/{merge}?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let commit: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(commit["comment"], "Fusion de refs/heads/fusion-file dans refs/heads/main");
    assert_eq!(commit["parents"].as_array().unwrap().len(), 2);
    assert_eq!(commit["parents"][0], target_tip);
    assert_eq!(commit["parents"][1], source_tip);
    let (status, body, _) = call(
        &app,
        "GET",
        &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/commits/{merge}/changes?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let changes: serde_json::Value = serde_json::from_str(&body).unwrap();
    let entry = changes["changes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["item"]["path"] == "/src/fusion-pompe.txt")
        .unwrap();
    assert_eq!(entry["changeType"], "add");
    assert_eq!(entry["newContent"]["content"], "debit-fusion");
    let (status, body, _) = call(
        &app,
        "GET",
        &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/items?path=/src/fusion-pompe.txt&versionDescriptor.version={merge}&versionDescriptor.versionType=commit&api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let at_merge: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(at_merge["content"], "debit-fusion");
    let (status, body, _) = call(
        &app,
        "GET",
        &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/items?path=/src/fusion-cible.txt&api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let cible: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(cible["content"], "cible");
    let (status, body, _) = call(
        &app,
        "GET",
        &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/items?path=/src/fusion-pompe.txt&api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let current: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(current["content"], "debit-fusion");
    let (status, body, _) = call(
        &app,
        "GET",
        &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/refs?filter=heads&api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(body.contains("refs/heads/fusion-file"), "{body}");
}

#[tokio::test]
async fn definition_variables_override_the_group_and_stay_masked() {
    let app = app();
    let auth = basic("azurz", "azurz-lab-pat");
    let (status, body, _) = call(
        &app,
        "GET",
        "/azurz/AzurzDemo/_apis/build/definitions/1?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let seeded: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(seeded["variables"], serde_json::json!({}));
    let (status, body, _) = call(
        &app,
        "PUT",
        "/azurz/AzurzDemo/_apis/build/definitions/1?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"variables":{" ":{"value":"x"}}}"#),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert!(body.contains("nom de variable absent"));
    let (status, body, _) = call(
        &app,
        "GET",
        "/azurz/AzurzDemo/_apis/build/definitions/1?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let untouched: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(untouched["variables"], serde_json::json!({}));
    let (status, body, _) = call(
        &app,
        "POST",
        "/azurz/AzurzDemo/_apis/distributedtask/variablegroups?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"name":"ligne","type":"Vsts","variables":{"host":{"value":"ignition","isSecret":false},"token":{"value":"ligne-clef","isSecret":true}}}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let created: serde_json::Value = serde_json::from_str(&body).unwrap();
    let group_id = created["id"].as_i64().unwrap();
    let (status, body, _) = call(
        &app,
        "PUT",
        "/azurz/AzurzDemo/_apis/build/definitions/1?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        Some(&format!(
            r#"{{"variableGroups":[{{"id":{group_id}}}],"variables":{{"host":{{"value":"lab","allowOverride":true}},"token":{{"value":"ligne-def","isSecret":true,"allowOverride":false}},"port":{{"value":"8088"}}}}}}"#
        )),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let linked: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(linked["variables"]["host"]["value"], "lab");
    assert_eq!(linked["variables"]["token"]["value"], "");
    assert_eq!(linked["variables"]["token"]["isSecret"], true);
    assert_eq!(linked["variables"]["token"]["allowOverride"], false);
    assert_eq!(linked["variables"]["port"]["value"], "8088");
    assert!(!body.contains("ligne-def"));
    assert!(!body.contains("ligne-clef"));
    let (status, body, _) = call(
        &app,
        "PUT",
        "/azurz/AzurzDemo/_apis/build/definitions/1?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"variables":{"host":{"value":"lab","allowOverride":true},"token":{"value":"","isSecret":true,"allowOverride":false},"port":{"value":"8088"}}}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(!body.contains("ligne-def"));
    let (status, body, _) = call(
        &app,
        "PUT",
        "/azurz/AzurzDemo/_apis/build/definitions/1?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"variables":{"host":{"value":"lab","allowOverride":true},"token":{"value":"","isSecret":false,"allowOverride":false},"port":{"value":"8088"}}}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let revealed: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(revealed["variables"]["token"]["value"], "ligne-def");
    let (status, body, _) = call(
        &app,
        "PUT",
        "/azurz/AzurzDemo/_apis/build/definitions/1?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"variables":{"host":{"value":"lab","allowOverride":true},"token":{"value":"","isSecret":true,"allowOverride":false},"port":{"value":"8088"}}}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(!body.contains("ligne-def"));
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
    let queued: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(queued["variables"]["host"]["value"], "lab");
    assert_eq!(queued["variables"]["port"]["value"], "8088");
    assert_eq!(queued["variables"]["token"]["value"], "");
    assert_eq!(queued["variables"]["token"]["isSecret"], true);
    assert!(!body.contains("ligne-def"));
    assert!(!body.contains("ligne-clef"));
    let (status, body, _) = call(
        &app,
        "POST",
        "/azurz/AzurzDemo/_apis/build/builds?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"definition":{"id":1},"sourceBranch":"refs/heads/main","parameters":"{\"host\":\"atelier\",\"token\":\"pirate\"}"}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let overridden: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(overridden["variables"]["host"]["value"], "atelier");
    assert_eq!(overridden["variables"]["token"]["value"], "");
    assert_eq!(overridden["variables"]["token"]["isSecret"], true);
    assert_eq!(overridden["parameters"], r#"{"host":"atelier","token":"pirate"}"#);
    assert!(!body.contains("ligne-def"));
    assert!(!body.contains("ligne-clef"));
    let (status, body, _) = call(
        &app,
        "POST",
        "/azurz/AzurzDemo/_apis/build/builds?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"definition":{"id":1},"parameters":"pas-du-json"}"#),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert!(body.contains("paramètres de file illisibles"));
}

#[tokio::test]
async fn ci_trigger_queues_a_build_for_the_matching_branch_and_path() {
    let app = app();
    let auth = basic("azurz", "azurz-lab-pat");
    let (status, body, _) = call(
        &app,
        "GET",
        "/azurz/AzurzDemo/_apis/build/definitions/1?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let seeded: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(seeded["triggers"], serde_json::json!([]));
    let (status, body, _) = call(
        &app,
        "PUT",
        "/azurz/AzurzDemo/_apis/build/definitions/1?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"triggers":[{"triggerType":"octopus"}]}"#),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert!(body.contains("déclencheur inconnu"));
    let (status, body, _) = call(
        &app,
        "PUT",
        "/azurz/AzurzDemo/_apis/build/definitions/1?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"triggers":[{"triggerType":"continuousIntegration","branchFilters":[" "]}]}"#),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert!(body.contains("filtre de branche absent"));
    let (status, body, _) = call(
        &app,
        "PUT",
        "/azurz/AzurzDemo/_apis/build/definitions/1?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"queueStatus":"zzz"}"#),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert!(body.contains("état de file inconnu"));
    let (status, body, _) = call(
        &app,
        "GET",
        "/azurz/AzurzDemo/_apis/build/definitions/1?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let untouched: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(untouched["triggers"], serde_json::json!([]));
    assert_eq!(untouched["queueStatus"], "enabled");
    let (status, body, _) = call(
        &app,
        "PUT",
        "/azurz/AzurzDemo/_apis/build/definitions/1?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"triggers":[{"triggerType":"continuousIntegration","branchFilters":["+refs/heads/main","-refs/heads/wip"],"pathFilters":["+/src/*"],"batchChanges":false}]}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let armed: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(armed["triggers"][0]["triggerType"], "continuousIntegration");
    assert_eq!(armed["triggers"][0]["branchFilters"][0], "+refs/heads/main");
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
    let push = |app: &axum::Router, repo: &str, branch: &str, old: &str, path: &str| {
        let app = app.clone();
        let repo = repo.to_string();
        let branch = branch.to_string();
        let old = old.to_string();
        let path = path.to_string();
        let auth = basic("azurz", "azurz-lab-pat");
        async move {
            call(
                &app,
                "POST",
                &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo}/pushes?api-version=7.1"),
                Some(&auth),
                None,
                "application/json",
                Some(&format!(
                    r#"{{"refUpdates":[{{"name":"{branch}","oldObjectId":"{old}"}}],"commits":[{{"comment":"sonde","changes":[{{"changeType":"add","item":{{"path":"{path}"}},"newContent":{{"content":"sonde","contentType":"rawtext"}}}}]}}]}}"#
                )),
            )
            .await
        }
    };
    let (status, body, _) = push(&app, repo_id, "refs/heads/feature", "0000000000000000000000000000000000000000", "/src/hors.txt").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status, body, _) = call(
        &app,
        "GET",
        "/azurz/AzurzDemo/_apis/build/builds?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let quiet: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(quiet["count"], 0);
    let (status, body, _) = call(
        &app,
        "GET",
        &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/refs?filter=heads&api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let refs: serde_json::Value = serde_json::from_str(&body).unwrap();
    let main = refs["value"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["name"] == "refs/heads/main")
        .unwrap();
    let old = main["objectId"].as_str().unwrap();
    let (status, body, _) = push(&app, repo_id, "refs/heads/main", old, "/docs/note.txt").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status, body, _) = call(
        &app,
        "GET",
        "/azurz/AzurzDemo/_apis/build/builds?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let still: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(still["count"], 0, "{body}");
    let (status, body, _) = call(
        &app,
        "GET",
        &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/refs?filter=heads&api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let refs: serde_json::Value = serde_json::from_str(&body).unwrap();
    let main = refs["value"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["name"] == "refs/heads/main")
        .unwrap();
    let old = main["objectId"].as_str().unwrap().to_string();
    let (status, body, _) = push(&app, repo_id, "refs/heads/main", &old, "/src/pompe-ci.txt").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status, body, _) = call(
        &app,
        "GET",
        "/azurz/AzurzDemo/_apis/build/builds?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let queued: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(queued["count"], 1);
    assert_eq!(queued["value"][0]["reason"], "individualCI");
    assert_eq!(queued["value"][0]["sourceBranch"], "refs/heads/main");
    assert_eq!(queued["value"][0]["definition"]["id"], 1);
    assert_eq!(queued["value"][0]["status"], "completed");
    let (status, body, _) = call(
        &app,
        "PUT",
        "/azurz/AzurzDemo/_apis/build/definitions/1?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"queueStatus":"disabled"}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let paused: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(paused["queueStatus"], "disabled");
    assert_eq!(paused["triggers"][0]["triggerType"], "continuousIntegration");
    let (status, body, _) = call(
        &app,
        "GET",
        &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/refs?filter=heads&api-version=7.1"),
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
    let (status, body, _) = push(&app, repo_id, "refs/heads/main", &old, "/src/pause.txt").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status, body, _) = call(
        &app,
        "GET",
        "/azurz/AzurzDemo/_apis/build/builds?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let held: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(held["count"], 1, "{body}");
    let (status, body, _) = call(
        &app,
        "PUT",
        "/azurz/AzurzDemo/_apis/build/definitions/1?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"queueStatus":"enabled","triggers":[{"triggerType":"continuousIntegration","branchFilters":["+refs/heads/main"],"pathFilters":["+/src/*"],"batchChanges":true}]}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status, body, _) = call(
        &app,
        "GET",
        &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/refs?filter=heads&api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let refs: serde_json::Value = serde_json::from_str(&body).unwrap();
    let mut old = refs["value"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["name"] == "refs/heads/main")
        .unwrap()["objectId"]
        .as_str()
        .unwrap()
        .to_string();
    let (status, body, _) = push(&app, repo_id, "refs/heads/main", &old, "/src/lot-a.txt").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let pushed: serde_json::Value = serde_json::from_str(&body).unwrap();
    old = pushed["refUpdates"][0]["newObjectId"].as_str().unwrap().to_string();
    let (status, body, _) = push(&app, repo_id, "refs/heads/main", &old, "/src/lot-b.txt").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status, body, _) = call(
        &app,
        "GET",
        "/azurz/AzurzDemo/_apis/build/builds?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let batched: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(batched["count"], 2, "{body}");
    let (status, body, _) = call(
        &app,
        "POST",
        "/azurz/AzurzDemo/_apis/build/definitions?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"name":"Revue","triggers":[{"triggerType":"pullRequest","branchFilters":["+refs/heads/main"]}]}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let review: serde_json::Value = serde_json::from_str(&body).unwrap();
    let review_id = review["id"].as_i64().unwrap();
    let (status, body, _) = call(
        &app,
        "POST",
        &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/pullrequests?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"title":"Revue CI","sourceRefName":"refs/heads/feature","targetRefName":"refs/heads/main"}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status, body, _) = call(
        &app,
        "POST",
        "/azurz/AzurzDemo/_apis/build/definitions?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"name":"Brouillon","triggers":[{"triggerType":"pullRequest","branchFilters":["+refs/heads/main"]}]}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let draft_def: serde_json::Value = serde_json::from_str(&body).unwrap();
    let draft_id = draft_def["id"].as_i64().unwrap();
    let (status, body, _) = call(
        &app,
        "POST",
        &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/pullrequests?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"title":"Brouillon","sourceRefName":"refs/heads/feature","targetRefName":"refs/heads/main","isDraft":true}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status, body, _) = call(
        &app,
        "GET",
        "/azurz/AzurzDemo/_apis/build/builds?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let builds: serde_json::Value = serde_json::from_str(&body).unwrap();
    let rows = builds["value"].as_array().unwrap();
    let review_builds: Vec<_> = rows
        .iter()
        .filter(|item| item["definition"]["id"].as_i64() == Some(review_id))
        .collect();
    assert_eq!(review_builds.len(), 1);
    assert_eq!(review_builds[0]["reason"], "pullRequest");
    assert_eq!(review_builds[0]["sourceBranch"], "refs/heads/feature");
    assert!(rows.iter().all(|item| item["definition"]["id"].as_i64() != Some(draft_id)));
}

#[tokio::test]
async fn board_columns_replace_for_one_team_and_keep_the_other() {
    let app = app();
    let auth = basic("azurz", "azurz-lab-pat");
    let columns = "/azurz/AzurzDemo/AzurzDemo%20Team/_apis/work/boards/Stories/columns?api-version=7.1";
    let (status, body, _) = call(&app, "GET", columns, Some(&auth), None, "application/json", None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let defaults: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(defaults["count"], 4);
    assert_eq!(defaults["value"][1]["name"], "Active");
    assert_eq!(defaults["value"][0]["columnType"], "incoming");
    assert_eq!(defaults["value"][3]["columnType"], "outgoing");

    let rejected = [
        (r#"{"name":"New"}"#, "colonnes illisibles"),
        (r#"[{"name":" "}]"#, "nom de colonne absent"),
        (
            r#"[{"name":"Active","columnType":"incoming"},{"name":"active","columnType":"outgoing"}]"#,
            "colonne déjà présente",
        ),
        (
            r#"[{"name":"Active","columnType":"inProgress"},{"name":"New","columnType":"incoming"},{"name":"Closed","columnType":"outgoing"}]"#,
            "colonnes d'entrée et de sortie requises",
        ),
        (
            r#"[{"name":"New","columnType":"incoming"},{"name":"Active","columnType":"octopus"},{"name":"Closed","columnType":"outgoing"}]"#,
            "type de colonne inconnu",
        ),
        (
            r#"[{"name":"New","columnType":"incoming","stateMappings":{"Bug":" "}},{"name":"Closed","columnType":"outgoing"}]"#,
            "état de colonne absent",
        ),
        (
            r#"[{"name":"New","columnType":"incoming"},{"name":"Closed","columnType":"outgoing","itemLimit":-1}]"#,
            "limite de colonne invalide",
        ),
    ];
    for (payload, message) in rejected {
        let (status, body, _) = call(
            &app,
            "PUT",
            columns,
            Some(&auth),
            None,
            "application/json",
            Some(payload),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
        let error: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(error["message"], message, "{body}");
    }
    let (status, body, _) = call(&app, "GET", columns, Some(&auth), None, "application/json", None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let untouched: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(untouched["count"], 4, "{body}");
    assert!(!body.contains("Revue"), "{body}");

    let (status, body, _) = call(
        &app,
        "PUT",
        columns,
        Some(&auth),
        None,
        "application/json",
        Some(
            r#"[{"name":"New","columnType":"incoming"},{"name":"Active","columnType":"inProgress"},{"name":"Revue","itemLimit":2,"stateMappings":{"User Story":"En revue"}},{"name":"Closed","columnType":"outgoing"}]"#,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let saved: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(saved["count"], 4);
    assert_eq!(saved["value"][2]["name"], "Revue");
    assert_eq!(saved["value"][2]["columnType"], "inProgress");
    assert_eq!(saved["value"][2]["itemLimit"], 2);
    assert_eq!(saved["value"][2]["isSplit"], false);
    assert_eq!(saved["value"][2]["stateMappings"]["User Story"], "En revue");
    assert_eq!(saved["value"][2]["stateMappings"]["Bug"], "Revue");
    assert_eq!(saved["value"][2]["stateMappings"]["Task"], "Revue");

    let (status, body, _) = call(
        &app,
        "PUT",
        columns,
        Some(&auth),
        None,
        "application/json",
        Some(r#"[{"name":"Revue","columnType":"incoming"},{"name":"revue","columnType":"outgoing"}]"#),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    let (status, body, _) = call(&app, "GET", columns, Some(&auth), None, "application/json", None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let kept: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(kept["value"][2]["name"], "Revue");
    assert_eq!(kept["value"][2]["stateMappings"]["User Story"], "En revue");

    let (status, body, _) = call(
        &app,
        "GET",
        "/azurz/AzurzDemo/_apis/work/boards/Stories/columns?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(body.contains("En revue"), "{body}");
    let (status, body, _) = call(
        &app,
        "GET",
        "/azurz/AzurzDemo/Autre/_apis/work/boards/Stories/columns?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let other: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(other["count"], 4);
    assert!(!body.contains("Revue"), "{body}");
    let (status, body, _) = call(
        &app,
        "GET",
        "/azurz/AzurzDemo/AzurzDemo%20Team/_apis/work/boards/Stories?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let board: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(board["columns"][2]["name"], "Revue");
    assert_eq!(board["allowedMappings"]["User Story"][2], "En revue");
}

#[tokio::test]
async fn board_rows_replace_for_one_team_and_keep_the_other() {
    let app = app();
    let auth = basic("azurz", "azurz-lab-pat");
    let rows = "/azurz/AzurzDemo/AzurzDemo%20Team/_apis/work/boards/Stories/rows?api-version=7.1";
    let (status, body, _) = call(&app, "GET", rows, Some(&auth), None, "application/json", None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let defaults: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(defaults["count"], 1);
    assert!(defaults["value"][0]["name"].is_null(), "{body}");
    assert_eq!(defaults["value"][0]["id"], "row-1");

    let rejected = [
        (r#"{"name":null}"#, "lignes illisibles"),
        (r#"[]"#, "ligne par défaut requise"),
        (r#"[{"name":"Urgent"}]"#, "ligne par défaut requise"),
        (r#"[{"name":" "}]"#, "ligne par défaut requise"),
        (r#"[{"name":null},{"name":" "}]"#, "nom de ligne absent"),
        (r#"[{"name":null},{"name":"Urgent"},{"name":"urgent"}]"#, "ligne déjà présente"),
        (r#"[{"id":"row-1","name":null},{"id":"row-1","name":"Autre"}]"#, "ligne déjà présente"),
    ];
    for (payload, message) in rejected {
        let (status, body, _) = call(&app, "PUT", rows, Some(&auth), None, "application/json", Some(payload)).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
        let error: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(error["message"], message, "{body}");
    }
    let (status, body, _) = call(&app, "GET", rows, Some(&auth), None, "application/json", None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let untouched: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(untouched["count"], 1, "{body}");
    assert!(!body.contains("Urgent"), "{body}");

    let (status, body, _) = call(
        &app,
        "PUT",
        rows,
        Some(&auth),
        None,
        "application/json",
        Some(r#"[{"name":null},{"name":"Urgent"}]"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let saved: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(saved["count"], 2);
    assert!(saved["value"][0]["name"].is_null());
    assert_eq!(saved["value"][0]["id"], "row-1");
    assert_eq!(saved["value"][1]["name"], "Urgent");
    assert_eq!(saved["value"][1]["id"], "row-2");

    let (status, body, _) = call(
        &app,
        "PUT",
        rows,
        Some(&auth),
        None,
        "application/json",
        Some(r#"[{"name":null},{"name":"Urgent"},{"name":"urgent"}]"#),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    let (status, body, _) = call(&app, "GET", rows, Some(&auth), None, "application/json", None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let kept: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(kept["count"], 2);
    assert_eq!(kept["value"][1]["name"], "Urgent");

    let (status, body, _) = call(
        &app,
        "GET",
        "/azurz/AzurzDemo/_apis/work/boards/Stories/rows?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(body.contains("Urgent"), "{body}");
    let (status, body, _) = call(
        &app,
        "GET",
        "/azurz/AzurzDemo/Autre/_apis/work/boards/Stories/rows?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let other: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(other["count"], 1);
    assert!(!body.contains("Urgent"), "{body}");
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
    let columns: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(columns["count"], 4);
    assert_eq!(columns["value"][1]["name"], "Active");
    let (status, body, _) = call(
        &app,
        "GET",
        "/azurz/AzurzDemo/AzurzDemo%20Team/_apis/work/boards/Stories?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let board: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(board["rows"][1]["name"], "Urgent");
    assert!(board["rows"][0]["name"].is_null());
}

#[tokio::test]
async fn work_item_paths_identity_and_tags_follow_the_classification() {
    let app = app();
    let auth = basic("azurz", "azurz-lab-pat");
    let (status, body, _) = call(
        &app,
        "POST",
        "/azurz/AzurzDemo/_apis/wit/workitems/$User%20Story?api-version=7.1",
        Some(&auth),
        None,
        "application/json-patch+json",
        Some(r#"[{"op":"add","path":"/fields/System.Title","value":"Hors zone"},{"op":"add","path":"/fields/System.AreaPath","value":"AzurzDemo\\Inconnue"}]"#),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    let error: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(error["message"], "zone inconnue");
    let (status, body, _) = call(
        &app,
        "POST",
        "/azurz/AzurzDemo/_apis/wit/workitems/$User%20Story?api-version=7.1",
        Some(&auth),
        None,
        "application/json-patch+json",
        Some(r#"[{"op":"add","path":"/fields/System.Title","value":"Sans zone"},{"op":"add","path":"/fields/System.AreaPath","value":" "}]"#),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    let error: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(error["message"], "chemin de zone absent");
    let (status, body, _) = call(
        &app,
        "GET",
        "/azurz/AzurzDemo/_apis/wit/workitems?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let empty: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(empty["count"], 0, "{body}");

    let (status, body, _) = call(
        &app,
        "POST",
        "/azurz/AzurzDemo/_apis/wit/classificationnodes/areas?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"name":"Ligne"}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status, body, _) = call(
        &app,
        "POST",
        "/azurz/AzurzDemo/_apis/wit/workitems/$User%20Story?api-version=7.1",
        Some(&auth),
        None,
        "application/json-patch+json",
        Some(r#"[{"op":"add","path":"/fields/System.Title","value":"Pompe"},{"op":"add","path":"/fields/System.AreaPath","value":"azurzdemo\\ligne"},{"op":"add","path":"/fields/System.IterationPath","value":"AzurzDemo\\Sprint 1"},{"op":"add","path":"/fields/System.AssignedTo","value":"ada"},{"op":"add","path":"/fields/System.Tags","value":" pompe ; pompe ; ligne "}]"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let created: serde_json::Value = serde_json::from_str(&body).unwrap();
    let id = created["id"].as_i64().unwrap();
    assert_eq!(created["fields"]["System.AreaPath"], "AzurzDemo\\Ligne");
    assert_eq!(created["fields"]["System.IterationPath"], "AzurzDemo\\Sprint 1");
    assert_eq!(created["fields"]["System.AssignedTo"]["displayName"], "ada");
    assert_eq!(created["fields"]["System.AssignedTo"]["uniqueName"], "ada");
    assert_eq!(created["fields"]["System.Tags"], "pompe; ligne");
    assert_eq!(created["rev"], 1);

    let (status, body, _) = call(
        &app,
        "PATCH",
        &format!("/azurz/AzurzDemo/_apis/wit/workitems/{id}?api-version=7.1"),
        Some(&auth),
        None,
        "application/json-patch+json",
        Some(r#"[{"op":"replace","path":"/fields/System.Title","value":"Pompe changée"},{"op":"replace","path":"/fields/System.AreaPath","value":"AzurzDemo\\Absente"}]"#),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    let error: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(error["message"], "zone inconnue");
    let (status, body, _) = call(
        &app,
        "GET",
        &format!("/azurz/AzurzDemo/_apis/wit/workitems/{id}?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let kept: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(kept["rev"], 1);
    assert_eq!(kept["fields"]["System.Title"], "Pompe");
    assert_eq!(kept["fields"]["System.AreaPath"], "AzurzDemo\\Ligne");

    let (status, body, _) = call(
        &app,
        "PATCH",
        &format!("/azurz/AzurzDemo/_apis/wit/workitems/{id}?api-version=7.1"),
        Some(&auth),
        None,
        "application/json-patch+json",
        Some(r#"[{"op":"replace","path":"/fields/System.IterationPath","value":"AzurzDemo\\Sprint 9"}]"#),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    let error: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(error["message"], "itération inconnue");
    let (status, body, _) = call(
        &app,
        "PATCH",
        &format!("/azurz/AzurzDemo/_apis/wit/workitems/{id}?api-version=7.1"),
        Some(&auth),
        None,
        "application/json-patch+json",
        Some(r#"[{"op":"replace","path":"/fields/System.Tags","value":4}]"#),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    let error: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(error["message"], "étiquettes illisibles");
    let (status, body, _) = call(
        &app,
        "PATCH",
        &format!("/azurz/AzurzDemo/_apis/wit/workitems/{id}?api-version=7.1"),
        Some(&auth),
        None,
        "application/json-patch+json",
        Some(r#"[{"op":"replace","path":"/fields/System.AssignedTo","value":{"displayName":"Ligne","uniqueName":"ligne@azurz"}},{"op":"replace","path":"/fields/System.AreaPath","value":"AzurzDemo"}]"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let updated: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(updated["fields"]["System.AreaPath"], "AzurzDemo");
    assert_eq!(updated["fields"]["System.AssignedTo"]["displayName"], "Ligne");
    assert_eq!(updated["fields"]["System.AssignedTo"]["uniqueName"], "ligne@azurz");
    assert_eq!(updated["fields"]["System.IterationPath"], "AzurzDemo\\Sprint 1");
    assert_eq!(updated["fields"]["System.Tags"], "pompe; ligne");
    assert_eq!(updated["rev"], 2);
}

#[tokio::test]
async fn work_item_delete_moves_it_to_the_recycle_bin_and_restore_brings_it_back() {
    let app = app();
    let auth = basic("azurz", "azurz-lab-pat");
    let (status, body, _) = call(
        &app,
        "DELETE",
        "/azurz/AzurzDemo/_apis/wit/workitems/4?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    let error: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(error["message"], "work item introuvable");

    let (status, body, _) = call(
        &app,
        "POST",
        "/azurz/AzurzDemo/_apis/wit/workitems/$User%20Story?api-version=7.1",
        Some(&auth),
        None,
        "application/json-patch+json",
        Some(r#"[{"op":"add","path":"/fields/System.Title","value":"Pompe"}]"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let created: serde_json::Value = serde_json::from_str(&body).unwrap();
    let id = created["id"].as_i64().unwrap();
    let (status, body, _) = call(
        &app,
        "POST",
        "/azurz/AzurzDemo/_apis/wit/workitems/$Task?api-version=7.1",
        Some(&auth),
        None,
        "application/json-patch+json",
        Some(r#"[{"op":"add","path":"/fields/System.Title","value":"Voisine"}]"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let neighbor: serde_json::Value = serde_json::from_str(&body).unwrap();
    let neighbor_id = neighbor["id"].as_i64().unwrap();

    let (status, body, _) = call(
        &app,
        "POST",
        "/azurz/_apis/hooks/subscriptions?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"publisherId":"tfs","eventType":"workitem.deleted","consumerId":"webHooks","consumerActionId":"httpRequest","consumerInputs":{"url":"/_hooks/inbox"},"publisherInputs":{"projectId":"AzurzDemo"}}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status, body, _) = call(
        &app,
        "POST",
        "/azurz/_apis/hooks/subscriptions?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"publisherId":"tfs","eventType":"workitem.restored","consumerId":"webHooks","consumerActionId":"httpRequest","consumerInputs":{"url":"/_hooks/inbox"},"publisherInputs":{"projectId":"AzurzDemo"}}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");

    let (status, body, _) = call(
        &app,
        "DELETE",
        &format!("/azurz/AzurzDemo/_apis/wit/workitems/{id}?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let deleted: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(deleted["name"], "Pompe");
    assert_eq!(deleted["type"], "User Story");
    assert_eq!(deleted["resource"]["id"], id);
    assert!(deleted["resource"].get("fields").is_some());
    let (status, body, _) = call(
        &app,
        "GET",
        &format!("/azurz/AzurzDemo/_apis/wit/workitems/{id}?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    let (status, body, _) = call(
        &app,
        "GET",
        &format!("/azurz/AzurzDemo/_apis/wit/workitems/{neighbor_id}?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let kept: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(kept["fields"]["System.Title"], "Voisine");

    let (status, body, _) = call(
        &app,
        "PATCH",
        &format!("/azurz/AzurzDemo/_apis/wit/recyclebin/{id}?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"isDeleted":true}"#),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    let error: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(error["message"], "restauration refusée");
    let (status, body, _) = call(
        &app,
        "PATCH",
        &format!("/azurz/AzurzDemo/_apis/wit/recyclebin/{id}?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some("[]"),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    let error: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(error["message"], "restauration illisible");
    let (status, body, _) = call(
        &app,
        "GET",
        "/azurz/AzurzDemo/_apis/wit/recyclebin?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let bin: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(bin["count"], 1);
    assert_eq!(bin["value"][0]["name"], "Pompe");
    assert!(bin["value"][0].get("resource").is_none(), "{body}");

    let (status, body, _) = call(
        &app,
        "PATCH",
        &format!("/azurz/AzurzDemo/_apis/wit/recyclebin/{id}?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"isDeleted":false}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let restored: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(restored["id"], id);
    assert_eq!(restored["fields"]["System.Title"], "Pompe");
    assert_eq!(restored["rev"], 1);
    let (status, body, _) = call(
        &app,
        "GET",
        "/azurz/AzurzDemo/_apis/wit/recyclebin?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let empty: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(empty["count"], 0);

    let (status, body, _) = call(
        &app,
        "DELETE",
        &format!("/azurz/AzurzDemo/_apis/wit/workitems/{id}?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status, body, _) = call(
        &app,
        "DELETE",
        &format!("/azurz/AzurzDemo/_apis/wit/recyclebin/{id}?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT, "{body}");
    let (status, body, _) = call(
        &app,
        "PATCH",
        &format!("/azurz/AzurzDemo/_apis/wit/recyclebin/{id}?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"isDeleted":false}"#),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    let error: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(error["message"], "élément supprimé introuvable");

    let (status, body, _) = call(
        &app,
        "POST",
        "/azurz/AzurzDemo/_apis/wit/workitems/$Bug?api-version=7.1",
        Some(&auth),
        None,
        "application/json-patch+json",
        Some(r#"[{"op":"add","path":"/fields/System.Title","value":"Éphémère"}]"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let gone: serde_json::Value = serde_json::from_str(&body).unwrap();
    let gone_id = gone["id"].as_i64().unwrap();
    let (status, body, _) = call(
        &app,
        "DELETE",
        &format!("/azurz/AzurzDemo/_apis/wit/workitems/{gone_id}?destroy=true&api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status, body, _) = call(
        &app,
        "GET",
        "/azurz/AzurzDemo/_apis/wit/recyclebin?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let bin: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(bin["count"], 0, "{body}");
    let (status, body, _) = call(
        &app,
        "GET",
        "/azurz/_apis/hooks/inbox?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let inbox: serde_json::Value = serde_json::from_str(&body).unwrap();
    let events: Vec<&str> = inbox["value"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|item| item.get("eventType").and_then(|value| value.as_str()))
        .collect();
    assert!(events.contains(&"workitem.deleted"), "{body}");
    assert!(events.contains(&"workitem.restored"), "{body}");
    let titles: Vec<&str> = inbox["value"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|item| item.get("resource").and_then(|resource| resource.get("fields")).and_then(|fields| fields.get("System.Title")).and_then(|value| value.as_str()))
        .collect();
    assert!(titles.contains(&"Pompe"), "{body}");
}

#[tokio::test]
async fn work_item_comment_can_be_revised_and_removed() {
    let app = app();
    let auth = basic("azurz", "azurz-lab-pat");
    let (status, body, _) = call(
        &app,
        "POST",
        "/azurz/AzurzDemo/_apis/wit/workItems/40/comments?api-version=7.1-preview.3",
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"text":"absent"}"#),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    let error: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(error["message"], "work item introuvable");

    let (status, body, _) = call(
        &app,
        "POST",
        "/azurz/AzurzDemo/_apis/wit/workitems/$User%20Story?api-version=7.1",
        Some(&auth),
        None,
        "application/json-patch+json",
        Some(r#"[{"op":"add","path":"/fields/System.Title","value":"Pompe"}]"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let created: serde_json::Value = serde_json::from_str(&body).unwrap();
    let id = created["id"].as_i64().unwrap();
    let rev = created["rev"].as_i64().unwrap();

    let (status, body, _) = call(
        &app,
        "POST",
        &format!("/azurz/AzurzDemo/_apis/wit/workItems/{id}/comments?api-version=7.1-preview.3"),
        Some(&auth),
        None,
        "application/json",
        Some("[]"),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    let error: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(error["message"], "commentaire illisible");
    let (status, body, _) = call(
        &app,
        "POST",
        &format!("/azurz/AzurzDemo/_apis/wit/workItems/{id}/comments?api-version=7.1-preview.3"),
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"text":" "}"#),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    let error: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(error["message"], "commentaire absent");
    let (status, body, _) = call(
        &app,
        "GET",
        &format!("/azurz/AzurzDemo/_apis/wit/workItems/{id}/comments?api-version=7.1-preview.3"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let empty: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(empty["count"], 0);
    assert_eq!(empty["totalCount"], 0);

    let (status, body, _) = call(
        &app,
        "POST",
        &format!("/azurz/AzurzDemo/_apis/wit/workItems/{id}/comments?api-version=7.1-preview.3"),
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"text":"Pompe à réviser"}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let first: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(first["id"], 1);
    assert_eq!(first["version"], 1);
    assert_eq!(first["text"], "Pompe à réviser");
    let (status, body, _) = call(
        &app,
        "POST",
        &format!("/azurz/AzurzDemo/_apis/wit/workItems/{id}/comments?api-version=7.1-preview.3"),
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"text":"Voisin"}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let second: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(second["id"], 2);

    let (status, body, _) = call(
        &app,
        "PATCH",
        &format!("/azurz/AzurzDemo/_apis/wit/workItems/{id}/comments/1?api-version=7.1-preview.3"),
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"text":" "}"#),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    let error: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(error["message"], "commentaire absent");
    let (status, body, _) = call(
        &app,
        "GET",
        &format!("/azurz/AzurzDemo/_apis/wit/workItems/{id}/comments/1?api-version=7.1-preview.3"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let unchanged: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(unchanged["text"], "Pompe à réviser");
    assert_eq!(unchanged["version"], 1);

    let (status, body, _) = call(
        &app,
        "PATCH",
        &format!("/azurz/AzurzDemo/_apis/wit/workItems/{id}/comments/9?api-version=7.1-preview.3"),
        Some(&auth),
        None,
        "application/json",
        Some("[]"),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    let error: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(error["message"], "commentaire introuvable");

    let (status, body, _) = call(
        &app,
        "PATCH",
        &format!("/azurz/AzurzDemo/_apis/wit/workItems/{id}/comments/2?api-version=7.1-preview.3"),
        Some(&auth),
        None,
        "application/json",
        Some("[]"),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    let error: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(error["message"], "commentaire illisible");
    let (status, body, _) = call(
        &app,
        "GET",
        &format!("/azurz/AzurzDemo/_apis/wit/workItems/{id}/comments/2?api-version=7.1-preview.3"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let neighbor: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(neighbor["text"], "Voisin");

    let (status, body, _) = call(
        &app,
        "PATCH",
        &format!("/azurz/AzurzDemo/_apis/wit/workItems/{id}/comments/1?api-version=7.1-preview.3"),
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"text":"Pompe révisée"}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let revised: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(revised["text"], "Pompe révisée");
    assert_eq!(revised["version"], 2);
    assert_eq!(revised["modifiedBy"]["displayName"], "azurz");
    assert_eq!(revised["modifiedBy"]["uniqueName"], "azurz");
    let (status, body, _) = call(
        &app,
        "GET",
        &format!("/azurz/AzurzDemo/_apis/wit/workItems/{id}/comments?api-version=7.1-preview.3"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let listed: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(listed["count"], 2);
    assert_eq!(listed["comments"][1]["text"], "Voisin");
    assert_eq!(listed["comments"][1]["version"], 1);

    let (status, body, _) = call(
        &app,
        "DELETE",
        &format!("/azurz/AzurzDemo/_apis/wit/workItems/{id}/comments/1?api-version=7.1-preview.3"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT, "{body}");
    let (status, body, _) = call(
        &app,
        "GET",
        &format!("/azurz/AzurzDemo/_apis/wit/workItems/{id}/comments?api-version=7.1-preview.3"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let left: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(left["count"], 1);
    assert_eq!(left["comments"][0]["text"], "Voisin");
    assert_eq!(left["comments"][0]["id"], 2);
    let (status, body, _) = call(
        &app,
        "PATCH",
        &format!("/azurz/AzurzDemo/_apis/wit/workItems/{id}/comments/1?api-version=7.1-preview.3"),
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"text":"encore"}"#),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    let error: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(error["message"], "commentaire introuvable");
    let (status, body, _) = call(
        &app,
        "DELETE",
        &format!("/azurz/AzurzDemo/_apis/wit/workItems/{id}/comments/1?api-version=7.1-preview.3"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    let error: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(error["message"], "commentaire introuvable");

    let (status, body, _) = call(
        &app,
        "GET",
        &format!("/azurz/AzurzDemo/_apis/wit/workitems/{id}?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let item: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(item["rev"], rev);
    assert_eq!(item["fields"]["System.Title"], "Pompe");

    let (status, body, _) = call(
        &app,
        "POST",
        "/azurz/_apis/hooks/subscriptions?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"publisherId":"tfs","eventType":"workitem.commented","consumerId":"webHooks","consumerActionId":"httpRequest","consumerInputs":{"url":"/_hooks/inbox"},"publisherInputs":{"projectId":"AzurzDemo"}}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status, body, _) = call(
        &app,
        "POST",
        &format!("/azurz/AzurzDemo/_apis/wit/workItems/{id}/comments?api-version=7.1-preview.3"),
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"text":"Pompe signalée"}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let again: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(again["id"], 3);
    let (status, body, _) = call(
        &app,
        "GET",
        "/azurz/_apis/hooks/inbox?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let inbox: serde_json::Value = serde_json::from_str(&body).unwrap();
    let texts: Vec<&str> = inbox["value"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|item| item.get("resource").and_then(|resource| resource.get("text")).and_then(|value| value.as_str()))
        .collect();
    assert!(texts.contains(&"Pompe signalée"), "{body}");
}

#[tokio::test]
async fn pull_request_thread_can_be_resolved_and_its_comment_revised() {
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
    let missing = format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/pullrequests/40/threads/1?api-version=7.1");
    let (status, body, _) = call(&app, "PATCH", &missing, Some(&auth), None, "application/json", Some(r#"{"status":"fixed"}"#)).await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    let error: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(error["message"], "demande de tirage introuvable");

    let (status, body, _) = call(
        &app,
        "POST",
        &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/pullrequests?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"title":"Pompe","sourceRefName":"refs/heads/feature","targetRefName":"refs/heads/main"}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let pull: serde_json::Value = serde_json::from_str(&body).unwrap();
    let pull_id = pull["pullRequestId"].as_i64().unwrap();
    let threads = format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/pullrequests/{pull_id}/threads");

    let (status, body, _) = call(
        &app,
        "POST",
        &format!("{threads}?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some("[]"),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    let error: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(error["message"], "fil illisible");
    let (status, body, _) = call(
        &app,
        "POST",
        &format!("{threads}?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"comments":[{"content":" "}],"status":"active"}"#),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    let error: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(error["message"], "commentaire absent");
    let (status, body, _) = call(&app, "GET", &format!("{threads}?api-version=7.1"), Some(&auth), None, "application/json", None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let empty: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(empty["count"], 0);

    let (status, body, _) = call(
        &app,
        "POST",
        &format!("{threads}?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"comments":[{"content":"Pompe à revoir"}],"status":"active"}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let thread: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(thread["id"], 1);
    assert_eq!(thread["status"], "active");
    assert_eq!(thread["comments"][0]["content"], "Pompe à revoir");
    assert_eq!(thread["comments"][0]["version"], 1);
    let (status, body, _) = call(
        &app,
        "POST",
        &format!("{threads}/1/comments?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"content":"Voisin"}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let neighbor: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(neighbor["id"], 2);

    let (status, body, _) = call(
        &app,
        "PATCH",
        &format!("{threads}/9?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some("[]"),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    let error: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(error["message"], "fil introuvable");
    let (status, body, _) = call(
        &app,
        "PATCH",
        &format!("{threads}/1?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"status":" "}"#),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    let error: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(error["message"], "statut de fil absent");
    let (status, body, _) = call(
        &app,
        "PATCH",
        &format!("{threads}/1?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"status":"inconnu"}"#),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    let error: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(error["message"], "statut de fil inconnu");
    let (status, body, _) = call(&app, "GET", &format!("{threads}/1?api-version=7.1"), Some(&auth), None, "application/json", None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let unchanged: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(unchanged["status"], "active");
    assert_eq!(unchanged["comments"][0]["content"], "Pompe à revoir");

    let (status, body, _) = call(
        &app,
        "PATCH",
        &format!("{threads}/1?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"status":"fixed"}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let resolved: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(resolved["status"], "fixed");
    assert_eq!(resolved["comments"][1]["content"], "Voisin");
    assert_eq!(resolved["comments"][1]["version"], 1);

    let (status, body, _) = call(
        &app,
        "PATCH",
        &format!("{threads}/1/comments/1?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"content":" "}"#),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    let error: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(error["message"], "commentaire absent");
    let (status, body, _) = call(
        &app,
        "PATCH",
        &format!("{threads}/1/comments/9?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some("[]"),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    let error: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(error["message"], "commentaire introuvable");
    let (status, body, _) = call(
        &app,
        "PATCH",
        &format!("{threads}/1/comments/2?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some("[]"),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    let error: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(error["message"], "commentaire illisible");
    let (status, body, _) = call(
        &app,
        "PATCH",
        &format!("{threads}/1/comments/1?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"content":"Pompe revue"}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let revised: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(revised["content"], "Pompe revue");
    assert_eq!(revised["version"], 2);
    assert_eq!(revised["modifiedBy"]["displayName"], "azurz");
    let (status, body, _) = call(&app, "GET", &format!("{threads}/1/comments/2?api-version=7.1"), Some(&auth), None, "application/json", None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let kept: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(kept["content"], "Voisin");
    assert_eq!(kept["version"], 1);

    let (status, body, _) = call(&app, "DELETE", &format!("{threads}/1/comments/1?api-version=7.1"), Some(&auth), None, "application/json", None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let deleted: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(deleted["isDeleted"], true);
    let (status, body, _) = call(&app, "GET", &format!("{threads}/1?api-version=7.1"), Some(&auth), None, "application/json", None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let left: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(left["status"], "fixed");
    assert_eq!(left["comments"].as_array().unwrap().len(), 1);
    assert_eq!(left["comments"][0]["id"], 2);
    assert_eq!(left["comments"][0]["content"], "Voisin");
    let (status, body, _) = call(
        &app,
        "PATCH",
        &format!("{threads}/1/comments/1?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"content":"encore"}"#),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    let error: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(error["message"], "commentaire introuvable");

    let (status, body, _) = call(
        &app,
        "GET",
        &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/pullrequests/{pull_id}?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let still: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(still["status"], "active");
    assert_eq!(still["title"], "Pompe");

    let (status, body, _) = call(
        &app,
        "POST",
        "/azurz/_apis/hooks/subscriptions?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"publisherId":"tfs","eventType":"git.pullrequest.commented","consumerId":"webHooks","consumerActionId":"httpRequest","consumerInputs":{"url":"/_hooks/inbox"},"publisherInputs":{"projectId":"AzurzDemo"}}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status, body, _) = call(
        &app,
        "POST",
        &format!("{threads}/1/comments?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"content":"Pompe signalée"}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let again: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(again["id"], 3);
    let (status, body, _) = call(&app, "GET", "/azurz/_apis/hooks/inbox?api-version=7.1", Some(&auth), None, "application/json", None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let inbox: serde_json::Value = serde_json::from_str(&body).unwrap();
    let texts: Vec<&str> = inbox["value"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|item| item.get("resource").and_then(|resource| resource.get("content")).and_then(|value| value.as_str()))
        .collect();
    assert!(texts.contains(&"Pompe signalée"), "{body}");
}

#[tokio::test]
async fn wiki_page_moves_with_its_children_and_can_be_deleted() {
    let app = app();
    let auth = basic("azurz", "azurz-lab-pat");
    let (status, body, _) = call(
        &app,
        "POST",
        "/azurz/AzurzDemo/_apis/wiki/wikis/manuel-absent/pagemoves?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        Some("[]"),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    let error: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(error["message"], "wiki introuvable");

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
    let wiki_id = wiki["id"].as_str().unwrap();
    let pages = format!("/azurz/AzurzDemo/_apis/wiki/wikis/{wiki_id}/pages");
    let moves = format!("/azurz/AzurzDemo/_apis/wiki/wikis/{wiki_id}/pagemoves?api-version=7.1");

    let (status, body, _) = call(&app, "POST", &moves, Some(&auth), None, "application/json", Some("[]")).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    let error: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(error["message"], "déplacement illisible");
    let (status, body, _) = call(
        &app,
        "POST",
        &moves,
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"path":"  ","newPath":"/Atelier"}"#),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    let error: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(error["message"], "chemin de page absent");
    let (status, body, _) = call(
        &app,
        "POST",
        &moves,
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"path":"/Inconnue","newPath":"/Atelier","newOrder":"haut"}"#),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    let error: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(error["message"], "ordre illisible");

    for (path, content) in [("/Pompe", "Pompe racine"), ("/Pompe/ligne", "ligne de pompe"), ("/Pompe/ligne/bas", "fond"), ("/Autre", "voisin")] {
        let (status, body, _) = call(
            &app,
            "PUT",
            &format!("{pages}?path={path}&api-version=7.1"),
            Some(&auth),
            None,
            "application/json",
            Some(&format!(r#"{{"content":"{content}"}}"#)),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{body}");
    }

    let (status, body, _) = call(
        &app,
        "POST",
        &moves,
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"path":"/Absente","newPath":"/Atelier"}"#),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    let error: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(error["message"], "page introuvable");
    let (status, body, _) = call(
        &app,
        "POST",
        &moves,
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"path":"/Pompe","newPath":"/Pompe/cave"}"#),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    let error: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(error["message"], "déplacement refusé");
    let (status, body, _) = call(
        &app,
        "POST",
        &moves,
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"path":"/Pompe","newPath":"/Autre"}"#),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    let error: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(error["message"], "page déjà présente");
    let (status, body, _) = call(&app, "GET", &format!("{pages}?path=/Pompe&api-version=7.1"), Some(&auth), None, "application/json", None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let kept: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(kept["content"], "Pompe racine");
    let (status, body, _) = call(&app, "GET", &format!("{pages}?path=/Autre&api-version=7.1"), Some(&auth), None, "application/json", None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let neighbor: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(neighbor["content"], "voisin");

    let (status, body, _) = call(
        &app,
        "POST",
        &moves,
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"path":"/Pompe","newPath":"/Atelier","newOrder":2}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let moved: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(moved["path"], "/Atelier");
    assert_eq!(moved["content"], "Pompe racine");
    assert_eq!(moved["newOrder"], 2);
    let (status, body, _) = call(&app, "GET", &format!("{pages}?path=/Pompe&api-version=7.1"), Some(&auth), None, "application/json", None).await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    let (status, body, _) = call(&app, "GET", &format!("{pages}?path=/Atelier/ligne&api-version=7.1"), Some(&auth), None, "application/json", None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let child: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(child["content"], "ligne de pompe");
    assert_eq!(child["path"], "/Atelier/ligne");
    let (status, body, _) = call(&app, "GET", &format!("{pages}?path=/Atelier/ligne/bas&api-version=7.1"), Some(&auth), None, "application/json", None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let nested: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(nested["content"], "fond");
    let (status, body, _) = call(&app, "GET", &format!("{pages}?path=/Autre&api-version=7.1"), Some(&auth), None, "application/json", None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let neighbor: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(neighbor["content"], "voisin");

    let (status, body, _) = call(&app, "DELETE", &format!("{pages}?path=/Atelier&api-version=7.1"), Some(&auth), None, "application/json", None).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    let error: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(error["message"], "sous-pages présentes");
    let (status, body, _) = call(
        &app,
        "DELETE",
        &format!("{pages}?path=/Atelier&recursionLevel=oneLevel&api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    let error: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(error["message"], "sous-pages présentes");
    let (status, body, _) = call(
        &app,
        "DELETE",
        &format!("{pages}?path=/Atelier/ligne/bas&recursionLevel=demi&api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    let error: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(error["message"], "profondeur inconnue");
    let (status, body, _) = call(&app, "GET", &format!("{pages}?path=/Atelier/ligne/bas&api-version=7.1"), Some(&auth), None, "application/json", None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let still: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(still["content"], "fond");

    let (status, body, _) = call(&app, "DELETE", &format!("{pages}?path=/Atelier/ligne/bas&api-version=7.1"), Some(&auth), None, "application/json", None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let dropped: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(dropped["path"], "/Atelier/ligne/bas");
    let (status, body, _) = call(
        &app,
        "DELETE",
        &format!("{pages}?path=/Atelier/ligne&recursionLevel=oneLevel&api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status, body, _) = call(&app, "GET", &format!("{pages}?path=/Atelier/ligne&api-version=7.1"), Some(&auth), None, "application/json", None).await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    let (status, body, _) = call(
        &app,
        "PUT",
        &format!("{pages}?path=/Atelier/ligne&api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"content":"ligne de pompe"}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status, body, _) = call(
        &app,
        "DELETE",
        &format!("{pages}?path=/Atelier&recursionLevel=full&api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let deleted: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(deleted["path"], "/Atelier");
    assert_eq!(deleted["content"], "Pompe racine");
    let (status, body, _) = call(&app, "GET", &format!("{pages}?path=/Atelier/ligne&api-version=7.1"), Some(&auth), None, "application/json", None).await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    let (status, body, _) = call(&app, "GET", &format!("{pages}?path=/Autre&api-version=7.1"), Some(&auth), None, "application/json", None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let neighbor: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(neighbor["content"], "voisin");
    let (status, body, _) = call(
        &app,
        "DELETE",
        &format!("{pages}?path=/Atelier&recursionLevel=full&api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    let error: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(error["message"], "page introuvable");

    let (status, body, _) = call(
        &app,
        "POST",
        "/azurz/_apis/hooks/subscriptions?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"publisherId":"tfs","eventType":"wiki.page.moved","consumerId":"webHooks","consumerActionId":"httpRequest","consumerInputs":{"url":"/_hooks/inbox"},"publisherInputs":{"projectId":"AzurzDemo"}}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status, body, _) = call(
        &app,
        "PUT",
        &format!("{pages}?path=/Signal&api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"content":"page signalée"}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status, body, _) = call(
        &app,
        "POST",
        &moves,
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"path":"/Signal","newPath":"/Signal"}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status, body, _) = call(&app, "GET", "/azurz/_apis/hooks/inbox?api-version=7.1", Some(&auth), None, "application/json", None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let inbox: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(inbox["count"], 0);
    let (status, body, _) = call(
        &app,
        "POST",
        &moves,
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"path":"/Signal","newPath":"/Alerte"}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status, body, _) = call(&app, "GET", "/azurz/_apis/hooks/inbox?api-version=7.1", Some(&auth), None, "application/json", None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let inbox: serde_json::Value = serde_json::from_str(&body).unwrap();
    let moved_note = inbox["value"].as_array().unwrap().iter().find(|item| item["eventType"] == "wiki.page.moved").unwrap();
    assert_eq!(moved_note["resource"]["path"], "/Alerte");
    assert_eq!(moved_note["resource"]["content"], "page signalée");

    let (status, body, _) = call(
        &app,
        "POST",
        "/azurz/_apis/hooks/subscriptions?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"publisherId":"tfs","eventType":"wiki.page.deleted","consumerId":"webHooks","consumerActionId":"httpRequest","consumerInputs":{"url":"/_hooks/inbox"},"publisherInputs":{"projectId":"AzurzDemo"}}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status, body, _) = call(&app, "DELETE", &format!("{pages}?path=/Alerte&api-version=7.1"), Some(&auth), None, "application/json", None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status, body, _) = call(&app, "GET", "/azurz/_apis/hooks/inbox?api-version=7.1", Some(&auth), None, "application/json", None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let inbox: serde_json::Value = serde_json::from_str(&body).unwrap();
    let deleted_note = inbox["value"].as_array().unwrap().iter().find(|item| item["eventType"] == "wiki.page.deleted").unwrap();
    assert_eq!(deleted_note["resource"]["path"], "/Alerte");
    assert_eq!(deleted_note["resource"]["content"], "page signalée");
}

#[tokio::test]
async fn test_suite_can_be_renamed_and_removed_with_its_cases() {
    let app = app();
    let auth = basic("azurz", "azurz-lab-pat");
    let (status, body, _) = call(
        &app,
        "PATCH",
        "/azurz/AzurzDemo/_apis/testplan/plans/99/suites/1?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        Some("[]"),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    let error: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(error["message"], "plan introuvable");

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
    let suites = format!("/azurz/AzurzDemo/_apis/testplan/plans/{plan_id}/suites");

    let (status, body, _) = call(&app, "PATCH", &format!("{suites}/{root_id}?api-version=7.1"), Some(&auth), None, "application/json", Some("[]")).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    let error: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(error["message"], "suite illisible");
    let (status, body, _) = call(
        &app,
        "PATCH",
        &format!("{suites}/{root_id}?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"name":" "}"#),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    let error: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(error["message"], "nom de suite absent");
    let (status, body, _) = call(&app, "GET", &format!("{suites}/{root_id}?api-version=7.1"), Some(&auth), None, "application/json", None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let root: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(root["name"], "Recette pompe");

    let (status, body, _) = call(
        &app,
        "PATCH",
        &format!("{suites}/{root_id}?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"name":"Recette atelier"}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let renamed_root: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(renamed_root["name"], "Recette atelier");
    let (status, body, _) = call(
        &app,
        "GET",
        &format!("/azurz/AzurzDemo/_apis/testplan/plans/{plan_id}?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let plan: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(plan["name"], "Recette pompe");

    let (status, body, _) = call(&app, "DELETE", &format!("{suites}/{root_id}?api-version=7.1"), Some(&auth), None, "application/json", None).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    let error: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(error["message"], "suite racine");
    let (status, body, _) = call(&app, "GET", &format!("{suites}/{root_id}?api-version=7.1"), Some(&auth), None, "application/json", None).await;
    assert_eq!(status, StatusCode::OK, "{body}");

    let (status, body, _) = call(
        &app,
        "POST",
        &format!("{suites}?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some(&format!(r#"{{"name":"Ligne","parentSuite":{{"id":{root_id}}}}}"#)),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let ligne: serde_json::Value = serde_json::from_str(&body).unwrap();
    let ligne_id = ligne["id"].as_i64().unwrap();
    let (status, body, _) = call(
        &app,
        "POST",
        &format!("{suites}?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some(&format!(r#"{{"name":"Cave","parentSuite":{{"id":{ligne_id}}}}}"#)),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let cave: serde_json::Value = serde_json::from_str(&body).unwrap();
    let cave_id = cave["id"].as_i64().unwrap();
    let (status, body, _) = call(
        &app,
        "POST",
        &format!("{suites}?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some(&format!(r#"{{"name":"Pompe","parentSuite":{{"id":{root_id}}}}}"#)),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let pompe: serde_json::Value = serde_json::from_str(&body).unwrap();
    let pompe_id = pompe["id"].as_i64().unwrap();

    let (status, body, _) = call(
        &app,
        "POST",
        &format!("{suites}/{cave_id}/testcases?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some(r#"[{"title":"Debit"}]"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let debit: serde_json::Value = serde_json::from_str(&body).unwrap();
    let debit_id = debit["value"][0]["id"].as_i64().unwrap();
    let (status, body, _) = call(
        &app,
        "POST",
        &format!("{suites}/{ligne_id}/testcases?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some(r#"[{"title":"Voisin"}]"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let voisin: serde_json::Value = serde_json::from_str(&body).unwrap();
    let voisin_id = voisin["value"][0]["id"].as_i64().unwrap();
    let (status, body, _) = call(
        &app,
        "POST",
        &format!("{suites}/{pompe_id}/testcases?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some(r#"[{"title":"Stable"}]"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");

    let (status, body, _) = call(
        &app,
        "DELETE",
        &format!("{suites}/{ligne_id}/testcases/999?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    let error: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(error["message"], "cas introuvable");
    let (status, body, _) = call(
        &app,
        "GET",
        &format!("{suites}/{ligne_id}/testpoint?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let points: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(points["count"], 1);
    assert_eq!(points["value"][0]["testCase"]["name"], "Voisin");

    let (status, body, _) = call(&app, "PATCH", &format!("{suites}/80?api-version=7.1"), Some(&auth), None, "application/json", Some("[]")).await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    let error: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(error["message"], "suite introuvable");
    let (status, body, _) = call(&app, "PATCH", &format!("{suites}/{ligne_id}?api-version=7.1"), Some(&auth), None, "application/json", Some("[]")).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    let error: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(error["message"], "suite illisible");
    let (status, body, _) = call(&app, "GET", &format!("{suites}/{cave_id}?api-version=7.1"), Some(&auth), None, "application/json", None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let cave: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(cave["name"], "Cave");

    let (status, body, _) = call(
        &app,
        "PATCH",
        &format!("{suites}/{ligne_id}?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"name":"Atelier"}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let renamed: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(renamed["name"], "Atelier");
    assert_eq!(renamed["parentSuite"]["id"], root_id);

    let (status, body, _) = call(
        &app,
        "DELETE",
        &format!("{suites}/{ligne_id}/testcases/{voisin_id}?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT, "{body}");
    let (status, body, _) = call(
        &app,
        "GET",
        &format!("/azurz/AzurzDemo/_apis/wit/workitems/{voisin_id}?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let work: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(work["fields"]["System.Title"], "Voisin");
    let (status, body, _) = call(&app, "GET", &format!("{suites}/{ligne_id}/testpoint?api-version=7.1"), Some(&auth), None, "application/json", None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let points: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(points["count"], 0);
    let (status, body, _) = call(&app, "GET", &format!("{suites}/{cave_id}/testpoint?api-version=7.1"), Some(&auth), None, "application/json", None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let points: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(points["value"][0]["testCase"]["name"], "Debit");
    let (status, body, _) = call(&app, "GET", &format!("{suites}/{pompe_id}/testpoint?api-version=7.1"), Some(&auth), None, "application/json", None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let points: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(points["value"][0]["testCase"]["name"], "Stable");

    let (status, body, _) = call(
        &app,
        "POST",
        "/azurz/_apis/hooks/subscriptions?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"publisherId":"tfs","eventType":"test.suite.deleted","consumerId":"webHooks","consumerActionId":"httpRequest","consumerInputs":{"url":"/_hooks/inbox"},"publisherInputs":{"projectId":"AzurzDemo"}}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status, body, _) = call(&app, "DELETE", &format!("{suites}/{ligne_id}?api-version=7.1"), Some(&auth), None, "application/json", None).await;
    assert_eq!(status, StatusCode::NO_CONTENT, "{body}");
    let (status, body, _) = call(&app, "GET", &format!("{suites}/{cave_id}?api-version=7.1"), Some(&auth), None, "application/json", None).await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    let (status, body, _) = call(&app, "GET", &format!("{suites}/{cave_id}/testpoint?api-version=7.1"), Some(&auth), None, "application/json", None).await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    let (status, body, _) = call(
        &app,
        "GET",
        &format!("/azurz/AzurzDemo/_apis/wit/workitems/{debit_id}?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let work: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(work["fields"]["System.Title"], "Debit");
    let (status, body, _) = call(&app, "GET", &format!("{suites}/{pompe_id}?api-version=7.1"), Some(&auth), None, "application/json", None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let pompe: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(pompe["name"], "Pompe");
    let (status, body, _) = call(&app, "DELETE", &format!("{suites}/{ligne_id}?api-version=7.1"), Some(&auth), None, "application/json", None).await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    let error: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(error["message"], "suite introuvable");

    let (status, body, _) = call(
        &app,
        "POST",
        &format!("{suites}?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"name":"Suite suivante"}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let following: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_ne!(following["id"].as_i64().unwrap(), ligne_id);
    assert_ne!(following["id"].as_i64().unwrap(), cave_id);

    let (status, body, _) = call(&app, "GET", "/azurz/_apis/hooks/inbox?api-version=7.1", Some(&auth), None, "application/json", None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let inbox: serde_json::Value = serde_json::from_str(&body).unwrap();
    let deleted_note = inbox["value"].as_array().unwrap().iter().find(|item| item["eventType"] == "test.suite.deleted").unwrap();
    assert_eq!(deleted_note["resource"]["name"], "Atelier");
}

#[tokio::test]
async fn release_can_be_abandoned() {
    let app = app();
    let auth = basic("azurz", "azurz-lab-pat");
    let (status, body, _) = call(
        &app,
        "PATCH",
        "/azurz/AzurzDemo/_apis/release/releases/99?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        Some("[]"),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    let error: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(error["message"], "version introuvable");

    let (status, body, _) = call(
        &app,
        "POST",
        "/azurz/AzurzDemo/_apis/release/definitions?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"name":"Deploiement","environments":[{"name":"Production","rank":1,"preDeployApprovals":{"approvals":[{"approver":{"displayName":"azurz"}}]}}]}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let definition: serde_json::Value = serde_json::from_str(&body).unwrap();
    let definition_id = definition["id"].as_i64().unwrap();
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
    assert_eq!(release["status"], "active");
    assert_eq!(release["environments"][0]["status"], "inProgress");
    let (status, body, _) = call(
        &app,
        "POST",
        "/azurz/AzurzDemo/_apis/release/releases?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        Some(&format!(r#"{{"definitionId":{definition_id},"description":"stable"}}"#)),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let sibling: serde_json::Value = serde_json::from_str(&body).unwrap();
    let sibling_id = sibling["id"].as_i64().unwrap();

    let (status, body, _) = call(
        &app,
        "PATCH",
        &format!("/azurz/AzurzDemo/_apis/release/releases/{release_id}?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some("[]"),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    let error: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(error["message"], "version illisible");
    let (status, body, _) = call(
        &app,
        "PATCH",
        &format!("/azurz/AzurzDemo/_apis/release/releases/{release_id}?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"status":" "}"#),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    let error: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(error["message"], "statut de version absent");
    let (status, body, _) = call(
        &app,
        "PATCH",
        &format!("/azurz/AzurzDemo/_apis/release/releases/{release_id}?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"status":1}"#),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    let error: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(error["message"], "version illisible");
    let (status, body, _) = call(
        &app,
        "PATCH",
        &format!("/azurz/AzurzDemo/_apis/release/releases/{release_id}?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"status":"archive"}"#),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    let error: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(error["message"], "statut de version inconnu");
    let (status, body, _) = call(
        &app,
        "PATCH",
        &format!("/azurz/AzurzDemo/_apis/release/releases/{release_id}?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"status":"abandoned","description":2}"#),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    let error: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(error["message"], "description illisible");
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
    let unchanged: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(unchanged["status"], "active");
    assert_eq!(unchanged["description"], "1.0.0");
    assert_eq!(unchanged["environments"][0]["status"], "inProgress");

    let (status, body, _) = call(
        &app,
        "POST",
        "/azurz/_apis/hooks/subscriptions?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"publisherId":"tfs","eventType":"release.abandoned","consumerId":"webHooks","consumerActionId":"httpRequest","consumerInputs":{"url":"/_hooks/inbox"},"publisherInputs":{"projectId":"AzurzDemo"}}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status, body, _) = call(
        &app,
        "PATCH",
        &format!("/azurz/AzurzDemo/_apis/release/releases/{release_id}?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"status":"Abandoned"}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let abandoned: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(abandoned["status"], "abandoned");
    assert_eq!(abandoned["description"], "1.0.0");
    assert_eq!(abandoned["environments"][0]["status"], "canceled");
    assert_eq!(abandoned["name"], release["name"]);

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
    let rows = approvals["value"].as_array().unwrap();
    let own = rows.iter().find(|item| item["release"]["id"].as_i64() == Some(release_id)).unwrap();
    let other = rows.iter().find(|item| item["release"]["id"].as_i64() == Some(sibling_id)).unwrap();
    assert_eq!(own["status"], "canceled");
    assert_eq!(other["status"], "pending");

    let (status, body, _) = call(
        &app,
        "GET",
        &format!("/azurz/AzurzDemo/_apis/release/releases/{sibling_id}?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let sibling: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(sibling["status"], "active");
    assert_eq!(sibling["description"], "stable");
    assert_eq!(sibling["environments"][0]["status"], "inProgress");

    let (status, body, _) = call(
        &app,
        "PATCH",
        &format!("/azurz/AzurzDemo/_apis/release/releases/{release_id}?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"status":"abandoned"}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status, body, _) = call(
        &app,
        "PATCH",
        &format!("/azurz/AzurzDemo/_apis/release/releases/{release_id}?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"status":"active"}"#),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    let error: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(error["message"], "version abandonnée");
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
    let still: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(still["status"], "abandoned");
    assert_eq!(still["environments"][0]["status"], "canceled");

    let (status, body, _) = call(&app, "GET", "/azurz/_apis/hooks/inbox?api-version=7.1", Some(&auth), None, "application/json", None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let inbox: serde_json::Value = serde_json::from_str(&body).unwrap();
    let notes: Vec<_> = inbox["value"].as_array().unwrap().iter().filter(|item| item["eventType"] == "release.abandoned").collect();
    assert_eq!(notes.len(), 1);
    assert_eq!(notes[0]["resource"]["name"], release["name"]);
    assert_eq!(notes[0]["resource"]["description"], "1.0.0");
}

#[tokio::test]
async fn git_file_can_be_edited_and_deleted() {
    let app = app();
    let auth = basic("azurz", "azurz-lab-pat");
    let (status, body, _) = call(
        &app,
        "POST",
        "/azurz/AzurzDemo/_apis/git/repositories/00000000-0000-0000-0000-000000000099/pushes?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        Some("[]"),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    let error: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(error["message"], "dépôt introuvable");

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
    let pushes = format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/pushes?api-version=7.1");
    let (status, body, _) = call(&app, "GET", &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/refs?api-version=7.1"), Some(&auth), None, "application/json", None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let refs: serde_json::Value = serde_json::from_str(&body).unwrap();
    let old = refs["value"].as_array().unwrap().iter().find(|item| item["name"] == "refs/heads/main").unwrap()["objectId"].as_str().unwrap().to_string();
    let (status, body, _) = call(&app, "GET", &pushes, Some(&auth), None, "application/json", None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let before: serde_json::Value = serde_json::from_str(&body).unwrap();
    let before_count = before["count"].as_i64().unwrap();

    let refused = [
        (r#"{"refUpdates":[{"name":"refs/heads/main","oldObjectId":"OLD"}],"commits":[{"comment":"retrait","changes":[{"changeType":"delete","item":{"path":"/absente.txt"}}]}]}"#, "fichier introuvable"),
        (r#"{"refUpdates":[{"name":"refs/heads/main","oldObjectId":"OLD"}],"commits":[{"comment":"revue","changes":[{"changeType":"edit","item":{"path":"/absente.txt"},"newContent":{"content":"x","contentType":"rawtext"}}]}]}"#, "fichier introuvable"),
        (r#"{"refUpdates":[{"name":"refs/heads/main","oldObjectId":"OLD"}],"commits":[{"comment":"vide","changes":[{"changeType":"add","item":{"path":"  "}}]}]}"#, "chemin de fichier absent"),
        (r#"{"refUpdates":[{"name":"refs/heads/main","oldObjectId":"OLD"}],"commits":[{"comment":"rename","changes":[{"changeType":"rename","item":{"path":"/README.md"}}]}]}"#, "type de changement inconnu"),
        (r#"{"refUpdates":[{"name":"refs/heads/main","oldObjectId":"OLD"}],"commits":[{"comment":"racine","changes":[{"changeType":"delete","item":{"path":"/"}}]}]}"#, "chemin de fichier refusé"),
    ];
    for (template, message) in refused {
        let payload = template.replace("OLD", &old);
        let (status, body, _) = call(&app, "POST", &pushes, Some(&auth), None, "application/json", Some(&payload)).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
        let error: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(error["message"], message, "{body}");
    }
    let (status, body, _) = call(&app, "GET", &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/refs?api-version=7.1"), Some(&auth), None, "application/json", None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let refs: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(refs["value"].as_array().unwrap().iter().find(|item| item["name"] == "refs/heads/main").unwrap()["objectId"], old);
    let (status, body, _) = call(&app, "GET", &pushes, Some(&auth), None, "application/json", None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let after: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(after["count"], before_count);
    let (status, body, _) = call(&app, "GET", &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/items?path=/README.md&api-version=7.1"), Some(&auth), None, "application/json", None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let readme: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(readme["content"], "# AzurzDemo\n");

    let (status, body, _) = call(
        &app,
        "POST",
        &pushes,
        Some(&auth),
        None,
        "application/json",
        Some(&format!(r#"{{"refUpdates":[{{"name":"refs/heads/main","oldObjectId":"{old}"}}],"commits":[{{"comment":"ajout pompe","changes":[{{"changeType":"add","item":{{"path":"/src/pompe.txt"}},"newContent":{{"content":"debit 12","contentType":"rawtext"}}}}]}}]}}"#)),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let created: serde_json::Value = serde_json::from_str(&body).unwrap();
    let added = created["commits"][0]["commitId"].as_str().unwrap().to_string();
    let (status, body, _) = call(
        &app,
        "POST",
        &pushes,
        Some(&auth),
        None,
        "application/json",
        Some(&format!(r#"{{"refUpdates":[{{"name":"refs/heads/main","oldObjectId":"{added}"}}],"commits":[{{"comment":"dossier","changes":[{{"changeType":"edit","item":{{"path":"/src"}},"newContent":{{"content":"x","contentType":"rawtext"}}}}]}}]}}"#)),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    let error: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(error["message"], "chemin de fichier refusé");
    let (status, body, _) = call(
        &app,
        "POST",
        &pushes,
        Some(&auth),
        None,
        "application/json",
        Some(&format!(r#"{{"refUpdates":[{{"name":"refs/heads/main","oldObjectId":"{added}"}}],"commits":[{{"comment":"illisible","changes":[{{"changeType":"edit","item":{{"path":"/src/pompe.txt"}},"newContent":{{"content":1}}}}]}}]}}"#)),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    let error: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(error["message"], "contenu illisible");
    let (status, body, _) = call(&app, "GET", &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/items?path=/src/pompe.txt&api-version=7.1"), Some(&auth), None, "application/json", None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let file: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(file["content"], "debit 12");

    let (status, body, _) = call(
        &app,
        "POST",
        "/azurz/_apis/hooks/subscriptions?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"publisherId":"tfs","eventType":"git.push","consumerId":"webHooks","consumerActionId":"httpRequest","consumerInputs":{"url":"/_hooks/inbox"},"publisherInputs":{"projectId":"AzurzDemo"}}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status, body, _) = call(
        &app,
        "POST",
        &pushes,
        Some(&auth),
        None,
        "application/json",
        Some(&format!(r#"{{"refUpdates":[{{"name":"refs/heads/main","oldObjectId":"{added}"}}],"commits":[{{"comment":"Pompe revue","changes":[{{"changeType":"edit","item":{{"path":"/src/pompe.txt"}},"newContent":{{"content":"debit 18","contentType":"rawtext"}}}}]}}]}}"#)),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let edited: serde_json::Value = serde_json::from_str(&body).unwrap();
    let edited_id = edited["commits"][0]["commitId"].as_str().unwrap();
    let (status, body, _) = call(&app, "GET", &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/items?path=/src/pompe.txt&api-version=7.1"), Some(&auth), None, "application/json", None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let file: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(file["content"], "debit 18");
    let (status, body, _) = call(&app, "GET", &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/items?path=/src/pompe.txt&versionDescriptor.version={added}&api-version=7.1"), Some(&auth), None, "application/json", None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let previous: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(previous["content"], "debit 12");
    let (status, body, _) = call(&app, "GET", &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/items?path=/README.md&api-version=7.1"), Some(&auth), None, "application/json", None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let readme: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(readme["content"], "# AzurzDemo\n");

    let (status, body, _) = call(
        &app,
        "POST",
        &pushes,
        Some(&auth),
        None,
        "application/json",
        Some(&format!(r#"{{"refUpdates":[{{"name":"refs/heads/main","oldObjectId":"{edited_id}"}}],"commits":[{{"comment":"retrait pompe","changes":[{{"changeType":"delete","item":{{"path":"/src/pompe.txt"}}}}]}}]}}"#)),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status, body, _) = call(&app, "GET", &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/items?path=/src/pompe.txt&api-version=7.1"), Some(&auth), None, "application/json", None).await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    let error: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(error["message"], "fichier introuvable");
    let (status, body, _) = call(&app, "GET", &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/items?path=/src/pompe.txt&versionDescriptor.version={edited_id}&api-version=7.1"), Some(&auth), None, "application/json", None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let kept: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(kept["content"], "debit 18");
    let (status, body, _) = call(&app, "GET", &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/items?path=/README.md&api-version=7.1"), Some(&auth), None, "application/json", None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let readme: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(readme["content"], "# AzurzDemo\n");
    let (status, body, _) = call(
        &app,
        "POST",
        &pushes,
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"refUpdates":[{"name":"refs/heads/main","oldObjectId":"0000000000000000000000000000000000000000"}],"commits":[{"comment":"encore","changes":[{"changeType":"delete","item":{"path":"/src/pompe.txt"}}]}]}"#),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    let error: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(error["message"], "fichier introuvable");

    let (status, body, _) = call(&app, "GET", "/azurz/_apis/hooks/inbox?api-version=7.1", Some(&auth), None, "application/json", None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let inbox: serde_json::Value = serde_json::from_str(&body).unwrap();
    let notes: Vec<_> = inbox["value"].as_array().unwrap().iter().filter(|item| item["eventType"] == "git.push" && item["resource"]["commits"][0]["comment"] == "Pompe revue").collect();
    assert_eq!(notes.len(), 1);
}

#[tokio::test]
async fn dashboard_can_be_renamed_and_a_widget_removed() {
    let app = app();
    let auth = basic("azurz", "azurz-lab-pat");
    let (status, body, _) = call(
        &app,
        "PATCH",
        "/azurz/AzurzDemo/_apis/dashboard/dashboards/dash-99?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        Some("[]"),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    let error: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(error["message"], "tableau de bord introuvable");
    let (status, body, _) = call(
        &app,
        "DELETE",
        "/azurz/AzurzDemo/_apis/dashboard/dashboards/dash-99/widgets/widget-1?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    let error: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(error["message"], "tableau de bord introuvable");

    let (status, body, _) = call(
        &app,
        "POST",
        "/azurz/AzurzDemo/_apis/dashboard/dashboards?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"name":"Vue ligne"}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let dashboard: serde_json::Value = serde_json::from_str(&body).unwrap();
    let dashboard_id = dashboard["id"].as_str().unwrap();
    let position = dashboard["position"].clone();
    let (status, body, _) = call(
        &app,
        "POST",
        "/azurz/AzurzDemo/_apis/dashboard/dashboards?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"name":"Vue pompe"}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let sibling: serde_json::Value = serde_json::from_str(&body).unwrap();
    let sibling_id = sibling["id"].as_str().unwrap();
    let (status, body, _) = call(
        &app,
        "POST",
        &format!("/azurz/AzurzDemo/_apis/dashboard/dashboards/{dashboard_id}/widgets?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"name":"Note","contributionId":"ms.vss-dashboards.MarkdownWidget","settings":{"content":"debit 12"}}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let note: serde_json::Value = serde_json::from_str(&body).unwrap();
    let note_id = note["id"].as_str().unwrap();
    let (status, body, _) = call(
        &app,
        "POST",
        &format!("/azurz/AzurzDemo/_apis/dashboard/dashboards/{dashboard_id}/widgets?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"name":"Stable","contributionId":"ms.vss-dashboards.MarkdownWidget","settings":{"content":"stable"}}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");

    for (payload, message) in [
        ("[]", "tableau illisible"),
        ("{}", "nom de tableau absent"),
        (r#"{"name":"  "}"#, "nom de tableau absent"),
        (r#"{"name":1}"#, "tableau illisible"),
    ] {
        let (status, body, _) = call(
            &app,
            "PATCH",
            &format!("/azurz/AzurzDemo/_apis/dashboard/dashboards/{dashboard_id}?api-version=7.1"),
            Some(&auth),
            None,
            "application/json",
            Some(payload),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
        let error: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(error["message"], message, "{body}");
    }
    let (status, body, _) = call(
        &app,
        "GET",
        &format!("/azurz/AzurzDemo/_apis/dashboard/dashboards/{dashboard_id}?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let stored: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(stored["name"], "Vue ligne");
    assert_eq!(stored["position"], position);
    assert_eq!(stored["widgets"].as_array().unwrap().len(), 2);
    assert_eq!(stored["widgets"][0]["rendered"]["text"], "debit 12");

    let (status, body, _) = call(
        &app,
        "POST",
        "/azurz/_apis/hooks/subscriptions?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"publisherId":"tfs","eventType":"dashboard.updated","consumerId":"webHooks","consumerActionId":"httpRequest","consumerInputs":{"url":"/_hooks/inbox"},"publisherInputs":{"projectId":"AzurzDemo"}}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status, body, _) = call(
        &app,
        "PATCH",
        &format!("/azurz/AzurzDemo/_apis/dashboard/dashboards/{dashboard_id}?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"name":" Vue atelier ","widgets":[]}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let renamed: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(renamed["name"], "Vue atelier");
    assert_eq!(renamed["id"], dashboard_id);
    assert_eq!(renamed["position"], position);
    assert_eq!(renamed["widgets"].as_array().unwrap().len(), 2);
    let (status, body, _) = call(
        &app,
        "GET",
        &format!("/azurz/AzurzDemo/_apis/dashboard/dashboards/{sibling_id}?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let sibling: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(sibling["name"], "Vue pompe");
    assert!(sibling["widgets"].as_array().unwrap().is_empty());

    let (status, body, _) = call(
        &app,
        "POST",
        "/azurz/_apis/hooks/subscriptions?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"publisherId":"tfs","eventType":"dashboard.widget.deleted","consumerId":"webHooks","consumerActionId":"httpRequest","consumerInputs":{"url":"/_hooks/inbox"},"publisherInputs":{"projectId":"AzurzDemo"}}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status, _, _) = call(
        &app,
        "DELETE",
        &format!("/azurz/AzurzDemo/_apis/dashboard/dashboards/{dashboard_id}/widgets/{note_id}?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (status, body, _) = call(
        &app,
        "GET",
        &format!("/azurz/AzurzDemo/_apis/dashboard/dashboards/{dashboard_id}?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let stored: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(stored["name"], "Vue atelier");
    assert_eq!(stored["widgets"].as_array().unwrap().len(), 1);
    assert_eq!(stored["widgets"][0]["name"], "Stable");
    assert_eq!(stored["widgets"][0]["rendered"]["text"], "stable");
    let (status, body, _) = call(
        &app,
        "DELETE",
        &format!("/azurz/AzurzDemo/_apis/dashboard/dashboards/{dashboard_id}/widgets/{note_id}?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    let error: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(error["message"], "widget introuvable");

    let (status, body, _) = call(&app, "GET", "/azurz/_apis/hooks/inbox?api-version=7.1", Some(&auth), None, "application/json", None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let inbox: serde_json::Value = serde_json::from_str(&body).unwrap();
    let renamed_notes: Vec<_> = inbox["value"].as_array().unwrap().iter().filter(|item| item["eventType"] == "dashboard.updated").collect();
    assert_eq!(renamed_notes.len(), 1);
    assert_eq!(renamed_notes[0]["resource"]["name"], "Vue atelier");
    let removed_notes: Vec<_> = inbox["value"].as_array().unwrap().iter().filter(|item| item["eventType"] == "dashboard.widget.deleted").collect();
    assert_eq!(removed_notes.len(), 1);
    assert_eq!(removed_notes[0]["resource"]["name"], "Note");
}

#[tokio::test]
async fn package_can_be_removed_from_the_feed() {
    let app = app();
    let auth = basic("azurz", "azurz-lab-pat");
    let pom = "<project><artifactId>oee</artifactId><version>1.0.0</version></project>";
    let (status, body, _) = call(
        &app,
        "DELETE",
        "/azurz/AzurzDemo/_packaging/absent/maven/v1/com/azurz/oee/1.0.0/oee-1.0.0.pom",
        Some(&auth),
        None,
        "application/xml",
        Some("[]"),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    let error: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(error["message"], "flux introuvable");

    for name in ["ligne-packages", "autre-packages"] {
        let (status, body, _) = call(
            &app,
            "POST",
            "/azurz/AzurzDemo/_apis/packaging/feeds?api-version=7.1",
            Some(&auth),
            None,
            "application/json",
            Some(&format!(r#"{{"name":"{name}"}}"#)),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{body}");
    }
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
    let later = "<project><version>1.1.0</version></project>";
    let (status, body, _) = call(
        &app,
        "PUT",
        "/azurz/AzurzDemo/_packaging/ligne-packages/maven/v1/com/azurz/oee/1.1.0/oee-1.1.0.pom",
        Some(&auth),
        None,
        "application/xml",
        Some(later),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status, body, _) = call(
        &app,
        "PUT",
        "/azurz/AzurzDemo/_packaging/autre-packages/maven/v1/com/azurz/oee/1.0.0/oee-1.0.0.pom",
        Some(&auth),
        None,
        "application/xml",
        Some(pom),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status, body, _) = call(
        &app,
        "PUT",
        "/azurz/AzurzDemo/_packaging/ligne-packages/pypi/simple/oee/oee-1.2.0.tar.gz",
        Some(&auth),
        None,
        "application/octet-stream",
        Some("paquet 1.2.0"),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status, body, _) = call(
        &app,
        "PUT",
        "/azurz/AzurzDemo/_packaging/ligne-packages/pypi/simple/oee/oee-1.3.0.tar.gz",
        Some(&auth),
        None,
        "application/octet-stream",
        Some("paquet 1.3.0"),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");

    let (status, body, _) = call(
        &app,
        "DELETE",
        "/azurz/AzurzDemo/_packaging/ligne-packages/maven/v1/com/azurz/oee/maven-metadata.xml",
        Some(&auth),
        None,
        "application/xml",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::METHOD_NOT_ALLOWED, "{body}");
    let error: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(error["message"], "méthode refusée");
    let (status, body, _) = call(
        &app,
        "DELETE",
        "/azurz/AzurzDemo/_packaging/ligne-packages/maven/v1/com/azurz/oee/1.0.0/oee-1.0.0.jar",
        Some(&auth),
        None,
        "application/xml",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    let error: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(error["message"], "paquet Maven introuvable");
    let (status, body, _) = call(
        &app,
        "GET",
        "/azurz/AzurzDemo/_packaging/ligne-packages/maven/v1/com/azurz/oee/1.0.0/oee-1.0.0.pom",
        Some(&auth),
        None,
        "application/xml",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body, pom);

    let (status, body, _) = call(
        &app,
        "POST",
        "/azurz/_apis/hooks/subscriptions?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"publisherId":"tfs","eventType":"packaging.package.deleted","consumerId":"webHooks","consumerActionId":"httpRequest","consumerInputs":{"url":"/_hooks/inbox"},"publisherInputs":{"projectId":"AzurzDemo"}}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status, _, _) = call(
        &app,
        "DELETE",
        "/azurz/AzurzDemo/_packaging/ligne-packages/maven/v1/com/azurz/oee/1.0.0/oee-1.0.0.pom",
        Some(&auth),
        None,
        "application/xml",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (status, body, _) = call(
        &app,
        "GET",
        "/azurz/AzurzDemo/_packaging/ligne-packages/maven/v1/com/azurz/oee/1.0.0/oee-1.0.0.pom",
        Some(&auth),
        None,
        "application/xml",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    let (status, body, _) = call(
        &app,
        "GET",
        "/azurz/AzurzDemo/_packaging/ligne-packages/maven/v1/com/azurz/oee/1.1.0/oee-1.1.0.pom",
        Some(&auth),
        None,
        "application/xml",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body, later);
    let (status, body, _) = call(
        &app,
        "GET",
        "/azurz/AzurzDemo/_packaging/ligne-packages/maven/v1/com/azurz/oee/maven-metadata.xml",
        Some(&auth),
        None,
        "application/xml",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(body.contains("<version>1.1.0</version>"), "{body}");
    assert!(!body.contains("<version>1.0.0</version>"), "{body}");
    let (status, body, _) = call(
        &app,
        "GET",
        "/azurz/AzurzDemo/_packaging/autre-packages/maven/v1/com/azurz/oee/1.0.0/oee-1.0.0.pom",
        Some(&auth),
        None,
        "application/xml",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body, pom);
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
    assert_eq!(maven["count"], 1);
    assert_eq!(maven["value"][0]["version"], "1.1.0");
    assert!(maven["value"][0].get("content").is_none());

    let (status, _, _) = call(
        &app,
        "DELETE",
        "/azurz/AzurzDemo/_packaging/ligne-packages/pypi/simple/oee/oee-1.2.0.tar.gz",
        Some(&auth),
        None,
        "application/octet-stream",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
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
    assert!(body.contains("oee-1.3.0.tar.gz"), "{body}");
    assert!(!body.contains("oee-1.2.0.tar.gz"), "{body}");
    let (status, body, _) = call(
        &app,
        "GET",
        "/azurz/AzurzDemo/_packaging/ligne-packages/pypi/simple/oee/oee-1.3.0.tar.gz",
        Some(&auth),
        None,
        "application/octet-stream",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body, "paquet 1.3.0");
    let (status, body, _) = call(
        &app,
        "DELETE",
        "/azurz/AzurzDemo/_packaging/ligne-packages/pypi/simple/oee/oee-1.2.0.tar.gz",
        Some(&auth),
        None,
        "application/octet-stream",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    let error: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(error["message"], "paquet PyPI introuvable");

    let (status, body, _) = call(&app, "GET", "/azurz/_apis/hooks/inbox?api-version=7.1", Some(&auth), None, "application/json", None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let inbox: serde_json::Value = serde_json::from_str(&body).unwrap();
    let notes: Vec<_> = inbox["value"].as_array().unwrap().iter().filter(|item| item["eventType"] == "packaging.package.deleted").collect();
    assert_eq!(notes.len(), 2);
    assert_eq!(notes[0]["resource"]["filename"], "oee-1.0.0.pom");
    assert_eq!(notes[0]["resource"]["name"], "oee");
    assert_eq!(notes[0]["resource"]["protocol"], "maven");
    assert!(notes[0]["resource"].get("content").is_none());
    assert_eq!(notes[1]["resource"]["filename"], "oee-1.2.0.tar.gz");
    assert_eq!(notes[1]["resource"]["protocol"], "pypi");
    assert!(notes[1]["resource"].get("content").is_none());
}

#[tokio::test]
async fn npm_package_version_can_be_removed() {
    let app = app();
    let auth = basic("azurz", "azurz-lab-pat");
    let (status, body, _) = call(
        &app,
        "DELETE",
        "/azurz/AzurzDemo/_packaging/absent-npm/npm/oee/1.0.0",
        Some(&auth),
        None,
        "application/json",
        Some("[]"),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    let error: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(error["message"], "flux introuvable");

    for name in ["ligne-npm", "autre-npm"] {
        let (status, body, _) = call(
            &app,
            "POST",
            "/azurz/AzurzDemo/_apis/packaging/feeds?api-version=7.1",
            Some(&auth),
            None,
            "application/json",
            Some(&format!(r#"{{"name":"{name}"}}"#)),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{body}");
    }
    let document = r#"{"name":"oee","description":"ligne","versions":{"1.0.0":{"version":"1.0.0"},"1.1.0":{"version":"1.1.0"}}}"#;
    let (status, body, _) = call(
        &app,
        "PUT",
        "/azurz/AzurzDemo/_packaging/ligne-npm/npm/oee",
        Some(&auth),
        None,
        "application/json",
        Some(document),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status, body, _) = call(
        &app,
        "PUT",
        "/azurz/AzurzDemo/_packaging/ligne-npm/npm/pompe",
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"name":"pompe","versions":{"1.0.0":{"version":"1.0.0"}}}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status, body, _) = call(
        &app,
        "PUT",
        "/azurz/AzurzDemo/_packaging/autre-npm/npm/oee",
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"name":"oee","versions":{"1.0.0":{"version":"1.0.0"}}}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");

    let (status, body, _) = call(
        &app,
        "DELETE",
        "/azurz/AzurzDemo/_packaging/ligne-npm/npm/absente/1.0.0",
        Some(&auth),
        None,
        "application/json",
        Some("[]"),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    let error: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(error["message"], "paquet npm introuvable");
    let (status, body, _) = call(
        &app,
        "DELETE",
        "/azurz/AzurzDemo/_packaging/ligne-npm/npm/oee/9.9.9",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    let error: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(error["message"], "version npm introuvable");
    let (status, body, _) = call(
        &app,
        "DELETE",
        "/azurz/AzurzDemo/_packaging/ligne-npm/npm/oee",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    let error: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(error["message"], "version npm absente");
    let (status, body, _) = call(
        &app,
        "DELETE",
        "/azurz/AzurzDemo/_packaging/ligne-npm/npm/oee/%20",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    let error: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(error["message"], "version npm absente");
    let (status, body, _) = call(
        &app,
        "GET",
        "/azurz/AzurzDemo/_packaging/ligne-npm/npm/oee",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let package: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(package["description"], "ligne");
    assert_eq!(package["versions"]["1.0.0"]["version"], "1.0.0");
    assert_eq!(package["versions"]["1.1.0"]["version"], "1.1.0");

    let (status, body, _) = call(
        &app,
        "POST",
        "/azurz/_apis/hooks/subscriptions?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"publisherId":"tfs","eventType":"packaging.package.deleted","consumerId":"webHooks","consumerActionId":"httpRequest","consumerInputs":{"url":"/_hooks/inbox"},"publisherInputs":{"projectId":"AzurzDemo"}}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status, _, _) = call(
        &app,
        "DELETE",
        "/azurz/AzurzDemo/_packaging/ligne-npm/npm/oee/1.0.0",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (status, body, _) = call(
        &app,
        "GET",
        "/azurz/AzurzDemo/_packaging/ligne-npm/npm/oee",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let package: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert!(package["versions"].get("1.0.0").is_none(), "{body}");
    assert_eq!(package["versions"]["1.1.0"]["version"], "1.1.0");
    assert_eq!(package["description"], "ligne");
    let (status, body, _) = call(
        &app,
        "GET",
        "/azurz/AzurzDemo/_packaging/ligne-npm/npm/pompe",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let pompe: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(pompe["versions"]["1.0.0"]["version"], "1.0.0");
    let (status, body, _) = call(
        &app,
        "GET",
        "/azurz/AzurzDemo/_packaging/autre-npm/npm/oee",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let other: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(other["versions"]["1.0.0"]["version"], "1.0.0");
    let (status, body, _) = call(
        &app,
        "GET",
        "/azurz/AzurzDemo/_apis/packaging/feeds/ligne-npm/npm/packages?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let listed: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(listed["count"], 2);
    let oee = listed["value"].as_array().unwrap().iter().find(|item| item["name"] == "oee").unwrap();
    assert!(oee["versions"].get("1.0.0").is_none(), "{body}");
    assert_eq!(oee["versions"]["1.1.0"]["version"], "1.1.0");
    assert!(oee.get("content").is_none());

    let (status, _, _) = call(
        &app,
        "DELETE",
        "/azurz/AzurzDemo/_packaging/ligne-npm/npm/oee/1.1.0",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (status, body, _) = call(
        &app,
        "GET",
        "/azurz/AzurzDemo/_packaging/ligne-npm/npm/oee",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    let error: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(error["message"], "paquet introuvable");
    let (status, body, _) = call(
        &app,
        "DELETE",
        "/azurz/AzurzDemo/_packaging/ligne-npm/npm/oee/1.1.0",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    let error: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(error["message"], "paquet npm introuvable");
    let (status, body, _) = call(
        &app,
        "GET",
        "/azurz/AzurzDemo/_packaging/ligne-npm/npm/pompe",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");

    let (status, body, _) = call(&app, "GET", "/azurz/_apis/hooks/inbox?api-version=7.1", Some(&auth), None, "application/json", None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let inbox: serde_json::Value = serde_json::from_str(&body).unwrap();
    let notes: Vec<_> = inbox["value"].as_array().unwrap().iter().filter(|item| item["eventType"] == "packaging.package.deleted").collect();
    assert_eq!(notes.len(), 2);
    assert_eq!(notes[0]["resource"]["filename"], "oee-1.0.0.tgz");
    assert_eq!(notes[0]["resource"]["name"], "oee");
    assert_eq!(notes[0]["resource"]["version"], "1.0.0");
    assert_eq!(notes[0]["resource"]["protocol"], "npm");
    assert!(notes[0]["resource"].get("content").is_none());
    assert_eq!(notes[1]["resource"]["filename"], "oee-1.1.0.tgz");
    assert_eq!(notes[1]["resource"]["protocol"], "npm");
    assert!(notes[1]["resource"].get("content").is_none());
}

#[tokio::test]
async fn variable_group_can_be_removed() {
    let app = app();
    let auth = basic("azurz", "azurz-lab-pat");
    let (status, body, _) = call(
        &app,
        "DELETE",
        "/azurz/AzurzDemo/_apis/distributedtask/variablegroups/99?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        Some("[]"),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    let error: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(error["message"], "groupe introuvable");

    let (status, body, _) = call(
        &app,
        "POST",
        "/azurz/AzurzDemo/_apis/distributedtask/variablegroups?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"name":"Ligne","type":"Vsts","variables":{"host":{"value":"ignition","isSecret":false},"token":{"value":"ligne-clef","isSecret":true}}}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let created: serde_json::Value = serde_json::from_str(&body).unwrap();
    let group_id = created["id"].as_i64().unwrap();
    assert_eq!(created["variables"]["token"]["value"], "");
    assert!(!body.contains("ligne-clef"));

    let (status, body, _) = call(
        &app,
        "POST",
        "/azurz/AzurzDemo/_apis/distributedtask/variablegroups?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"name":"Voisin","type":"Vsts","variables":{"site":{"value":"atelier","isSecret":false},"clef":{"value":"voisin-clef","isSecret":true}}}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let sibling: serde_json::Value = serde_json::from_str(&body).unwrap();
    let sibling_id = sibling["id"].as_i64().unwrap();

    let (status, body, _) = call(
        &app,
        "PUT",
        "/azurz/AzurzDemo/_apis/build/definitions/1?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        Some(&format!(
            r#"{{"variableGroups":[{{"id":{group_id}}},{{"id":{sibling_id}}}],"variables":{{"poste":{{"value":"pompe","isSecret":false}}}}}}"#
        )),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let linked: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(linked["variableGroups"].as_array().unwrap().len(), 2);
    assert_eq!(linked["variables"]["poste"]["value"], "pompe");

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
    let queued: serde_json::Value = serde_json::from_str(&body).unwrap();
    let build_id = queued["id"].as_i64().unwrap();
    assert_eq!(queued["variables"]["host"]["value"], "ignition");
    assert_eq!(queued["variables"]["token"]["value"], "");
    assert_eq!(queued["variables"]["token"]["isSecret"], true);
    assert_eq!(queued["variables"]["poste"]["value"], "pompe");
    assert!(!body.contains("ligne-clef"));
    assert!(!body.contains("voisin-clef"));

    let (status, body, _) = call(
        &app,
        "POST",
        "/azurz/_apis/hooks/subscriptions?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"publisherId":"tfs","eventType":"library.variablegroup.deleted","consumerId":"webHooks","consumerActionId":"httpRequest","consumerInputs":{"url":"/_hooks/inbox"},"publisherInputs":{"projectId":"AzurzDemo"}}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");

    let (status, _, _) = call(
        &app,
        "DELETE",
        &format!("/azurz/AzurzDemo/_apis/distributedtask/variablegroups/{group_id}?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    let (status, body, _) = call(
        &app,
        "GET",
        &format!("/azurz/AzurzDemo/_apis/distributedtask/variablegroups/{group_id}?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    let error: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(error["message"], "groupe introuvable");

    let (status, body, _) = call(
        &app,
        "GET",
        &format!("/azurz/AzurzDemo/_apis/distributedtask/variablegroups/{sibling_id}?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let kept: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(kept["name"], "Voisin");
    assert_eq!(kept["variables"]["site"]["value"], "atelier");
    assert_eq!(kept["variables"]["clef"]["value"], "");
    assert_eq!(kept["variables"]["clef"]["isSecret"], true);
    assert!(!body.contains("voisin-clef"));

    let (status, body, _) = call(
        &app,
        "GET",
        "/azurz/AzurzDemo/_apis/build/definitions/1?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let definition: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(definition["variableGroups"].as_array().unwrap().len(), 2);
    assert_eq!(definition["variables"]["poste"]["value"], "pompe");

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
    assert_eq!(finished["variables"]["host"]["value"], "ignition");
    assert_eq!(finished["variables"]["token"]["value"], "");
    assert_eq!(finished["variables"]["token"]["isSecret"], true);
    assert_eq!(finished["variables"]["poste"]["value"], "pompe");
    assert!(!body.contains("ligne-clef"));

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
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    let error: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(error["message"], "groupe introuvable");

    let (status, body, _) = call(
        &app,
        "DELETE",
        &format!("/azurz/AzurzDemo/_apis/distributedtask/variablegroups/{group_id}?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some("[]"),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    let error: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(error["message"], "groupe introuvable");

    let (status, body, _) = call(&app, "GET", "/azurz/_apis/hooks/inbox?api-version=7.1", Some(&auth), None, "application/json", None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let inbox: serde_json::Value = serde_json::from_str(&body).unwrap();
    let notes: Vec<_> = inbox["value"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|item| item["eventType"] == "library.variablegroup.deleted")
        .collect();
    assert_eq!(notes.len(), 1);
    assert_eq!(notes[0]["resource"]["name"], "Ligne");
    assert_eq!(notes[0]["resource"]["type"], "Vsts");
    assert_eq!(notes[0]["resource"]["variables"]["token"]["value"], "");
    assert_eq!(notes[0]["resource"]["variables"]["token"]["isSecret"], true);
    assert_eq!(notes[0]["resource"]["variables"]["host"]["value"], "ignition");
    assert!(!body.contains("ligne-clef"));
    assert!(!body.contains("voisin-clef"));
}

#[tokio::test]
async fn schedule_trigger_queues_one_build_at_the_lab_clock() {
    let app = app();
    let auth = basic("azurz", "azurz-lab-pat");
    let put = |body: &'static str| {
        let app = app.clone();
        let auth = auth.clone();
        async move {
            call(
                &app,
                "PUT",
                "/azurz/AzurzDemo/_apis/build/definitions/1?api-version=7.1",
                Some(&auth),
                None,
                "application/json",
                Some(body),
            )
            .await
        }
    };
    let (status, body, _) = put(r#"{"triggers":[{"triggerType":"schedule","schedules":[]}]}"#).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert!(body.contains("horaire absent"));
    let (status, body, _) = put(r#"{"triggers":[{"triggerType":"schedule","schedules":"jeudi"}]}"#).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert!(body.contains("horaire illisible"));
    let (status, body, _) = put(r#"{"triggers":[{"triggerType":"schedule","schedules":[{"daysToBuild":[""]}]}]}"#).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert!(body.contains("jour absent"));
    let (status, body, _) = put(r#"{"triggers":[{"triggerType":"schedule","schedules":[{"daysToBuild":["funday"]}]}]}"#).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert!(body.contains("jour inconnu"));
    let (status, body, _) = put(r#"{"triggers":[{"triggerType":"schedule","schedules":[{"daysToBuild":["thursday"],"startHours":25}]}]}"#).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert!(body.contains("heure inconnue"));
    let (status, body, _) = put(r#"{"triggers":[{"triggerType":"schedule","schedules":[{"daysToBuild":["thursday"],"startMinutes":60}]}]}"#).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert!(body.contains("minute inconnue"));
    let (status, body, _) = call(
        &app,
        "GET",
        "/azurz/AzurzDemo/_apis/build/definitions/1?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let untouched: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(untouched["triggers"], serde_json::json!([]));

    let (status, body, _) = put(r#"{"triggers":[{"triggerType":"schedule","schedules":[{"daysToBuild":["Thursday"],"startHours":6,"startMinutes":0,"branchFilters":["+refs/heads/main"],"scheduleOnlyWithChanges":false}]}]}"#).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let morning: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(morning["triggers"][0]["schedules"][0]["daysToBuild"][0], "thursday");
    assert_eq!(morning["triggers"][0]["schedules"][0]["startHours"], 6);
    let (status, body, _) = call(
        &app,
        "GET",
        "/azurz/AzurzDemo/_apis/build/builds?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let quiet: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(quiet["count"], 0);

    let (status, body, _) = put(r#"{"triggers":[{"triggerType":"schedule","schedules":[{"daysToBuild":["monday"],"startHours":0,"startMinutes":0,"branchFilters":["+refs/heads/main"]}]}]}"#).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status, body, _) = call(
        &app,
        "GET",
        "/azurz/AzurzDemo/_apis/build/builds?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let monday: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(monday["count"], 0);

    let (status, body, _) = put(r#"{"triggers":[{"triggerType":"schedule","schedules":[{"daysToBuild":["thursday"],"startHours":0,"startMinutes":0,"branchFilters":["+refs/heads/wip"]}]}]}"#).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status, body, _) = call(
        &app,
        "GET",
        "/azurz/AzurzDemo/_apis/build/builds?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let other_branch: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(other_branch["count"], 0);

    let (status, body, _) = call(
        &app,
        "POST",
        "/azurz/_apis/hooks/subscriptions?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"publisherId":"tfs","eventType":"build.complete","consumerId":"webHooks","consumerActionId":"httpRequest","consumerInputs":{"url":"/_hooks/inbox"},"publisherInputs":{"projectId":"AzurzDemo"}}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");

    let (status, body, _) = put(r#"{"triggers":[{"triggerType":"schedule","schedules":[{"daysToBuild":["thursday"],"startHours":0,"startMinutes":0,"branchFilters":["+refs/heads/main"],"scheduleOnlyWithChanges":true}]}]}"#).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let armed: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(armed["triggers"][0]["schedules"][0]["scheduleOnlyWithChanges"], true);
    assert_eq!(armed["triggers"][0]["schedules"][0]["timeZoneId"], "UTC");

    let (status, body, _) = call(
        &app,
        "GET",
        "/azurz/AzurzDemo/_apis/build/builds?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let queued: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(queued["count"], 1);
    assert_eq!(queued["value"][0]["reason"], "schedule");
    assert_eq!(queued["value"][0]["sourceBranch"], "refs/heads/main");
    assert_eq!(queued["value"][0]["definition"]["id"], 1);
    assert_eq!(queued["value"][0]["definition"]["name"], "CI");
    assert_eq!(queued["value"][0]["status"], "completed");
    assert_eq!(queued["value"][0]["result"], "succeeded");

    let (status, body, _) = call(
        &app,
        "GET",
        "/azurz/AzurzDemo/_apis/build/builds?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let again: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(again["count"], 1);

    let (status, body, _) = call(
        &app,
        "POST",
        "/azurz/AzurzDemo/_apis/build/definitions?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"name":"Nuit","queueStatus":"disabled","triggers":[{"triggerType":"schedule","schedules":[{"daysToBuild":["thursday"],"startHours":0,"startMinutes":0,"branchFilters":["+refs/heads/main"]}]}]}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let night: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(night["queueStatus"], "disabled");
    let (status, body, _) = call(
        &app,
        "GET",
        "/azurz/AzurzDemo/_apis/build/builds?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let held: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(held["count"], 1);
    assert!(held["value"].as_array().unwrap().iter().all(|item| item["definition"]["id"] != night["id"]));

    let (status, body, _) = call(&app, "GET", "/azurz/_apis/hooks/inbox?api-version=7.1", Some(&auth), None, "application/json", None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let inbox: serde_json::Value = serde_json::from_str(&body).unwrap();
    let notes: Vec<_> = inbox["value"].as_array().unwrap().iter().filter(|item| item["eventType"] == "build.complete").collect();
    assert_eq!(notes.len(), 1);
    assert_eq!(notes[0]["resource"]["reason"], "schedule");
    assert_eq!(notes[0]["resource"]["definition"]["name"], "CI");
}

#[tokio::test]
async fn work_item_type_states_keep_a_known_state_and_a_board_mapping() {
    let app = app();
    let auth = basic("azurz", "azurz-lab-pat");
    let (status, body, _) = call(
        &app,
        "GET",
        "/azurz/AzurzDemo/_apis/wit/workitemtypes/Inconnu?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    assert!(body.contains("type introuvable"), "{body}");

    let (status, body, _) = call(
        &app,
        "GET",
        "/azurz/AzurzDemo/_apis/wit/workitemtypes/user%20story?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let story: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(story["name"], "User Story");
    assert_eq!(story["referenceName"], "Microsoft.VSTS.WorkItemTypes.UserStory");
    assert_eq!(story["states"].as_array().unwrap().len(), 4);
    assert_eq!(story["states"][0]["name"], "New");
    assert_eq!(story["states"][0]["color"], "b2b2b2");
    assert_eq!(story["states"][0]["category"], "Proposed");

    let (status, body, _) = call(
        &app,
        "GET",
        "/azurz/AzurzDemo/_apis/wit/workitemtypes/User%20Story/states?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let states: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(states["count"], 4);
    assert_eq!(states["value"][1]["name"], "Active");
    assert_eq!(states["value"][1]["category"], "InProgress");
    assert_eq!(states["value"][1]["color"], "007acc");
    assert_eq!(states["value"][2]["name"], "Resolved");
    assert_eq!(states["value"][3]["name"], "Closed");
    assert_eq!(states["value"][3]["category"], "Completed");

    let (status, body, _) = call(
        &app,
        "GET",
        "/azurz/AzurzDemo/_apis/wit/workitemtypes/User%20Story/states/active?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let active: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(active["name"], "Active");

    let (status, body, _) = call(
        &app,
        "GET",
        "/azurz/AzurzDemo/_apis/wit/workitemtypes/User%20Story/states/gele?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    assert!(body.contains("état introuvable"), "{body}");

    let create = |body: &'static str| {
        let app = app.clone();
        let auth = auth.clone();
        async move {
            call(
                &app,
                "POST",
                "/azurz/AzurzDemo/_apis/wit/workitems/$User%20Story?api-version=7.1",
                Some(&auth),
                None,
                "application/json-patch+json",
                Some(body),
            )
            .await
        }
    };
    for (payload, message) in [
        (r#"[{"op":"add","path":"/fields/System.Title","value":"Vide"},{"op":"add","path":"/fields/System.State","value":" "}]"#, "état absent"),
        (r#"[{"op":"add","path":"/fields/System.Title","value":"Nombre"},{"op":"add","path":"/fields/System.State","value":3}]"#, "état illisible"),
        (r#"[{"op":"add","path":"/fields/System.Title","value":"Gele"},{"op":"add","path":"/fields/System.State","value":"Gelé"}]"#, "état inconnu"),
    ] {
        let (status, body, _) = create(payload).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
        let error: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(error["message"], message, "{body}");
    }
    let (status, body, _) = call(
        &app,
        "GET",
        "/azurz/AzurzDemo/_apis/wit/workitems?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let empty: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(empty["count"], 0, "{body}");

    let (status, body, _) = create(r#"[{"op":"add","path":"/fields/System.Title","value":"Pompe"},{"op":"add","path":"/fields/System.State","value":"active"}]"#).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let item: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(item["fields"]["System.State"], "Active");
    let id = item["id"].as_i64().unwrap();

    let (status, body, _) = call(
        &app,
        "PATCH",
        &format!("/azurz/AzurzDemo/_apis/wit/workitems/{id}?api-version=7.1"),
        Some(&auth),
        None,
        "application/json-patch+json",
        Some(r#"[{"op":"replace","path":"/fields/System.State","value":"Gelé"}]"#),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert!(body.contains("état inconnu"), "{body}");
    let (status, body, _) = call(
        &app,
        "GET",
        &format!("/azurz/AzurzDemo/_apis/wit/workitems/{id}?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let kept: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(kept["fields"]["System.State"], "Active");

    let (status, body, _) = call(
        &app,
        "PUT",
        "/azurz/AzurzDemo/AzurzDemo%20Team/_apis/work/boards/Stories/columns?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        Some(r#"[{"name":"New","columnType":"incoming"},{"name":"Active","columnType":"inProgress"},{"name":"Revue","stateMappings":{"User Story":"En revue"}},{"name":"Closed","columnType":"outgoing"}]"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");

    let (status, body, _) = call(
        &app,
        "PATCH",
        &format!("/azurz/AzurzDemo/_apis/wit/workitems/{id}?api-version=7.1"),
        Some(&auth),
        None,
        "application/json-patch+json",
        Some(r#"[{"op":"replace","path":"/fields/System.State","value":"En revue"}]"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let moved: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(moved["fields"]["System.State"], "En revue");

    let (status, body, _) = call(
        &app,
        "POST",
        "/azurz/AzurzDemo/_apis/wit/workitems/$Bug?api-version=7.1",
        Some(&auth),
        None,
        "application/json-patch+json",
        Some(r#"[{"op":"add","path":"/fields/System.Title","value":"Fuite"},{"op":"add","path":"/fields/System.State","value":"En revue"}]"#),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert!(body.contains("état inconnu"), "{body}");

    let (status, body, _) = call(
        &app,
        "POST",
        "/azurz/AzurzDemo/_apis/wit/workitems/$Bug?api-version=7.1",
        Some(&auth),
        None,
        "application/json-patch+json",
        Some(r#"[{"op":"add","path":"/fields/System.Title","value":"Fuite"},{"op":"add","path":"/fields/System.State","value":"Revue"}]"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let bug: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(bug["fields"]["System.WorkItemType"], "Bug");
    assert_eq!(bug["fields"]["System.State"], "Revue");

    let (status, body, _) = call(
        &app,
        "GET",
        "/azurz/AzurzDemo/_apis/wit/workitemtypes/Bug/states?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let bug_states: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(bug_states["count"], 4);
    assert!(bug_states["value"].as_array().unwrap().iter().all(|state| state["name"] != "En revue" && state["name"] != "Revue"));
}

#[tokio::test]
async fn work_item_type_transitions_refuse_a_hop_between_mapped_states() {
    let app = app();
    let auth = basic("azurz", "azurz-lab-pat");
    let (status, body, _) = call(
        &app,
        "GET",
        "/azurz/AzurzDemo/_apis/wit/workitemtypes/Inconnu/transitions?api-version=7.1-preview.1",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    assert!(body.contains("type introuvable"), "{body}");

    let (status, body, _) = call(
        &app,
        "GET",
        "/azurz/AzurzDemo/_apis/wit/workitemtypes/user%20story/transitions?api-version=7.1-preview.1",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let before: serde_json::Value = serde_json::from_str(&body).unwrap();
    let mut keys: Vec<_> = before.as_object().unwrap().keys().cloned().collect();
    keys.sort();
    assert_eq!(keys, vec!["", "Active", "Closed", "New", "Resolved"]);
    assert!(before.get("En revue").is_none());
    assert!(before["New"][0]["actions"].is_null());
    let new_targets: Vec<_> = before["New"].as_array().unwrap().iter().map(|edge| edge["to"].as_str().unwrap().to_string()).collect();
    assert_eq!(new_targets, vec!["New", "Active", "Resolved", "Closed"]);

    let (status, body, _) = call(
        &app,
        "PUT",
        "/azurz/AzurzDemo/AzurzDemo%20Team/_apis/work/boards/Stories/columns?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        Some(
            r#"[{"name":"New","columnType":"incoming"},{"name":"Active","columnType":"inProgress"},{"name":"Revue","stateMappings":{"User Story":"En revue"}},{"name":"Bloqué","stateMappings":{"User Story":"Bloqué"}},{"name":"Closed","columnType":"outgoing"}]"#,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");

    let (status, body, _) = call(
        &app,
        "GET",
        "/azurz/AzurzDemo/_apis/wit/workitemtypes/User%20Story/transitions?api-version=7.1-preview.1",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let graph: serde_json::Value = serde_json::from_str(&body).unwrap();
    let targets = |key: &str| -> Vec<String> {
        graph[key].as_array().unwrap().iter().map(|edge| edge["to"].as_str().unwrap().to_string()).collect()
    };
    let from_new = targets("New");
    assert!(from_new.iter().any(|name| name == "En revue"));
    assert!(from_new.iter().any(|name| name == "Bloqué"));
    let from_review = targets("En revue");
    assert!(from_review.iter().any(|name| name == "Active"));
    assert!(from_review.iter().any(|name| name == "En revue"));
    assert!(from_review.iter().all(|name| name != "Bloqué"));
    assert!(targets("").iter().any(|name| name == "Bloqué"));

    let (status, body, _) = call(
        &app,
        "GET",
        "/azurz/AzurzDemo/_apis/wit/workitemtypes/User%20Story/states?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let states: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(states["count"], 4);
    assert!(states["value"].as_array().unwrap().iter().all(|state| state["name"] != "En revue" && state["name"] != "Bloqué"));

    let (status, body, _) = call(
        &app,
        "POST",
        "/azurz/AzurzDemo/_apis/wit/workitems/$User%20Story?api-version=7.1",
        Some(&auth),
        None,
        "application/json-patch+json",
        Some(r#"[{"op":"add","path":"/fields/System.Title","value":"Pompe"},{"op":"add","path":"/fields/System.State","value":"active"}]"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let item: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(item["fields"]["System.State"], "Active");
    let id = item["id"].as_i64().unwrap();

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

    let (status, body, _) = call(
        &app,
        "PATCH",
        &format!("/azurz/AzurzDemo/_apis/wit/workitems/{id}?api-version=7.1"),
        Some(&auth),
        None,
        "application/json-patch+json",
        Some(r#"[{"op":"replace","path":"/fields/System.State","value":"En revue"},{"op":"replace","path":"/fields/System.State","value":"Bloqué"}]"#),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert!(body.contains("transition inconnue"), "{body}");
    let (status, body, _) = call(
        &app,
        "GET",
        &format!("/azurz/AzurzDemo/_apis/wit/workitems/{id}?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let stayed: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(stayed["fields"]["System.State"], "Active");

    let (status, body, _) = call(
        &app,
        "PATCH",
        &format!("/azurz/AzurzDemo/_apis/wit/workitems/{id}?api-version=7.1"),
        Some(&auth),
        None,
        "application/json-patch+json",
        Some(r#"[{"op":"replace","path":"/fields/System.State","value":"En revue"}]"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let reviewed: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(reviewed["fields"]["System.State"], "En revue");

    let (status, body, _) = call(
        &app,
        "PATCH",
        &format!("/azurz/AzurzDemo/_apis/wit/workitems/{id}?api-version=7.1"),
        Some(&auth),
        None,
        "application/json-patch+json",
        Some(r#"[{"op":"replace","path":"/fields/System.State","value":"Bloqué"}]"#),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert!(body.contains("transition inconnue"), "{body}");
    let (status, body, _) = call(
        &app,
        "GET",
        &format!("/azurz/AzurzDemo/_apis/wit/workitems/{id}?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let kept: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(kept["fields"]["System.State"], "En revue");

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

    let (status, body, _) = call(
        &app,
        "PATCH",
        &format!("/azurz/AzurzDemo/_apis/wit/workitems/{id}?api-version=7.1"),
        Some(&auth),
        None,
        "application/json-patch+json",
        Some(r#"[{"op":"replace","path":"/fields/System.State","value":"Bloqué"}]"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let blocked: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(blocked["fields"]["System.State"], "Bloqué");

    let (status, body, _) = call(
        &app,
        "PATCH",
        &format!("/azurz/AzurzDemo/_apis/wit/workitems/{id}?api-version=7.1"),
        Some(&auth),
        None,
        "application/json-patch+json",
        Some(r#"[{"op":"replace","path":"/fields/System.State","value":"En revue"}]"#),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert!(body.contains("transition inconnue"), "{body}");

    let (status, body, _) = call(
        &app,
        "POST",
        "/azurz/AzurzDemo/_apis/wit/workitems/$User%20Story?api-version=7.1",
        Some(&auth),
        None,
        "application/json-patch+json",
        Some(r#"[{"op":"add","path":"/fields/System.Title","value":"Fermee"},{"op":"add","path":"/fields/System.State","value":"New"}]"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let other: serde_json::Value = serde_json::from_str(&body).unwrap();
    let other_id = other["id"].as_i64().unwrap();
    let (status, body, _) = call(
        &app,
        "PATCH",
        &format!("/azurz/AzurzDemo/_apis/wit/workitems/{other_id}?api-version=7.1"),
        Some(&auth),
        None,
        "application/json-patch+json",
        Some(r#"[{"op":"replace","path":"/fields/System.State","value":"Closed"}]"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let closed: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(closed["fields"]["System.State"], "Closed");

    let (status, body, _) = call(
        &app,
        "POST",
        "/azurz/AzurzDemo/_apis/wit/workitems/$Bug?api-version=7.1",
        Some(&auth),
        None,
        "application/json-patch+json",
        Some(r#"[{"op":"add","path":"/fields/System.Title","value":"Fuite"},{"op":"add","path":"/fields/System.State","value":"En revue"}]"#),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert!(body.contains("état inconnu"), "{body}");

    let (status, body, _) = call(
        &app,
        "PUT",
        "/azurz/AzurzDemo/AzurzDemo%20Team/_apis/work/boards/Stories/columns?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        Some(r#"[{"name":"New","columnType":"incoming"},{"name":"Active","columnType":"inProgress"},{"name":"Resolved"},{"name":"Closed","columnType":"outgoing"}]"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");

    let (status, body, _) = call(
        &app,
        "PATCH",
        &format!("/azurz/AzurzDemo/_apis/wit/workitems/{id}?api-version=7.1"),
        Some(&auth),
        None,
        "application/json-patch+json",
        Some(r#"[{"op":"replace","path":"/fields/System.State","value":"En revue"}]"#),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert!(body.contains("état inconnu"), "{body}");

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
    let back: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(back["fields"]["System.State"], "Active");
}

#[tokio::test]
async fn universal_package_version_can_be_removed() {
    let app = app();
    let auth = basic("azurz", "azurz-lab-pat");
    let (status, body, _) = call(
        &app,
        "DELETE",
        "/azurz/AzurzDemo/_apis/packaging/feeds/absent/upack/packages/module/versions/1.0.0?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    assert!(body.contains("flux introuvable"), "{body}");

    for name in ["ligne-upack", "autre-upack"] {
        let (status, body, _) = call(
            &app,
            "POST",
            "/azurz/AzurzDemo/_apis/packaging/feeds?api-version=7.1",
            Some(&auth),
            None,
            "application/json",
            Some(&format!(r#"{{"name":"{name}"}}"#)),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{body}");
    }
    let publish = |feed: &'static str, name: &'static str, version: &'static str, content: &'static str| {
        let app = app.clone();
        let auth = auth.clone();
        async move {
            call(
                &app,
                "PUT",
                &format!("/azurz/AzurzDemo/_apis/packaging/feeds/{feed}/upack/packages/{name}/versions/{version}?api-version=7.1"),
                Some(&auth),
                None,
                "application/octet-stream",
                Some(content),
            )
            .await
        }
    };
    let (status, body, _) = publish("ligne-upack", "module", "1.0.0", "secret-module").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body, "secret-module");
    let (status, body, _) = publish("ligne-upack", "module", "1.1.0", "secret-voisin").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status, body, _) = publish("ligne-upack", "pompe", "1.0.0", "secret-pompe").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status, body, _) = publish("autre-upack", "module", "1.0.0", "secret-autre").await;
    assert_eq!(status, StatusCode::OK, "{body}");

    let (status, body, _) = call(
        &app,
        "GET",
        "/azurz/AzurzDemo/_apis/packaging/feeds/ligne-upack/upack/packages?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(!body.contains("secret-module"), "{body}");
    let listed: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(listed["count"], 3);
    assert!(listed["value"].as_array().unwrap().iter().all(|item| item.get("content").is_none()));
    assert!(listed["value"].as_array().unwrap().iter().any(|item| item["name"] == "module" && item["version"] == "1.0.0"));
    assert!(listed["value"].as_array().unwrap().iter().any(|item| item["name"] == "module" && item["version"] == "1.1.0"));

    let (status, body, _) = call(
        &app,
        "POST",
        "/azurz/_apis/hooks/subscriptions?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"publisherId":"tfs","eventType":"packaging.package.deleted","consumerId":"webHooks","consumerActionId":"httpRequest","consumerInputs":{"url":"/_hooks/inbox"},"publisherInputs":{"projectId":"AzurzDemo"}}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");

    let (status, _, _) = call(
        &app,
        "DELETE",
        "/azurz/AzurzDemo/_apis/packaging/feeds/ligne-upack/upack/packages/module/versions/1.0.0?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    let (status, body, _) = call(
        &app,
        "GET",
        "/azurz/AzurzDemo/_apis/packaging/feeds/ligne-upack/upack/packages/module/versions/1.0.0?api-version=7.1",
        Some(&auth),
        None,
        "application/octet-stream",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    assert!(body.contains("paquet universel introuvable"), "{body}");

    let (status, body, _) = call(
        &app,
        "GET",
        "/azurz/AzurzDemo/_apis/packaging/feeds/ligne-upack/upack/packages/module/versions/1.1.0?api-version=7.1",
        Some(&auth),
        None,
        "application/octet-stream",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body, "secret-voisin");

    let (status, body, _) = call(
        &app,
        "GET",
        "/azurz/AzurzDemo/_apis/packaging/feeds/ligne-upack/upack/packages?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(!body.contains("secret-"), "{body}");
    let left: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(left["count"], 2);
    assert!(left["value"].as_array().unwrap().iter().all(|item| item["version"] != "1.0.0" || item["name"] != "module"));
    assert!(left["value"].as_array().unwrap().iter().any(|item| item["name"] == "module" && item["version"] == "1.1.0"));
    assert!(left["value"].as_array().unwrap().iter().any(|item| item["name"] == "pompe" && item["version"] == "1.0.0"));

    let (status, body, _) = call(
        &app,
        "GET",
        "/azurz/AzurzDemo/_apis/packaging/feeds/autre-upack/upack/packages/module/versions/1.0.0?api-version=7.1",
        Some(&auth),
        None,
        "application/octet-stream",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body, "secret-autre");

    let (status, body, _) = call(
        &app,
        "DELETE",
        "/azurz/AzurzDemo/_apis/packaging/feeds/ligne-upack/upack/packages/module/versions/1.0.0?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    assert!(body.contains("paquet universel introuvable"), "{body}");

    let (status, body, _) = call(&app, "GET", "/azurz/_apis/hooks/inbox?api-version=7.1", Some(&auth), None, "application/json", None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(!body.contains("secret-module"), "{body}");
    let inbox: serde_json::Value = serde_json::from_str(&body).unwrap();
    let notes: Vec<_> = inbox["value"].as_array().unwrap().iter().filter(|item| item["eventType"] == "packaging.package.deleted").collect();
    assert_eq!(notes.len(), 1);
    assert_eq!(notes[0]["resource"]["protocol"], "upack");
    assert_eq!(notes[0]["resource"]["name"], "module");
    assert_eq!(notes[0]["resource"]["version"], "1.0.0");
    assert!(notes[0]["resource"].get("content").is_none());
}

#[tokio::test]
async fn nuget_package_version_can_be_removed() {
    let app = app();
    let auth = basic("azurz", "azurz-lab-pat");
    let (status, body, _) = call(
        &app,
        "DELETE",
        "/azurz/AzurzDemo/_packaging/absent/nuget/v3/flat2/oee/1.0.0/oee.1.0.0.nupkg",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    assert!(body.contains("flux introuvable"), "{body}");

    let (status, body, _) = call(
        &app,
        "GET",
        "/azurz/AzurzDemo/_apis/packaging/feeds/absent/nuget/packages?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    assert!(body.contains("flux introuvable"), "{body}");

    for name in ["ligne-nuget", "autre-nuget"] {
        let (status, body, _) = call(
            &app,
            "POST",
            "/azurz/AzurzDemo/_apis/packaging/feeds?api-version=7.1",
            Some(&auth),
            None,
            "application/json",
            Some(&format!(r#"{{"name":"{name}"}}"#)),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{body}");
    }
    let publish = |feed: &'static str, name: &'static str, version: &'static str, content: &'static str| {
        let app = app.clone();
        let auth = auth.clone();
        async move {
            let filename = format!("{name}.{version}.nupkg");
            call(
                &app,
                "PUT",
                &format!("/azurz/AzurzDemo/_packaging/{feed}/nuget/v3/flat2/{name}/{version}/{filename}"),
                Some(&auth),
                None,
                "application/octet-stream",
                Some(content),
            )
            .await
        }
    };
    let (status, body, _) = publish("ligne-nuget", "oee", "1.0.0", "secret-nupkg").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body, "secret-nupkg");
    let (status, body, _) = publish("ligne-nuget", "oee", "1.1.0", "secret-voisin").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status, body, _) = publish("ligne-nuget", "pompe", "1.0.0", "secret-pompe").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status, body, _) = publish("autre-nuget", "oee", "1.0.0", "secret-autre").await;
    assert_eq!(status, StatusCode::OK, "{body}");

    let (status, body, _) = call(
        &app,
        "GET",
        "/azurz/AzurzDemo/_apis/packaging/feeds/ligne-nuget/nuget/packages?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(!body.contains("secret-nupkg"), "{body}");
    let listed: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(listed["count"], 3);
    assert!(listed["value"].as_array().unwrap().iter().all(|item| item.get("content").is_none()));
    assert!(listed["value"].as_array().unwrap().iter().any(|item| item["name"] == "oee" && item["version"] == "1.0.0"));
    assert!(listed["value"].as_array().unwrap().iter().any(|item| item["name"] == "oee" && item["version"] == "1.1.0"));

    let (status, body, _) = call(
        &app,
        "GET",
        "/azurz/AzurzDemo/_packaging/ligne-nuget/nuget/v3/flat2/oee/index.json",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let versions: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(versions["versions"], serde_json::json!(["1.0.0", "1.1.0"]));

    let (status, body, _) = call(
        &app,
        "POST",
        "/azurz/_apis/hooks/subscriptions?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"publisherId":"tfs","eventType":"packaging.package.deleted","consumerId":"webHooks","consumerActionId":"httpRequest","consumerInputs":{"url":"/_hooks/inbox"},"publisherInputs":{"projectId":"AzurzDemo"}}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");

    let (status, _, _) = call(
        &app,
        "DELETE",
        "/azurz/AzurzDemo/_packaging/ligne-nuget/nuget/v3/flat2/OEE/1.0.0/oee.1.0.0.nupkg",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    let (status, body, _) = call(
        &app,
        "GET",
        "/azurz/AzurzDemo/_packaging/ligne-nuget/nuget/v3/flat2/oee/1.0.0/oee.1.0.0.nupkg",
        Some(&auth),
        None,
        "application/octet-stream",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    assert!(body.contains("paquet NuGet introuvable"), "{body}");

    let (status, body, _) = call(
        &app,
        "GET",
        "/azurz/AzurzDemo/_packaging/ligne-nuget/nuget/v3/flat2/oee/1.1.0/oee.1.1.0.nupkg",
        Some(&auth),
        None,
        "application/octet-stream",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body, "secret-voisin");

    let (status, body, _) = call(
        &app,
        "GET",
        "/azurz/AzurzDemo/_packaging/ligne-nuget/nuget/v3/flat2/oee/index.json",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let left_versions: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(left_versions["versions"], serde_json::json!(["1.1.0"]));

    let (status, body, _) = call(
        &app,
        "GET",
        "/azurz/AzurzDemo/_apis/packaging/feeds/ligne-nuget/nuget/packages?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(!body.contains("secret-"), "{body}");
    let left: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(left["count"], 2);
    assert!(left["value"].as_array().unwrap().iter().all(|item| item["version"] != "1.0.0" || item["name"] != "oee"));
    assert!(left["value"].as_array().unwrap().iter().any(|item| item["name"] == "oee" && item["version"] == "1.1.0"));
    assert!(left["value"].as_array().unwrap().iter().any(|item| item["name"] == "pompe" && item["version"] == "1.0.0"));

    let (status, body, _) = call(
        &app,
        "GET",
        "/azurz/AzurzDemo/_packaging/autre-nuget/nuget/v3/flat2/oee/1.0.0/oee.1.0.0.nupkg",
        Some(&auth),
        None,
        "application/octet-stream",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body, "secret-autre");

    let (status, body, _) = call(
        &app,
        "DELETE",
        "/azurz/AzurzDemo/_packaging/ligne-nuget/nuget/v3/flat2/oee/1.0.0/oee.1.0.0.nupkg",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    assert!(body.contains("paquet NuGet introuvable"), "{body}");

    let (status, body, _) = call(&app, "GET", "/azurz/_apis/hooks/inbox?api-version=7.1", Some(&auth), None, "application/json", None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(!body.contains("secret-nupkg"), "{body}");
    let inbox: serde_json::Value = serde_json::from_str(&body).unwrap();
    let notes: Vec<_> = inbox["value"].as_array().unwrap().iter().filter(|item| item["eventType"] == "packaging.package.deleted").collect();
    assert_eq!(notes.len(), 1);
    assert_eq!(notes[0]["resource"]["protocol"], "nuget");
    assert_eq!(notes[0]["resource"]["name"], "oee");
    assert_eq!(notes[0]["resource"]["version"], "1.0.0");
    assert!(notes[0]["resource"].get("content").is_none());
}

#[tokio::test]
async fn disabled_subscription_stays_silent_while_its_sibling_delivers() {
    let app = app();
    let auth = basic("azurz", "azurz-lab-pat");
    let (status, body, _) = call(
        &app,
        "PATCH",
        "/azurz/_apis/hooks/subscriptions/absent?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"status":"disabled"}"#),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    assert!(body.contains("abonnement introuvable"), "{body}");

    let subscribe = |app: axum::Router, auth: String| async move {
        call(
            &app,
            "POST",
            "/azurz/_apis/hooks/subscriptions?api-version=7.1",
            Some(&auth),
            None,
            "application/json",
            Some(r#"{"publisherId":"tfs","eventType":"workitem.created","consumerId":"webHooks","consumerActionId":"httpRequest","consumerInputs":{"url":"/_hooks/inbox"},"publisherInputs":{"projectId":"AzurzDemo"}}"#),
        )
        .await
    };
    let (status, body, _) = subscribe(app.clone(), auth.clone()).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let first: serde_json::Value = serde_json::from_str(&body).unwrap();
    let first_id = first["id"].as_str().unwrap().to_string();
    assert_eq!(first["status"], "enabled");
    let (status, body, _) = subscribe(app.clone(), auth.clone()).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let second: serde_json::Value = serde_json::from_str(&body).unwrap();
    let second_id = second["id"].as_str().unwrap().to_string();
    assert_ne!(first_id, second_id);

    let (status, body, _) = call(
        &app,
        "PATCH",
        &format!("/azurz/_apis/hooks/subscriptions/{first_id}?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"status":1}"#),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert!(body.contains("statut illisible"), "{body}");

    let (status, body, _) = call(
        &app,
        "PATCH",
        &format!("/azurz/_apis/hooks/subscriptions/{first_id}?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some(r#"{}"#),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert!(body.contains("statut absent"), "{body}");

    let (status, body, _) = call(
        &app,
        "PATCH",
        &format!("/azurz/_apis/hooks/subscriptions/{first_id}?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"status":"  "}"#),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert!(body.contains("statut absent"), "{body}");

    let (status, body, _) = call(
        &app,
        "PATCH",
        &format!("/azurz/_apis/hooks/subscriptions/{first_id}?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"status":"onProbation"}"#),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert!(body.contains("statut inconnu"), "{body}");

    let (status, body, _) = call(
        &app,
        "GET",
        &format!("/azurz/_apis/hooks/subscriptions/{first_id}?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let still: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(still["status"], "enabled");
    assert_eq!(still["eventType"], "workitem.created");

    let (status, body, _) = call(
        &app,
        "PATCH",
        &format!("/azurz/_apis/hooks/subscriptions/{first_id}?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"status":"Disabled"}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let disabled: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(disabled["status"], "disabled");
    assert_eq!(disabled["id"], first_id);
    assert_eq!(disabled["eventType"], "workitem.created");
    assert_eq!(disabled["consumerInputs"]["url"], "/_hooks/inbox");

    let (status, body, _) = call(
        &app,
        "POST",
        "/azurz/AzurzDemo/_apis/wit/workitems/$Task?api-version=7.1",
        Some(&auth),
        None,
        "application/json-patch+json",
        Some(r#"[{"op":"add","path":"/fields/System.Title","value":"Sonde muette"}]"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");

    let (status, body, _) = call(&app, "GET", "/azurz/_apis/hooks/inbox?api-version=7.1", Some(&auth), None, "application/json", None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let inbox: serde_json::Value = serde_json::from_str(&body).unwrap();
    let quiet: Vec<_> = inbox["value"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|item| item["eventType"] == "workitem.created" && item["resource"]["fields"]["System.Title"] == "Sonde muette")
        .collect();
    assert_eq!(quiet.len(), 1);
    assert_eq!(quiet[0]["subscriptionId"], second_id);

    let (status, body, _) = call(
        &app,
        "GET",
        &format!("/azurz/_apis/hooks/subscriptions/{first_id}/notifications?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let silent: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(silent["count"], 0);

    let (status, body, _) = call(
        &app,
        "GET",
        &format!("/azurz/_apis/hooks/subscriptions/{second_id}/notifications?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let heard: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(heard["count"], 1);
    assert_eq!(heard["value"][0]["resource"]["fields"]["System.Title"], "Sonde muette");

    let (status, body, _) = call(
        &app,
        "GET",
        &format!("/azurz/_apis/hooks/subscriptions/{second_id}?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let sibling: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(sibling["status"], "enabled");

    let (status, body, _) = call(
        &app,
        "PATCH",
        &format!("/azurz/_apis/hooks/subscriptions/{first_id}?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"status":"enabled"}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let resumed: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(resumed["status"], "enabled");

    let (status, body, _) = call(
        &app,
        "POST",
        "/azurz/AzurzDemo/_apis/wit/workitems/$Task?api-version=7.1",
        Some(&auth),
        None,
        "application/json-patch+json",
        Some(r#"[{"op":"add","path":"/fields/System.Title","value":"Sonde reprise"}]"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");

    let (status, body, _) = call(&app, "GET", "/azurz/_apis/hooks/inbox?api-version=7.1", Some(&auth), None, "application/json", None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let again: serde_json::Value = serde_json::from_str(&body).unwrap();
    let resumed_notes: Vec<_> = again["value"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|item| item["eventType"] == "workitem.created" && item["resource"]["fields"]["System.Title"] == "Sonde reprise")
        .collect();
    assert_eq!(resumed_notes.len(), 2);
    assert!(resumed_notes.iter().any(|item| item["subscriptionId"] == first_id));
    assert!(resumed_notes.iter().any(|item| item["subscriptionId"] == second_id));
}

#[tokio::test]
async fn removing_one_team_member_keeps_the_sibling() {
    let app = app();
    let auth = basic("azurz", "azurz-lab-pat");
    let (status, body, _) = call(
        &app,
        "DELETE",
        "/azurz/_apis/projects/AzurzDemo/teams/absent/members/azurz?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    assert!(body.contains("équipe introuvable"), "{body}");

    let (status, body, _) = call(
        &app,
        "POST",
        "/azurz/_apis/projects/AzurzDemo/teams?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"name":"Ligne Pompe","description":"Atelier"}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let line: serde_json::Value = serde_json::from_str(&body).unwrap();
    let line_id = line["id"].as_str().unwrap();

    let (status, body, _) = call(
        &app,
        "POST",
        "/azurz/_apis/projects/AzurzDemo/teams?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"name":"Ligne Voisine","description":"Atelier voisin"}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let other: serde_json::Value = serde_json::from_str(&body).unwrap();
    let other_id = other["id"].as_str().unwrap();

    for (team_id, payload) in [
        (line_id, r#"{"displayName":"Pompe","uniqueName":"pompe@azurz.lab","isTeamAdmin":false}"#),
        (line_id, r#"{"displayName":"Ada","uniqueName":"ada@azurz.lab","isTeamAdmin":false}"#),
        (other_id, r#"{"displayName":"Pompe","uniqueName":"pompe@azurz.lab","isTeamAdmin":false}"#),
    ] {
        let (status, body, _) = call(
            &app,
            "POST",
            &format!("/azurz/_apis/projects/AzurzDemo/teams/{team_id}/members?api-version=7.1"),
            Some(&auth),
            None,
            "application/json",
            Some(payload),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{body}");
    }

    let (status, body, _) = call(
        &app,
        "DELETE",
        &format!("/azurz/_apis/projects/AzurzDemo/teams/{line_id}/members?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::METHOD_NOT_ALLOWED, "{body}");
    assert!(body.contains("méthode refusée"), "{body}");

    let (status, body, _) = call(
        &app,
        "DELETE",
        &format!("/azurz/_apis/projects/AzurzDemo/teams/{line_id}/members/pompe@azurz.lab/extra?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    assert!(body.contains("membre introuvable"), "{body}");

    let (status, body, _) = call(
        &app,
        "DELETE",
        &format!("/azurz/_apis/projects/AzurzDemo/teams/{line_id}/members/inconnu@azurz.lab?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    assert!(body.contains("membre introuvable"), "{body}");

    let (status, body, _) = call(
        &app,
        "GET",
        &format!("/azurz/_apis/projects/AzurzDemo/teams/{line_id}/members?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let before: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(before["count"], 3);

    let (status, body, _) = call(
        &app,
        "DELETE",
        &format!("/azurz/_apis/projects/AzurzDemo/teams/{line_id}/members/POMPE@azurz.lab?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT, "{body}");

    let (status, body, _) = call(
        &app,
        "GET",
        &format!("/azurz/_apis/projects/AzurzDemo/teams/{line_id}/members?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let kept: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(kept["count"], 2);
    let names: Vec<_> = kept["value"]
        .as_array()
        .unwrap()
        .iter()
        .map(|item| item["identity"]["uniqueName"].as_str().unwrap().to_string())
        .collect();
    assert!(names.iter().any(|name| name == "azurz"));
    assert!(names.iter().any(|name| name == "ada@azurz.lab"));
    assert!(!names.iter().any(|name| name == "pompe@azurz.lab"));

    let (status, body, _) = call(
        &app,
        "GET",
        &format!("/azurz/_apis/projects/AzurzDemo/teams/{other_id}/members?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let sibling_team: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(sibling_team["count"], 2);
    assert!(sibling_team["value"]
        .as_array()
        .unwrap()
        .iter()
        .any(|item| item["identity"]["uniqueName"] == "pompe@azurz.lab"));

    let (status, body, _) = call(
        &app,
        "DELETE",
        &format!("/azurz/_apis/projects/AzurzDemo/teams/{line_id}/members/pompe@azurz.lab?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    assert!(body.contains("membre introuvable"), "{body}");

    let (status, body, _) = call(
        &app,
        "DELETE",
        &format!("/azurz/_apis/projects/AzurzDemo/teams/{line_id}/members/ada@azurz.lab?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT, "{body}");
    let (status, body, _) = call(
        &app,
        "DELETE",
        &format!("/azurz/_apis/projects/AzurzDemo/teams/{line_id}/members/azurz?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT, "{body}");
    let (status, body, _) = call(
        &app,
        "GET",
        &format!("/azurz/_apis/projects/AzurzDemo/teams/{line_id}/members?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let empty: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(empty["count"], 0);
    let (status, body, _) = call(
        &app,
        "GET",
        &format!("/azurz/_apis/projects/AzurzDemo/teams/{line_id}?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(serde_json::from_str::<serde_json::Value>(&body).unwrap()["name"], "Ligne Pompe");

    let (status, body, _) = call(
        &app,
        "POST",
        "/azurz/_apis/projects/AzurzDemo/teams/1/members?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"displayName":"Sonde","uniqueName":"sonde@azurz.lab","isTeamAdmin":false}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status, body, _) = call(
        &app,
        "DELETE",
        "/azurz/_apis/projects/AzurzDemo/teams/1/members/sonde@azurz.lab?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT, "{body}");
    let (status, body, _) = call(
        &app,
        "GET",
        "/azurz/_apis/projects/AzurzDemo/teams/1/members?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let default_members: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert!(!default_members["value"]
        .as_array()
        .unwrap()
        .iter()
        .any(|item| item["identity"]["uniqueName"] == "sonde@azurz.lab"));
    let (status, body, _) = call(
        &app,
        "DELETE",
        "/azurz/_apis/projects/AzurzDemo/teams/1?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert!(body.contains("équipe par défaut"), "{body}");
    let (status, body, _) = call(
        &app,
        "GET",
        "/azurz/_apis/projects/AzurzDemo/teams/1?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(serde_json::from_str::<serde_json::Value>(&body).unwrap()["id"], "1");
}

#[tokio::test]
async fn removing_one_branch_keeps_the_sibling_and_the_default() {
    let app = app();
    let auth = basic("azurz", "azurz-lab-pat");
    let zeros = "0000000000000000000000000000000000000000";
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
        &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/refs?filter=heads&api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let heads: serde_json::Value = serde_json::from_str(&body).unwrap();
    let main = heads["value"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["name"] == "refs/heads/main")
        .unwrap()["objectId"]
        .as_str()
        .unwrap()
        .to_string();

    for name in ["refs/heads/pompe", "refs/heads/ada"] {
        let (status, body, _) = call(
            &app,
            "POST",
            &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/refs?api-version=7.1"),
            Some(&auth),
            None,
            "application/json",
            Some(&format!(r#"[{{"name":"{name}","oldObjectId":"{zeros}","newObjectId":"{main}"}}]"#)),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{body}");
    }

    let (status, body, _) = call(
        &app,
        "POST",
        &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/refs?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some(&format!(r#"[{{"name":"refs/heads/MAIN","oldObjectId":"{main}","newObjectId":"{zeros}"}}]"#)),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert!(body.contains("branche par défaut"), "{body}");

    let (status, body, _) = call(
        &app,
        "POST",
        &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/refs?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some(&format!(
            r#"[{{"name":"refs/heads/pompe","oldObjectId":"{main}","newObjectId":"{zeros}"}},{{"name":"refs/heads/main","oldObjectId":"{main}","newObjectId":"{zeros}"}}]"#
        )),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert!(body.contains("branche par défaut"), "{body}");

    let (status, body, _) = call(
        &app,
        "GET",
        &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/refs?filter=heads&api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let before: serde_json::Value = serde_json::from_str(&body).unwrap();
    let before_names: Vec<_> = before["value"]
        .as_array()
        .unwrap()
        .iter()
        .map(|item| item["name"].as_str().unwrap().to_string())
        .collect();
    assert!(before_names.iter().any(|name| name == "refs/heads/main"));
    assert!(before_names.iter().any(|name| name == "refs/heads/pompe"));
    assert!(before_names.iter().any(|name| name == "refs/heads/ada"));

    let (status, body, _) = call(
        &app,
        "POST",
        &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/refs?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some(&format!(r#"[{{"name":"refs/heads/POMPE","oldObjectId":"{main}","newObjectId":"{zeros}"}}]"#)),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let removed: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(removed["value"][0]["success"], true);
    assert_eq!(removed["value"][0]["updateStatus"], "succeeded");

    let (status, body, _) = call(
        &app,
        "GET",
        &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/refs?filter=heads&api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let kept: serde_json::Value = serde_json::from_str(&body).unwrap();
    let names: Vec<_> = kept["value"]
        .as_array()
        .unwrap()
        .iter()
        .map(|item| item["name"].as_str().unwrap().to_string())
        .collect();
    assert!(names.iter().any(|name| name == "refs/heads/main"));
    assert!(names.iter().any(|name| name == "refs/heads/ada"));
    assert!(!names.iter().any(|name| name.eq_ignore_ascii_case("refs/heads/pompe")));

    let (status, body, _) = call(
        &app,
        "POST",
        &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/refs?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some(&format!(r#"[{{"name":"refs/heads/pompe","oldObjectId":"{main}","newObjectId":"{zeros}"}}]"#)),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let again: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(again["value"][0]["success"], false);
    assert_eq!(again["value"][0]["updateStatus"], "invalidRefName");
    let (status, body, _) = call(
        &app,
        "GET",
        &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/refs?filter=heads&api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(body.contains("refs/heads/ada"), "{body}");
    assert!(body.contains("refs/heads/main"), "{body}");
}

#[tokio::test]
async fn removing_one_team_iteration_keeps_the_sibling_and_the_current() {
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
    let seeded: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(seeded["count"], 1);
    assert_eq!(seeded["value"][0]["name"], "Sprint 1");
    assert_eq!(seeded["value"][0]["attributes"]["timeFrame"], "current");

    let (status, body, _) = call(
        &app,
        "POST",
        "/azurz/AzurzDemo/_apis/work/teamsettings/iterations?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        Some("[]"),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert!(body.contains("itération illisible"), "{body}");
    let (status, body, _) = call(
        &app,
        "POST",
        "/azurz/AzurzDemo/_apis/work/teamsettings/iterations?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        Some("{}"),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert!(body.contains("itération absente"), "{body}");
    let (status, body, _) = call(
        &app,
        "POST",
        "/azurz/AzurzDemo/_apis/work/teamsettings/iterations?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"id":3}"#),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert!(body.contains("itération illisible"), "{body}");

    let (status, body, _) = call(
        &app,
        "POST",
        "/azurz/AzurzDemo/_apis/work/teamsettings/iterations?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"id":"iteration"}"#),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert!(body.contains("itération racine"), "{body}");
    let (status, body, _) = call(
        &app,
        "POST",
        "/azurz/AzurzDemo/_apis/work/teamsettings/iterations?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"id":"node-3"}"#),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert!(body.contains("itération déjà retenue"), "{body}");
    let (status, body, _) = call(
        &app,
        "POST",
        "/azurz/AzurzDemo/_apis/work/teamsettings/iterations?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"id":"node-999"}"#),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    assert!(body.contains("itération introuvable"), "{body}");

    for (name, start, finish) in [
        ("Pompe", "2026-11-09", "2026-11-20"),
        ("Ada", "2026-12-01", "2026-12-12"),
        ("Revue", "2026-10-26", "2026-11-06"),
        ("Hiver", "2026-10-01", "2026-10-10"),
        ("Brouillon", "2026-11-01", "pas-une-date"),
    ] {
        let (status, body, _) = call(
            &app,
            "POST",
            "/azurz/AzurzDemo/_apis/wit/classificationnodes/iterations?api-version=7.1",
            Some(&auth),
            None,
            "application/json",
            Some(&format!(r#"{{"name":"{name}","attributes":{{"startDate":"{start}","finishDate":"{finish}"}}}}"#)),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{body}");
        let node: serde_json::Value = serde_json::from_str(&body).unwrap();
        let identifier = node["identifier"].as_str().unwrap().to_string();
        let (status, body, _) = call(
            &app,
            "POST",
            "/azurz/AzurzDemo/_apis/work/teamsettings/iterations?api-version=7.1",
            Some(&auth),
            None,
            "application/json",
            Some(&format!(r#"{{"id":"{identifier}"}}"#)),
        )
        .await;
        if name == "Brouillon" {
            assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
            assert!(body.contains("date illisible"), "{body}");
        } else {
            assert_eq!(status, StatusCode::OK, "{body}");
            let added: serde_json::Value = serde_json::from_str(&body).unwrap();
            assert_eq!(added["name"], name);
            let frame = match name {
                "Revue" => "current",
                "Hiver" => "past",
                _ => "future",
            };
            assert_eq!(added["attributes"]["timeFrame"], frame, "{body}");
        }
    }

    let (status, body, _) = call(
        &app,
        "POST",
        "/azurz/AzurzDemo/_apis/work/teamsettings/iterations?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"id":"pompe"}"#),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert!(body.contains("itération déjà retenue"), "{body}");

    let (status, body, _) = call(
        &app,
        "DELETE",
        "/azurz/AzurzDemo/_apis/work/teamsettings/iterations?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::METHOD_NOT_ALLOWED, "{body}");
    assert!(body.contains("méthode refusée"), "{body}");
    let (status, body, _) = call(
        &app,
        "DELETE",
        "/azurz/AzurzDemo/_apis/work/teamsettings/iterations/Revue?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert!(body.contains("itération courante"), "{body}");
    let (status, body, _) = call(
        &app,
        "DELETE",
        "/azurz/AzurzDemo/_apis/work/teamsettings/iterations/SPRINT%201?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert!(body.contains("itération courante"), "{body}");

    let (status, body, _) = call(
        &app,
        "DELETE",
        "/azurz/AzurzDemo/_apis/work/teamsettings/iterations/POMPE?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT, "{body}");
    let (status, body, _) = call(
        &app,
        "DELETE",
        "/azurz/AzurzDemo/_apis/work/teamsettings/iterations/POMPE?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    assert!(body.contains("itération introuvable"), "{body}");

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
    let kept: serde_json::Value = serde_json::from_str(&body).unwrap();
    let names: Vec<_> = kept["value"].as_array().unwrap().iter().map(|item| item["name"].as_str().unwrap().to_string()).collect();
    assert!(names.iter().any(|name| name == "Sprint 1"), "{names:?}");
    assert!(names.iter().any(|name| name == "Ada"), "{names:?}");
    assert!(names.iter().any(|name| name == "Revue"), "{names:?}");
    assert!(names.iter().any(|name| name == "Hiver"), "{names:?}");
    assert!(!names.iter().any(|name| name.eq_ignore_ascii_case("pompe")), "{names:?}");
    assert!(!names.iter().any(|name| name == "Brouillon"), "{names:?}");

    let (status, body, _) = call(
        &app,
        "GET",
        "/azurz/AzurzDemo/_apis/wit/classificationnodes/iterations?$depth=2&api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(body.contains("Pompe"), "{body}");
}

#[tokio::test]
async fn removing_one_saved_query_keeps_the_sibling_and_the_root_folders() {
    let app = app();
    let auth = basic("azurz", "azurz-lab-pat");
    let (status, body, _) = call(
        &app,
        "GET",
        "/azurz/AzurzDemo/_apis/wit/queries?$depth=2&api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let tree: serde_json::Value = serde_json::from_str(&body).unwrap();
    let names: Vec<&str> = tree["value"].as_array().unwrap().iter().filter_map(|item| item["name"].as_str()).collect();
    assert!(names.contains(&"Shared Queries"), "{body}");
    assert!(names.contains(&"My Queries"), "{body}");

    for root in ["shared-queries", "my-queries"] {
        let (status, body, _) = call(
            &app,
            "DELETE",
            &format!("/azurz/AzurzDemo/_apis/wit/queries/{root}?api-version=7.1"),
            Some(&auth),
            None,
            "application/json",
            None,
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
        assert!(body.contains("dossier racine"), "{body}");
    }
    let (status, body, _) = call(
        &app,
        "DELETE",
        "/azurz/AzurzDemo/_apis/wit/queries/query-999?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    assert!(body.contains("requête introuvable"), "{body}");

    let (status, body, _) = call(
        &app,
        "POST",
        "/azurz/AzurzDemo/_apis/wit/queries/shared-queries?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"name":"Pompe","wiql":"SELECT [System.Id] FROM WorkItems WHERE [System.Title] = 'Pompe'"}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let pompe: serde_json::Value = serde_json::from_str(&body).unwrap();
    let pompe_id = pompe["id"].as_str().unwrap();
    let (status, body, _) = call(
        &app,
        "POST",
        "/azurz/AzurzDemo/_apis/wit/queries/shared-queries?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"name":"Ada","wiql":"SELECT [System.Id] FROM WorkItems WHERE [System.Title] = 'Ada'"}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let ada: serde_json::Value = serde_json::from_str(&body).unwrap();
    let ada_id = ada["id"].as_str().unwrap();

    let (status, body, _) = call(
        &app,
        "DELETE",
        &format!("/azurz/AzurzDemo/_apis/wit/queries/{pompe_id}?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT, "{body}");
    let (status, body, _) = call(
        &app,
        "DELETE",
        &format!("/azurz/AzurzDemo/_apis/wit/queries/{pompe_id}?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    assert!(body.contains("requête introuvable"), "{body}");
    let (status, body, _) = call(
        &app,
        "GET",
        &format!("/azurz/AzurzDemo/_apis/wit/queries/{ada_id}?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(body.contains("Ada"), "{body}");

    let (status, body, _) = call(
        &app,
        "POST",
        "/azurz/AzurzDemo/_apis/wit/queries/shared-queries?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"name":"Ligne","isFolder":true}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let ligne: serde_json::Value = serde_json::from_str(&body).unwrap();
    let ligne_id = ligne["id"].as_str().unwrap();
    let (status, body, _) = call(
        &app,
        "POST",
        &format!("/azurz/AzurzDemo/_apis/wit/queries/{ligne_id}?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"name":"Cave","isFolder":true}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let cave: serde_json::Value = serde_json::from_str(&body).unwrap();
    let cave_id = cave["id"].as_str().unwrap();
    let (status, body, _) = call(
        &app,
        "POST",
        &format!("/azurz/AzurzDemo/_apis/wit/queries/{cave_id}?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"name":"Goutte","wiql":"SELECT [System.Id] FROM WorkItems"}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let goutte: serde_json::Value = serde_json::from_str(&body).unwrap();
    let goutte_id = goutte["id"].as_str().unwrap();
    let (status, body, _) = call(
        &app,
        "POST",
        "/azurz/AzurzDemo/_apis/wit/queries/shared-queries?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"name":"Atelier","isFolder":true}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let atelier: serde_json::Value = serde_json::from_str(&body).unwrap();
    let atelier_id = atelier["id"].as_str().unwrap();
    let (status, body, _) = call(
        &app,
        "POST",
        &format!("/azurz/AzurzDemo/_apis/wit/queries/{atelier_id}?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"name":"Bruit","wiql":"SELECT [System.Id] FROM WorkItems WHERE [System.Title] = 'Bruit'"}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let bruit: serde_json::Value = serde_json::from_str(&body).unwrap();
    let bruit_id = bruit["id"].as_str().unwrap();

    let (status, body, _) = call(
        &app,
        "DELETE",
        &format!("/azurz/AzurzDemo/_apis/wit/queries/{ligne_id}?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT, "{body}");
    for gone in [ligne_id, cave_id, goutte_id] {
        let (status, body, _) = call(
            &app,
            "GET",
            &format!("/azurz/AzurzDemo/_apis/wit/queries/{gone}?api-version=7.1"),
            Some(&auth),
            None,
            "application/json",
            None,
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{gone} {body}");
    }
    let (status, body, _) = call(
        &app,
        "DELETE",
        &format!("/azurz/AzurzDemo/_apis/wit/queries/{ligne_id}?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    assert!(body.contains("requête introuvable"), "{body}");

    for kept in [ada_id, atelier_id, bruit_id, "shared-queries", "my-queries"] {
        let (status, body, _) = call(
            &app,
            "GET",
            &format!("/azurz/AzurzDemo/_apis/wit/queries/{kept}?api-version=7.1"),
            Some(&auth),
            None,
            "application/json",
            None,
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{kept} {body}");
    }
    let (status, body, _) = call(
        &app,
        "GET",
        "/azurz/AzurzDemo/_apis/wit/queries?$depth=3&api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(body.contains("Ada"), "{body}");
    assert!(body.contains("Atelier"), "{body}");
    assert!(body.contains("Bruit"), "{body}");
    assert!(!body.contains("Pompe"), "{body}");
    assert!(!body.contains("Ligne"), "{body}");
    assert!(!body.contains("Goutte"), "{body}");
}

#[tokio::test]
async fn removing_one_test_run_keeps_the_sibling_and_its_results() {
    let app = app();
    let auth = basic("azurz", "azurz-lab-pat");
    let (status, body, _) = call(&app, "GET", "/azurz/AzurzDemo/_apis/test/runs?api-version=7.1", Some(&auth), None, "application/json", None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let empty: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(empty["count"], 0);

    let (status, body, _) = call(&app, "DELETE", "/azurz/AzurzDemo/_apis/test/runs?api-version=7.1", Some(&auth), None, "application/json", None).await;
    assert_eq!(status, StatusCode::METHOD_NOT_ALLOWED, "{body}");
    assert!(body.contains("méthode refusée"), "{body}");
    let (status, body, _) = call(&app, "DELETE", "/azurz/AzurzDemo/_apis/test/runs/999?api-version=7.1", Some(&auth), None, "application/json", None).await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    assert!(body.contains("exécution introuvable"), "{body}");

    let (status, body, _) = call(
        &app,
        "POST",
        "/azurz/AzurzDemo/_apis/testplan/plans?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"name":"Recette sonde"}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let plan: serde_json::Value = serde_json::from_str(&body).unwrap();
    let plan_id = plan["id"].as_i64().unwrap();
    let root_id = plan["rootSuite"]["id"].as_i64().unwrap();
    let (status, body, _) = call(
        &app,
        "POST",
        &format!("/azurz/AzurzDemo/_apis/testplan/plans/{plan_id}/suites?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some(&format!(r#"{{"name":"Ligne","parentSuite":{{"id":{root_id}}}}}"#)),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let suite: serde_json::Value = serde_json::from_str(&body).unwrap();
    let suite_id = suite["id"].as_i64().unwrap();
    let (status, body, _) = call(
        &app,
        "POST",
        &format!("/azurz/AzurzDemo/_apis/testplan/plans/{plan_id}/suites/{suite_id}/testcases?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some(r#"[{"title":"Debit sonde"}]"#),
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
    let point_id = points["value"][0]["id"].as_i64().unwrap();

    let (status, body, _) = call(
        &app,
        "POST",
        "/azurz/AzurzDemo/_apis/test/runs?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        Some(&format!(r#"{{"name":"Pompe","plan":{{"id":{plan_id}}}}}"#)),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let pompe: serde_json::Value = serde_json::from_str(&body).unwrap();
    let pompe_id = pompe["id"].as_i64().unwrap();
    let (status, body, _) = call(
        &app,
        "POST",
        &format!("/azurz/AzurzDemo/_apis/test/runs/{pompe_id}/results?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some(&format!(r#"[{{"testPoint":{{"id":{point_id}}},"outcome":"Passed","testCase":{{"name":"Debit sonde"}}}}]"#)),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");

    let (status, body, _) = call(
        &app,
        "POST",
        "/azurz/AzurzDemo/_apis/test/runs?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        Some(&format!(r#"{{"name":"Ada","plan":{{"id":{plan_id}}}}}"#)),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let ada: serde_json::Value = serde_json::from_str(&body).unwrap();
    let ada_id = ada["id"].as_i64().unwrap();
    let (status, body, _) = call(
        &app,
        "POST",
        &format!("/azurz/AzurzDemo/_apis/test/runs/{ada_id}/results?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some(r#"[{"outcome":"Failed","testCase":{"name":"Bruit"}}]"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");

    let (status, body, _) = call(
        &app,
        "DELETE",
        &format!("/azurz/AzurzDemo/_apis/test/runs/{pompe_id}?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT, "{body}");
    let (status, body, _) = call(
        &app,
        "DELETE",
        &format!("/azurz/AzurzDemo/_apis/test/runs/{pompe_id}?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    assert!(body.contains("exécution introuvable"), "{body}");
    let (status, body, _) = call(
        &app,
        "GET",
        &format!("/azurz/AzurzDemo/_apis/test/runs/{pompe_id}?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    let (status, body, _) = call(
        &app,
        "GET",
        &format!("/azurz/AzurzDemo/_apis/test/runs/{pompe_id}/results?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    assert!(body.contains("exécution introuvable"), "{body}");

    let (status, body, _) = call(
        &app,
        "GET",
        &format!("/azurz/AzurzDemo/_apis/test/runs/{ada_id}?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(body.contains("Ada"), "{body}");
    let (status, body, _) = call(
        &app,
        "GET",
        &format!("/azurz/AzurzDemo/_apis/test/runs/{ada_id}/results?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(body.contains("Bruit"), "{body}");
    assert!(body.contains("Failed"), "{body}");

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
    let kept_points: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(kept_points["value"][0]["outcome"], "Passed");

    let (status, body, _) = call(&app, "GET", "/azurz/AzurzDemo/_apis/test/runs?api-version=7.1", Some(&auth), None, "application/json", None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let kept: serde_json::Value = serde_json::from_str(&body).unwrap();
    let names: Vec<_> = kept["value"].as_array().unwrap().iter().map(|item| item["name"].as_str().unwrap().to_string()).collect();
    assert!(names.iter().any(|name| name == "Ada"), "{names:?}");
    assert!(!names.iter().any(|name| name == "Pompe"), "{names:?}");
}

#[tokio::test]
async fn removing_one_pull_request_reviewer_keeps_the_sibling() {
    let app = app();
    let auth = basic("azurz", "azurz-lab-pat");
    let (status, body, _) = call(
        &app,
        "POST",
        "/azurz/AzurzDemo/_apis/git/pullrequests?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"title":"Ligne","sourceRefName":"refs/heads/feature","targetRefName":"refs/heads/main"}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let pull: serde_json::Value = serde_json::from_str(&body).unwrap();
    let pull_id = pull["pullRequestId"].as_i64().unwrap();
    let repo_id = pull["repository"]["id"].as_str().unwrap();
    let path = format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/pullrequests/{pull_id}");
    let (status, body, _) = call(
        &app,
        "POST",
        &format!("{path}/reviewers?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"displayName":"Ada","uniqueName":"ada@azurz.lab","vote":5,"isRequired":false}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status, body, _) = call(
        &app,
        "POST",
        &format!("{path}/reviewers?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"displayName":"Pompe","uniqueName":"pompe@azurz.lab","vote":0,"isRequired":true}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");

    let (status, body, _) = call(
        &app,
        "POST",
        "/azurz/AzurzDemo/_apis/git/pullrequests?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"title":"Voisin","sourceRefName":"refs/heads/feature","targetRefName":"refs/heads/main"}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let voisin: serde_json::Value = serde_json::from_str(&body).unwrap();
    let voisin_id = voisin["pullRequestId"].as_i64().unwrap();
    let voisin_repo = voisin["repository"]["id"].as_str().unwrap();
    let voisin_path = format!("/azurz/AzurzDemo/_apis/git/repositories/{voisin_repo}/pullrequests/{voisin_id}");
    let (status, body, _) = call(
        &app,
        "POST",
        &format!("{voisin_path}/reviewers?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"displayName":"Pompe","uniqueName":"pompe@azurz.lab","vote":10}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");

    let (status, body, _) = call(
        &app,
        "DELETE",
        &format!("{path}/reviewers?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::METHOD_NOT_ALLOWED, "{body}");
    assert!(body.contains("méthode refusée"), "{body}");
    let (status, body, _) = call(
        &app,
        "DELETE",
        &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/pullrequests/999/reviewers/Pompe?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    assert!(body.contains("demande de tirage introuvable"), "{body}");
    let (status, body, _) = call(
        &app,
        "DELETE",
        &format!("{path}/reviewers/inconnu?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    assert!(body.contains("relecteur introuvable"), "{body}");
    let (status, body, _) = call(
        &app,
        "DELETE",
        &format!("{path}/reviewers/Pompe/extra?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    assert!(body.contains("relecteur introuvable"), "{body}");
    let (status, body, _) = call(
        &app,
        "GET",
        &format!("{path}/reviewers?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let before: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(before.as_array().unwrap().len(), 2, "{body}");

    let (status, body, _) = call(
        &app,
        "POST",
        "/azurz/_apis/hooks/subscriptions?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"publisherId":"tfs","eventType":"git.pullrequest.updated","consumerId":"webHooks","consumerActionId":"httpRequest","consumerInputs":{"url":"/_hooks/inbox"},"publisherInputs":{"projectId":"AzurzDemo","repository":"AzurzDemo"}}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let updated_hook: serde_json::Value = serde_json::from_str(&body).unwrap();
    let updated_id = updated_hook["id"].as_str().unwrap();
    let (status, body, _) = call(
        &app,
        "DELETE",
        &format!("{path}/reviewers/POMPE@azurz.lab?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT, "{body}");
    assert!(body.is_empty(), "{body}");
    let (status, body, _) = call(
        &app,
        "DELETE",
        &format!("{path}/reviewers/POMPE@azurz.lab?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    assert!(body.contains("relecteur introuvable"), "{body}");

    let (status, body, _) = call(
        &app,
        "GET",
        &format!("{path}/reviewers?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let kept: serde_json::Value = serde_json::from_str(&body).unwrap();
    let rows = kept.as_array().unwrap();
    assert_eq!(rows.len(), 1, "{body}");
    assert_eq!(rows[0]["displayName"], "Ada");
    assert_eq!(rows[0]["uniqueName"], "ada@azurz.lab");
    assert_eq!(rows[0]["vote"], 5);
    assert_eq!(rows[0]["isRequired"], false);
    let (status, body, _) = call(
        &app,
        "GET",
        &format!("{path}?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let still: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(still["status"], "active");
    let (status, body, _) = call(
        &app,
        "GET",
        &format!("/azurz/_apis/hooks/subscriptions/{updated_id}/notifications?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let notes: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(notes["count"], 1, "{body}");
    assert_eq!(notes["value"][0]["eventType"], "git.pullrequest.updated");

    let (status, body, _) = call(
        &app,
        "GET",
        &format!("{voisin_path}/reviewers?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let voisin_rows: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(voisin_rows.as_array().unwrap().len(), 1, "{body}");
    assert_eq!(voisin_rows[0]["displayName"], "Pompe");
    assert_eq!(voisin_rows[0]["vote"], 10);

    let (status, body, _) = call(
        &app,
        "POST",
        "/azurz/AzurzDemo/_apis/git/pullrequests?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"title":"Auto","sourceRefName":"refs/heads/feature","targetRefName":"refs/heads/main"}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let auto: serde_json::Value = serde_json::from_str(&body).unwrap();
    let auto_id = auto["pullRequestId"].as_i64().unwrap();
    let auto_repo = auto["repository"]["id"].as_str().unwrap();
    let auto_path = format!("/azurz/AzurzDemo/_apis/git/repositories/{auto_repo}/pullrequests/{auto_id}");
    let (status, body, _) = call(
        &app,
        "POST",
        &format!("{auto_path}/reviewers?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"displayName":"Ada","uniqueName":"ada@azurz.lab","vote":10}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status, body, _) = call(
        &app,
        "POST",
        &format!("{auto_path}/reviewers?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"displayName":"Bruit","uniqueName":"bruit@azurz.lab","vote":0,"isRequired":true}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status, body, _) = call(
        &app,
        "PATCH",
        &format!("{auto_path}?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"autoCompleteSetBy":{"id":"azurz"},"completionOptions":{"mergeStrategy":"noFastForward","deleteSourceBranch":false}}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let armed: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(armed["status"], "active");
    let (status, body, _) = call(
        &app,
        "GET",
        &format!("/azurz/_apis/hooks/subscriptions/{updated_id}/notifications?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let before_merge: serde_json::Value = serde_json::from_str(&body).unwrap();
    let updated_before = before_merge["count"].as_i64().unwrap();
    let (status, body, _) = call(
        &app,
        "POST",
        "/azurz/_apis/hooks/subscriptions?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"publisherId":"tfs","eventType":"git.pullrequest.merged","consumerId":"webHooks","consumerActionId":"httpRequest","consumerInputs":{"url":"/_hooks/inbox"},"publisherInputs":{"projectId":"AzurzDemo","repository":"AzurzDemo"}}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let merged_hook: serde_json::Value = serde_json::from_str(&body).unwrap();
    let merged_id = merged_hook["id"].as_str().unwrap();
    let (status, body, _) = call(
        &app,
        "DELETE",
        &format!("{auto_path}/reviewers/bruit@azurz.lab?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT, "{body}");
    let (status, body, _) = call(
        &app,
        "GET",
        &format!("{auto_path}?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let merged: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(merged["status"], "completed");
    let (status, body, _) = call(
        &app,
        "GET",
        &format!("{auto_path}/reviewers?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let auto_rows: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(auto_rows.as_array().unwrap().len(), 1, "{body}");
    assert_eq!(auto_rows[0]["displayName"], "Ada");
    assert_eq!(auto_rows[0]["vote"], 10);
    let (status, body, _) = call(
        &app,
        "GET",
        &format!("/azurz/_apis/hooks/subscriptions/{merged_id}/notifications?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let merged_notes: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(merged_notes["count"], 1, "{body}");
    assert_eq!(merged_notes["value"][0]["eventType"], "git.pullrequest.merged");
    let (status, body, _) = call(
        &app,
        "GET",
        &format!("/azurz/_apis/hooks/subscriptions/{updated_id}/notifications?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let after_merge: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(after_merge["count"].as_i64().unwrap(), updated_before, "{body}");
}

#[tokio::test]
async fn removing_one_build_definition_keeps_the_sibling_and_its_builds() {
    let app = app();
    let auth = basic("azurz", "azurz-lab-pat");
    let trigger = r#"{"triggerType":"continuousIntegration","branchFilters":["+refs/heads/main"],"pathFilters":["+/src/*"],"batchChanges":false}"#;
    let (status, body, _) = call(
        &app,
        "GET",
        "/azurz/AzurzDemo/_apis/build/definitions/1?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let seeded: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(seeded["name"], "CI");
    assert_eq!(seeded["triggers"], serde_json::json!([]));

    let (status, body, _) = call(
        &app,
        "POST",
        "/azurz/AzurzDemo/_apis/build/definitions?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        Some(&format!(r#"{{"name":"Pompe","triggers":[{trigger}],"variables":{{"token":{{"value":"secret-pompe","isSecret":true}}}}}}"#)),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let pompe: serde_json::Value = serde_json::from_str(&body).unwrap();
    let pompe_id = pompe["id"].as_i64().unwrap();
    assert_eq!(pompe["name"], "Pompe");
    let (status, body, _) = call(
        &app,
        "POST",
        "/azurz/AzurzDemo/_apis/build/definitions?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        Some(&format!(r#"{{"name":"Ada","triggers":[{trigger}],"variables":{{"host":{{"value":"atelier","isSecret":false}}}}}}"#)),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let ada: serde_json::Value = serde_json::from_str(&body).unwrap();
    let ada_id = ada["id"].as_i64().unwrap();
    assert_ne!(ada_id, pompe_id);

    let (status, body, _) = call(
        &app,
        "POST",
        "/azurz/AzurzDemo/_apis/build/builds?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        Some(&format!(r#"{{"definition":{{"id":{pompe_id}}},"sourceBranch":"refs/heads/main","reason":"manual"}}"#)),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let queued: serde_json::Value = serde_json::from_str(&body).unwrap();
    let build_id = queued["id"].as_i64().unwrap();
    assert_eq!(queued["definition"]["name"], "Pompe");
    assert_eq!(queued["reason"], "manual");
    assert_eq!(queued["variables"]["token"]["value"], "");
    assert_eq!(queued["variables"]["token"]["isSecret"], true);

    let (status, body, _) = call(
        &app,
        "DELETE",
        "/azurz/AzurzDemo/_apis/build/definitions?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::METHOD_NOT_ALLOWED, "{body}");
    assert!(body.contains("méthode refusée"), "{body}");
    let (status, body, _) = call(
        &app,
        "DELETE",
        "/azurz/AzurzDemo/_apis/build/definitions/999?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    assert!(body.contains("définition introuvable"), "{body}");
    let (status, body, _) = call(
        &app,
        "DELETE",
        &format!("/azurz/AzurzDemo/_apis/build/definitions/{pompe_id}/extra?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::METHOD_NOT_ALLOWED, "{body}");
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
    let before: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(before["count"], 3, "{body}");

    let (status, body, _) = call(
        &app,
        "DELETE",
        &format!("/azurz/AzurzDemo/_apis/build/definitions/{pompe_id}?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT, "{body}");
    assert!(body.is_empty(), "{body}");
    let (status, body, _) = call(
        &app,
        "DELETE",
        &format!("/azurz/AzurzDemo/_apis/build/definitions/{pompe_id}?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    assert!(body.contains("définition introuvable"), "{body}");
    let (status, body, _) = call(
        &app,
        "GET",
        &format!("/azurz/AzurzDemo/_apis/build/definitions/{pompe_id}?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");

    let (status, body, _) = call(
        &app,
        "GET",
        &format!("/azurz/AzurzDemo/_apis/build/definitions/{ada_id}?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let kept: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(kept["name"], "Ada");
    assert_eq!(kept["queueStatus"], "enabled");
    assert_eq!(kept["triggers"][0]["triggerType"], "continuousIntegration");
    assert_eq!(kept["variables"]["host"]["value"], "atelier");
    assert_eq!(kept["variables"]["host"]["isSecret"], false);
    let (status, body, _) = call(
        &app,
        "GET",
        "/azurz/AzurzDemo/_apis/build/definitions/1?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let still_seed: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(still_seed["name"], "CI");
    assert_eq!(still_seed["triggers"], serde_json::json!([]));

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
    let kept_build: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(kept_build["definition"]["id"], pompe_id);
    assert_eq!(kept_build["definition"]["name"], "Pompe");
    assert_eq!(kept_build["reason"], "manual");
    assert_eq!(kept_build["variables"]["token"]["value"], "");

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
        &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/refs?filter=heads&api-version=7.1"),
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
        .unwrap();
    let (status, body, _) = call(
        &app,
        "POST",
        &format!("/azurz/AzurzDemo/_apis/git/repositories/{repo_id}/pushes?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some(&format!(
            r#"{{"refUpdates":[{{"name":"refs/heads/main","oldObjectId":"{old}"}}],"commits":[{{"comment":"sonde","changes":[{{"changeType":"add","item":{{"path":"/src/pipeline.txt"}},"newContent":{{"content":"sonde","contentType":"rawtext"}}}}]}}]}}"#
        )),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status, body, _) = call(
        &app,
        "GET",
        "/azurz/AzurzDemo/_apis/build/builds?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let builds: serde_json::Value = serde_json::from_str(&body).unwrap();
    let rows = builds["value"].as_array().unwrap();
    assert_eq!(rows.len(), 2, "{body}");
    assert!(rows.iter().any(|item| item["definition"]["name"] == "Pompe" && item["reason"] == "manual"), "{body}");
    assert!(rows.iter().any(|item| item["definition"]["id"] == ada_id && item["reason"] == "individualCI"), "{body}");
    assert!(!rows.iter().any(|item| item["definition"]["name"] == "Pompe" && item["reason"] == "individualCI"), "{body}");
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
    let listed: serde_json::Value = serde_json::from_str(&body).unwrap();
    let names: Vec<_> = listed["value"].as_array().unwrap().iter().map(|item| item["name"].as_str().unwrap().to_string()).collect();
    assert!(names.iter().any(|name| name == "CI"), "{names:?}");
    assert!(names.iter().any(|name| name == "Ada"), "{names:?}");
    assert!(!names.iter().any(|name| name == "Pompe"), "{names:?}");
}

#[tokio::test]
async fn removing_one_feed_keeps_the_sibling_and_its_packages() {
    let app = app();
    let auth = basic("azurz", "azurz-lab-pat");
    for name in ["Pompe", "Ada"] {
        let (status, body, _) = call(
            &app,
            "POST",
            "/azurz/AzurzDemo/_apis/packaging/feeds?api-version=7.1",
            Some(&auth),
            None,
            "application/json",
            Some(&format!(r#"{{"name":"{name}"}}"#)),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{body}");
        let created: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(created["name"], name);
    }
    let publish = |feed: &'static str, content: &'static str| {
        let app = app.clone();
        let auth = auth.clone();
        async move {
            call(
                &app,
                "PUT",
                &format!("/azurz/AzurzDemo/_apis/packaging/feeds/{feed}/upack/packages/module/versions/1.0.0?api-version=7.1"),
                Some(&auth),
                None,
                "application/octet-stream",
                Some(content),
            )
            .await
        }
    };
    let (status, body, _) = publish("Pompe", "secret-pompe").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body, "secret-pompe");
    let (status, body, _) = publish("Ada", "secret-ada").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body, "secret-ada");

    let (status, body, _) = call(
        &app,
        "DELETE",
        "/azurz/AzurzDemo/_apis/packaging/feeds?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::METHOD_NOT_ALLOWED, "{body}");
    assert!(body.contains("méthode refusée"), "{body}");
    let (status, body, _) = call(
        &app,
        "DELETE",
        "/azurz/AzurzDemo/_apis/packaging/feeds/feed-99999999?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    assert!(body.contains("flux introuvable"), "{body}");
    let (status, body, _) = call(
        &app,
        "DELETE",
        "/azurz/AzurzDemo/_apis/packaging/feeds/Pompe/extra?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::METHOD_NOT_ALLOWED, "{body}");
    assert!(body.contains("méthode refusée"), "{body}");
    let (status, body, _) = call(
        &app,
        "GET",
        "/azurz/AzurzDemo/_apis/packaging/feeds?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let before: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(before["count"], 2);

    let (status, _, _) = call(
        &app,
        "DELETE",
        "/azurz/AzurzDemo/_apis/packaging/feeds/Pompe?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (status, body, _) = call(
        &app,
        "DELETE",
        "/azurz/AzurzDemo/_apis/packaging/feeds/Pompe?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    assert!(body.contains("flux introuvable"), "{body}");

    let (status, body, _) = call(
        &app,
        "GET",
        "/azurz/AzurzDemo/_apis/packaging/feeds/Pompe?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    assert!(body.contains("flux introuvable"), "{body}");
    let (status, body, _) = call(
        &app,
        "GET",
        "/azurz/AzurzDemo/_apis/packaging/feeds/Pompe/upack/packages/module/versions/1.0.0?api-version=7.1",
        Some(&auth),
        None,
        "application/octet-stream",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    assert!(body.contains("flux introuvable"), "{body}");

    let (status, body, _) = call(
        &app,
        "GET",
        "/azurz/AzurzDemo/_apis/packaging/feeds/Ada?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let kept: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(kept["name"], "Ada");
    let (status, body, _) = call(
        &app,
        "GET",
        "/azurz/AzurzDemo/_apis/packaging/feeds/Ada/upack/packages/module/versions/1.0.0?api-version=7.1",
        Some(&auth),
        None,
        "application/octet-stream",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body, "secret-ada");
    let (status, body, _) = call(
        &app,
        "GET",
        "/azurz/AzurzDemo/_apis/packaging/feeds/Ada/upack/packages?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(!body.contains("secret-"), "{body}");
    let packages: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(packages["count"], 1);
    assert_eq!(packages["value"][0]["name"], "module");
    assert_eq!(packages["value"][0]["version"], "1.0.0");

    let (status, body, _) = call(
        &app,
        "GET",
        "/azurz/AzurzDemo/_apis/packaging/feeds?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let listed: serde_json::Value = serde_json::from_str(&body).unwrap();
    let names: Vec<_> = listed["value"].as_array().unwrap().iter().map(|item| item["name"].as_str().unwrap().to_string()).collect();
    assert!(names.iter().any(|name| name == "Ada"), "{names:?}");
    assert!(!names.iter().any(|name| name == "Pompe"), "{names:?}");
}

#[tokio::test]
async fn removing_one_release_definition_keeps_the_sibling_and_its_releases() {
    let app = app();
    let auth = basic("azurz", "azurz-lab-pat");
    let mut pompe_id = 0_i64;
    for (name, environment) in [("Pompe", "Ligne"), ("Ada", "Atelier")] {
        let (status, body, _) = call(
            &app,
            "POST",
            "/azurz/AzurzDemo/_apis/release/definitions?api-version=7.1",
            Some(&auth),
            None,
            "application/json",
            Some(&format!(r#"{{"name":"{name}","environments":[{{"name":"{environment}","rank":1}}]}}"#)),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{body}");
        let created: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(created["name"], name);
        assert_eq!(created["environments"][0]["name"], environment);
        if name == "Pompe" {
            pompe_id = created["id"].as_i64().unwrap();
        }
    }
    let (status, body, _) = call(
        &app,
        "POST",
        "/azurz/AzurzDemo/_apis/release/releases?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        Some(&format!(r#"{{"definitionId":{pompe_id},"description":"1.0.0"}}"#)),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let release: serde_json::Value = serde_json::from_str(&body).unwrap();
    let release_id = release["id"].as_i64().unwrap();
    assert_eq!(release["status"], "active");
    assert_eq!(release["releaseDefinition"]["name"], "Pompe");
    assert_eq!(release["releaseDefinition"]["id"], pompe_id);

    let (status, body, _) = call(
        &app,
        "DELETE",
        "/azurz/AzurzDemo/_apis/release/definitions?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::METHOD_NOT_ALLOWED, "{body}");
    assert!(body.contains("méthode refusée"), "{body}");
    let (status, body, _) = call(
        &app,
        "DELETE",
        "/azurz/AzurzDemo/_apis/release/definitions/999?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    assert!(body.contains("définition introuvable"), "{body}");
    let (status, body, _) = call(
        &app,
        "DELETE",
        &format!("/azurz/AzurzDemo/_apis/release/definitions/{pompe_id}/extra?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::METHOD_NOT_ALLOWED, "{body}");
    assert!(body.contains("méthode refusée"), "{body}");
    let (status, body, _) = call(
        &app,
        "GET",
        "/azurz/AzurzDemo/_apis/release/definitions?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let before: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(before["count"], 2);

    let (status, _, _) = call(
        &app,
        "DELETE",
        "/azurz/AzurzDemo/_apis/release/definitions/Pompe?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (status, body, _) = call(
        &app,
        "DELETE",
        &format!("/azurz/AzurzDemo/_apis/release/definitions/{pompe_id}?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    assert!(body.contains("définition introuvable"), "{body}");

    let (status, body, _) = call(
        &app,
        "GET",
        "/azurz/AzurzDemo/_apis/release/definitions/Pompe?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    assert!(body.contains("définition introuvable"), "{body}");
    let (status, body, _) = call(
        &app,
        "GET",
        "/azurz/AzurzDemo/_apis/release/definitions/Ada?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let kept: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(kept["name"], "Ada");
    assert_eq!(kept["environments"][0]["name"], "Atelier");

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
    let kept_release: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(kept_release["status"], "active");
    assert_eq!(kept_release["description"], "1.0.0");
    assert_eq!(kept_release["releaseDefinition"]["id"], pompe_id);
    assert_eq!(kept_release["releaseDefinition"]["name"], "Pompe");

    let (status, body, _) = call(
        &app,
        "POST",
        "/azurz/AzurzDemo/_apis/release/releases?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        Some(&format!(r#"{{"definitionId":{pompe_id},"description":"absente"}}"#)),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    assert!(body.contains("définition introuvable"), "{body}");

    let (status, body, _) = call(
        &app,
        "GET",
        "/azurz/AzurzDemo/_apis/release/definitions?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let listed: serde_json::Value = serde_json::from_str(&body).unwrap();
    let names: Vec<_> = listed["value"].as_array().unwrap().iter().map(|item| item["name"].as_str().unwrap().to_string()).collect();
    assert!(names.iter().any(|name| name == "Ada"), "{names:?}");
    assert!(!names.iter().any(|name| name == "Pompe"), "{names:?}");
}

#[tokio::test]
async fn removing_one_test_plan_keeps_the_sibling_and_its_suites() {
    let app = app();
    let auth = basic("azurz", "azurz-lab-pat");
    let mut pompe_id = 0_i64;
    let mut ada_id = 0_i64;
    for name in ["Pompe", "Ada"] {
        let (status, body, _) = call(
            &app,
            "POST",
            "/azurz/AzurzDemo/_apis/testplan/plans?api-version=7.1",
            Some(&auth),
            None,
            "application/json",
            Some(&format!(r#"{{"name":"{name}"}}"#)),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{body}");
        let created: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(created["name"], name);
        if name == "Pompe" {
            pompe_id = created["id"].as_i64().unwrap();
        } else {
            ada_id = created["id"].as_i64().unwrap();
        }
    }
    let (status, body, _) = call(
        &app,
        "POST",
        &format!("/azurz/AzurzDemo/_apis/testplan/plans/{pompe_id}/suites?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some(&format!(r#"{{"name":"Ligne","parentSuite":{{"id":{pompe_id}}}}}"#)),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let pompe_suite: serde_json::Value = serde_json::from_str(&body).unwrap();
    let pompe_suite_id = pompe_suite["id"].as_i64().unwrap();
    let (status, body, _) = call(
        &app,
        "POST",
        &format!("/azurz/AzurzDemo/_apis/testplan/plans/{pompe_id}/suites/{pompe_suite_id}/testcases?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some(r#"[{"title":"Debit pompe"}]"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let pompe_cases: serde_json::Value = serde_json::from_str(&body).unwrap();
    let work_id = pompe_cases["value"][0]["workItem"]["id"].as_i64().unwrap();
    assert_eq!(pompe_cases["value"][0]["workItem"]["name"], "Debit pompe");
    let (status, body, _) = call(
        &app,
        "POST",
        "/azurz/AzurzDemo/_apis/test/runs?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        Some(&format!(r#"{{"name":"Pompe","plan":{{"id":{pompe_id}}}}}"#)),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let run: serde_json::Value = serde_json::from_str(&body).unwrap();
    let run_id = run["id"].as_i64().unwrap();
    assert_eq!(run["name"], "Pompe");

    let (status, body, _) = call(
        &app,
        "POST",
        &format!("/azurz/AzurzDemo/_apis/testplan/plans/{ada_id}/suites?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some(&format!(r#"{{"name":"Atelier","parentSuite":{{"id":{ada_id}}}}}"#)),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let ada_suite: serde_json::Value = serde_json::from_str(&body).unwrap();
    let ada_suite_id = ada_suite["id"].as_i64().unwrap();
    let (status, body, _) = call(
        &app,
        "POST",
        &format!("/azurz/AzurzDemo/_apis/testplan/plans/{ada_id}/suites/{ada_suite_id}/testcases?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some(r#"[{"title":"Debit ada"}]"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");

    let (status, body, _) = call(
        &app,
        "DELETE",
        "/azurz/AzurzDemo/_apis/testplan/plans?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::METHOD_NOT_ALLOWED, "{body}");
    assert!(body.contains("méthode refusée"), "{body}");
    let (status, body, _) = call(
        &app,
        "DELETE",
        "/azurz/AzurzDemo/_apis/testplan/plans/999?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    assert!(body.contains("plan introuvable"), "{body}");
    let (status, body, _) = call(
        &app,
        "DELETE",
        &format!("/azurz/AzurzDemo/_apis/testplan/plans/{pompe_id}/extra?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::METHOD_NOT_ALLOWED, "{body}");
    assert!(body.contains("méthode refusée"), "{body}");
    let (status, body, _) = call(
        &app,
        "GET",
        "/azurz/AzurzDemo/_apis/testplan/plans?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let before: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(before["count"], 2);

    let (status, _, _) = call(
        &app,
        "DELETE",
        "/azurz/AzurzDemo/_apis/testplan/plans/Pompe?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (status, body, _) = call(
        &app,
        "DELETE",
        &format!("/azurz/AzurzDemo/_apis/testplan/plans/{pompe_id}?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    assert!(body.contains("plan introuvable"), "{body}");

    let (status, body, _) = call(
        &app,
        "GET",
        &format!("/azurz/AzurzDemo/_apis/testplan/plans/{pompe_id}/suites?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    assert!(body.contains("plan introuvable"), "{body}");
    let (status, body, _) = call(
        &app,
        "POST",
        &format!("/azurz/AzurzDemo/_apis/testplan/plans/{pompe_id}/suites?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"name":"Absente"}"#),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    assert!(body.contains("plan introuvable"), "{body}");

    let (status, body, _) = call(
        &app,
        "GET",
        &format!("/azurz/AzurzDemo/_apis/wit/workitems/{work_id}?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let kept_case: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(kept_case["fields"]["System.Title"], "Debit pompe");
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
    assert_eq!(status, StatusCode::OK, "{body}");
    let kept_run: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(kept_run["name"], "Pompe");
    assert_eq!(kept_run["plan"]["id"], pompe_id);

    let (status, body, _) = call(
        &app,
        "GET",
        &format!("/azurz/AzurzDemo/_apis/testplan/plans/{ada_id}/suites?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let suites: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(suites["count"], 2);
    let suite_names: Vec<_> = suites["value"].as_array().unwrap().iter().map(|item| item["name"].as_str().unwrap().to_string()).collect();
    assert!(suite_names.iter().any(|name| name == "Ada"), "{suite_names:?}");
    assert!(suite_names.iter().any(|name| name == "Atelier"), "{suite_names:?}");
    let (status, body, _) = call(
        &app,
        "GET",
        &format!("/azurz/AzurzDemo/_apis/testplan/plans/{ada_id}/suites/{ada_suite_id}/testpoint?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let points: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(points["count"], 1);
    assert_eq!(points["value"][0]["testCase"]["name"], "Debit ada");
    assert_eq!(points["value"][0]["outcome"], "Unspecified");

    let (status, body, _) = call(
        &app,
        "GET",
        "/azurz/AzurzDemo/_apis/testplan/plans?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let listed: serde_json::Value = serde_json::from_str(&body).unwrap();
    let names: Vec<_> = listed["value"].as_array().unwrap().iter().map(|item| item["name"].as_str().unwrap().to_string()).collect();
    assert!(names.iter().any(|name| name == "Ada"), "{names:?}");
    assert!(!names.iter().any(|name| name == "Pompe"), "{names:?}");
}

#[tokio::test]
async fn removing_one_dashboard_keeps_the_sibling_and_its_widgets() {
    let app = app();
    let auth = basic("azurz", "azurz-lab-pat");
    let mut pompe_id = String::new();
    let mut ada_id = String::new();
    for name in ["Pompe", "Ada"] {
        let (status, body, _) = call(
            &app,
            "POST",
            "/azurz/AzurzDemo/_apis/dashboard/dashboards?api-version=7.1",
            Some(&auth),
            None,
            "application/json",
            Some(&format!(r#"{{"name":"{name}"}}"#)),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{body}");
        let created: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(created["name"], name);
        if name == "Pompe" {
            pompe_id = created["id"].as_str().unwrap().to_string();
        } else {
            ada_id = created["id"].as_str().unwrap().to_string();
        }
    }
    let (status, body, _) = call(
        &app,
        "POST",
        &format!("/azurz/AzurzDemo/_apis/dashboard/dashboards/{pompe_id}/widgets?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"name":"Note pompe","contributionId":"ms.vss-dashboards.MarkdownWidget","settings":{"content":"secret-pompe"}}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status, body, _) = call(
        &app,
        "POST",
        &format!("/azurz/AzurzDemo/_apis/dashboard/dashboards/{ada_id}/widgets?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"name":"Note ada","contributionId":"ms.vss-dashboards.MarkdownWidget","settings":{"content":"secret-ada"}}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let widget: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(widget["name"], "Note ada");
    assert_eq!(widget["rendered"]["text"], "secret-ada");

    let (status, body, _) = call(
        &app,
        "DELETE",
        "/azurz/AzurzDemo/_apis/dashboard/dashboards?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::METHOD_NOT_ALLOWED, "{body}");
    assert!(body.contains("méthode refusée"), "{body}");
    let (status, body, _) = call(
        &app,
        "DELETE",
        "/azurz/AzurzDemo/_apis/dashboard/dashboards/dash-999?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    assert!(body.contains("tableau de bord introuvable"), "{body}");
    let (status, body, _) = call(
        &app,
        "DELETE",
        &format!("/azurz/AzurzDemo/_apis/dashboard/dashboards/{pompe_id}/extra?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::METHOD_NOT_ALLOWED, "{body}");
    assert!(body.contains("méthode refusée"), "{body}");
    let (status, body, _) = call(
        &app,
        "GET",
        "/azurz/AzurzDemo/_apis/dashboard/dashboards?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let before: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(before["count"], 2);

    let (status, _, _) = call(
        &app,
        "DELETE",
        "/azurz/AzurzDemo/_apis/dashboard/dashboards/Pompe?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (status, body, _) = call(
        &app,
        "DELETE",
        &format!("/azurz/AzurzDemo/_apis/dashboard/dashboards/{pompe_id}?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    assert!(body.contains("tableau de bord introuvable"), "{body}");

    let (status, body, _) = call(
        &app,
        "GET",
        &format!("/azurz/AzurzDemo/_apis/dashboard/dashboards/{pompe_id}/widgets?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    assert!(body.contains("tableau de bord introuvable"), "{body}");

    let (status, body, _) = call(
        &app,
        "GET",
        &format!("/azurz/AzurzDemo/_apis/dashboard/dashboards/{ada_id}?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let kept: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(kept["name"], "Ada");
    assert_eq!(kept["widgets"].as_array().unwrap().len(), 1);
    assert_eq!(kept["widgets"][0]["name"], "Note ada");
    assert_eq!(kept["widgets"][0]["rendered"]["text"], "secret-ada");
    let (status, body, _) = call(
        &app,
        "GET",
        &format!("/azurz/AzurzDemo/_apis/dashboard/dashboards/{ada_id}/widgets?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let widgets: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(widgets["count"], 1);
    assert_eq!(widgets["value"][0]["rendered"]["text"], "secret-ada");

    let (status, body, _) = call(
        &app,
        "GET",
        "/azurz/AzurzDemo/_apis/dashboard/dashboards?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let listed: serde_json::Value = serde_json::from_str(&body).unwrap();
    let names: Vec<_> = listed["value"].as_array().unwrap().iter().map(|item| item["name"].as_str().unwrap().to_string()).collect();
    assert!(names.iter().any(|name| name == "Ada"), "{names:?}");
    assert!(!names.iter().any(|name| name == "Pompe"), "{names:?}");
}

#[tokio::test]
async fn removing_one_environment_keeps_the_sibling_and_its_checks() {
    let app = app();
    let auth = basic("azurz", "azurz-lab-pat");
    let mut pompe_id = String::new();
    let mut ada_id = String::new();
    for (name, description) in [("Pompe", "Ligne pompe"), ("Ada", "Ligne ada")] {
        let (status, body, _) = call(
            &app,
            "POST",
            "/azurz/AzurzDemo/_apis/distributedtask/environments?api-version=7.1",
            Some(&auth),
            None,
            "application/json",
            Some(&format!(r#"{{"name":"{name}","description":"{description}"}}"#)),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{body}");
        let created: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(created["name"], name);
        assert_eq!(created["description"], description);
        if name == "Pompe" {
            pompe_id = created["id"].to_string();
        } else {
            ada_id = created["id"].to_string();
        }
    }
    let (status, body, _) = call(
        &app,
        "POST",
        "/azurz/AzurzDemo/_apis/pipelines/checks/configurations?api-version=7.1-preview.1",
        Some(&auth),
        None,
        "application/json",
        Some(&format!(
            r#"{{"type":{{"id":"8c6f20a7-a545-4486-9777-f762fafe0d4d","name":"Approval"}},"settings":{{"approvers":[{{"displayName":"azurz"}}]}},"resource":{{"type":"environment","id":"{pompe_id}","name":"Pompe"}}}}"#
        )),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status, body, _) = call(
        &app,
        "POST",
        "/azurz/AzurzDemo/_apis/pipelines/checks/configurations?api-version=7.1-preview.1",
        Some(&auth),
        None,
        "application/json",
        Some(&format!(
            r#"{{"type":{{"id":"fe1de3ee-a436-41b4-bb20-f6eb4cb879a7","name":"Task Check"}},"settings":{{"displayName":"Branch control","inputs":{{"allowedBranches":"refs/heads/main"}}}},"resource":{{"type":"environment","id":"{ada_id}","name":"Ada"}}}}"#
        )),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let ada_check: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(ada_check["type"]["name"], "Task Check");

    let (status, body, _) = call(
        &app,
        "DELETE",
        "/azurz/AzurzDemo/_apis/distributedtask/environments?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::METHOD_NOT_ALLOWED, "{body}");
    assert!(body.contains("méthode refusée"), "{body}");
    let (status, body, _) = call(
        &app,
        "DELETE",
        "/azurz/AzurzDemo/_apis/distributedtask/environments/999?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    assert!(body.contains("environnement introuvable"), "{body}");
    let (status, body, _) = call(
        &app,
        "DELETE",
        &format!("/azurz/AzurzDemo/_apis/distributedtask/environments/{pompe_id}/extra?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::METHOD_NOT_ALLOWED, "{body}");
    assert!(body.contains("méthode refusée"), "{body}");
    let (status, body, _) = call(
        &app,
        "GET",
        "/azurz/AzurzDemo/_apis/distributedtask/environments?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let still: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(still["count"], 2, "{body}");

    let (status, body, _) = call(
        &app,
        "DELETE",
        "/azurz/AzurzDemo/_apis/distributedtask/environments/Pompe?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT, "{body}");
    let (status, body, _) = call(
        &app,
        "DELETE",
        &format!("/azurz/AzurzDemo/_apis/distributedtask/environments/{pompe_id}?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    assert!(body.contains("environnement introuvable"), "{body}");

    let (status, body, _) = call(
        &app,
        "GET",
        &format!("/azurz/AzurzDemo/_apis/pipelines/checks/configurations?resourceType=environment&resourceId={pompe_id}&api-version=7.1-preview.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let gone: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(gone["count"], 0, "{body}");
    let (status, body, _) = call(
        &app,
        "POST",
        "/azurz/AzurzDemo/_apis/pipelines/checks/configurations?api-version=7.1-preview.1",
        Some(&auth),
        None,
        "application/json",
        Some(&format!(
            r#"{{"type":{{"id":"8c6f20a7-a545-4486-9777-f762fafe0d4d"}},"settings":{{"approvers":[{{"displayName":"azurz"}}]}},"resource":{{"type":"environment","id":"{pompe_id}"}}}}"#
        )),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert!(body.contains("environnement introuvable"), "{body}");

    let (status, body, _) = call(
        &app,
        "GET",
        &format!("/azurz/AzurzDemo/_apis/distributedtask/environments/{ada_id}?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let kept: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(kept["name"], "Ada");
    assert_eq!(kept["description"], "Ligne ada");
    let (status, body, _) = call(
        &app,
        "GET",
        &format!("/azurz/AzurzDemo/_apis/pipelines/checks/configurations?resourceType=environment&resourceId={ada_id}&api-version=7.1-preview.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let checks: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(checks["count"], 1, "{body}");
    assert_eq!(checks["value"][0]["type"]["name"], "Task Check");
    assert_eq!(checks["value"][0]["settings"]["inputs"]["allowedBranches"], "refs/heads/main");
    let (status, body, _) = call(
        &app,
        "GET",
        "/azurz/AzurzDemo/_apis/distributedtask/environments?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let listed: serde_json::Value = serde_json::from_str(&body).unwrap();
    let names: Vec<_> = listed["value"].as_array().unwrap().iter().map(|item| item["name"].as_str().unwrap().to_string()).collect();
    assert!(names.iter().any(|name| name == "Ada"), "{names:?}");
    assert!(!names.iter().any(|name| name == "Pompe"), "{names:?}");
}

#[tokio::test]
async fn removing_one_agent_keeps_the_sibling_and_its_job() {
    let app = app();
    let auth = basic("azurz", "azurz-lab-pat");
    let mut pompe_id = 0_i64;
    let mut ada_id = 0_i64;
    for (name, os) in [("Pompe", "Linux pompe"), ("Ada", "Linux ada")] {
        let (status, body, _) = call(
            &app,
            "POST",
            "/azurz/_apis/distributedtask/pools/1/agents?api-version=7.1",
            Some(&auth),
            None,
            "application/json",
            Some(&format!(r#"{{"name":"{name}","osDescription":"{os}","status":"offline","version":"3.248.0"}}"#)),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{body}");
        let created: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(created["name"], name);
        assert_eq!(created["osDescription"], os);
        if name == "Pompe" {
            pompe_id = created["id"].as_i64().unwrap();
        } else {
            ada_id = created["id"].as_i64().unwrap();
        }
    }
    let (status, body, _) = call(
        &app,
        "PATCH",
        &format!("/azurz/_apis/distributedtask/pools/1/agents/{ada_id}?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some(r#"{"status":"online","systemCapabilities":{"Agent.OS":"Linux"}}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status, body, _) = call(
        &app,
        "POST",
        "/azurz/_apis/distributedtask/pools/1/jobrequests?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        Some(&format!(r#"{{"agentId":{pompe_id},"planType":"Build","definition":{{"id":1,"name":"CI"}}}}"#)),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let request: serde_json::Value = serde_json::from_str(&body).unwrap();
    let request_id = request["requestId"].as_i64().unwrap();
    assert_eq!(request["reservedAgent"]["name"], "Pompe");
    assert_eq!(request["status"], "queued");

    let (status, body, _) = call(
        &app,
        "DELETE",
        "/azurz/_apis/distributedtask/pools/1/agents?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::METHOD_NOT_ALLOWED, "{body}");
    assert!(body.contains("méthode refusée"), "{body}");
    let (status, body, _) = call(
        &app,
        "DELETE",
        "/azurz/_apis/distributedtask/pools/9/agents/1?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    assert!(body.contains("pool introuvable"), "{body}");
    let (status, body, _) = call(
        &app,
        "DELETE",
        "/azurz/_apis/distributedtask/pools/1/agents/999?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    assert!(body.contains("agent introuvable"), "{body}");
    let (status, body, _) = call(
        &app,
        "DELETE",
        &format!("/azurz/_apis/distributedtask/pools/1/agents/{pompe_id}/extra?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::METHOD_NOT_ALLOWED, "{body}");
    assert!(body.contains("méthode refusée"), "{body}");
    let (status, body, _) = call(
        &app,
        "GET",
        "/azurz/_apis/distributedtask/pools/1/agents?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let still: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(still["count"], 2, "{body}");

    let (status, body, _) = call(
        &app,
        "DELETE",
        "/azurz/_apis/distributedtask/pools/1/agents/Pompe?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT, "{body}");
    let (status, body, _) = call(
        &app,
        "DELETE",
        &format!("/azurz/_apis/distributedtask/pools/1/agents/{pompe_id}?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    assert!(body.contains("agent introuvable"), "{body}");

    let (status, body, _) = call(
        &app,
        "GET",
        &format!("/azurz/_apis/distributedtask/pools/1/agents/{ada_id}?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let kept: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(kept["name"], "Ada");
    assert_eq!(kept["status"], "online");
    assert_eq!(kept["osDescription"], "Linux ada");
    assert_eq!(kept["systemCapabilities"]["Agent.OS"], "Linux");
    let (status, body, _) = call(
        &app,
        "GET",
        "/azurz/_apis/distributedtask/pools/1/jobrequests?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let jobs: serde_json::Value = serde_json::from_str(&body).unwrap();
    let job = jobs["value"].as_array().unwrap().iter().find(|item| item["requestId"].as_i64() == Some(request_id)).unwrap();
    assert_eq!(job["reservedAgent"]["name"], "Pompe");
    assert_eq!(job["status"], "queued");
    let (status, body, _) = call(
        &app,
        "POST",
        "/azurz/_apis/distributedtask/pools/1/jobrequests?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        Some(&format!(r#"{{"agentId":{pompe_id},"planType":"Build","definition":{{"id":1,"name":"CI"}}}}"#)),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    assert!(body.contains("agent introuvable"), "{body}");
    let (status, body, _) = call(
        &app,
        "GET",
        "/azurz/_apis/distributedtask/pools/1?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let pool: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(pool["name"], "Azure Pipelines");
    assert_eq!(pool["size"], 1);
    let (status, body, _) = call(
        &app,
        "GET",
        "/azurz/_apis/distributedtask/pools/1/agents?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let listed: serde_json::Value = serde_json::from_str(&body).unwrap();
    let names: Vec<_> = listed["value"].as_array().unwrap().iter().map(|item| item["name"].as_str().unwrap().to_string()).collect();
    assert!(names.iter().any(|name| name == "Ada"), "{names:?}");
    assert!(!names.iter().any(|name| name == "Pompe"), "{names:?}");
}

#[tokio::test]
async fn removing_one_yaml_pipeline_keeps_the_sibling_and_its_run() {
    let app = app();
    let auth = basic("azurz", "azurz-lab-pat");
    let mut pompe_id = 0_i64;
    let mut ada_id = 0_i64;
    for name in ["Pompe", "Ada"] {
        let (status, body, _) = call(
            &app,
            "POST",
            "/azurz/AzurzDemo/_apis/pipelines/pipelines?api-version=7.1",
            Some(&auth),
            None,
            "application/json",
            Some(&format!(r#"{{"name":"{name}"}}"#)),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{body}");
        let created: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(created["name"], name);
        if name == "Pompe" {
            pompe_id = created["id"].as_i64().unwrap();
        } else {
            ada_id = created["id"].as_i64().unwrap();
        }
    }
    let (status, body, _) = call(
        &app,
        "POST",
        &format!("/azurz/AzurzDemo/_apis/pipelines/pipelines/{ada_id}/runs?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some(r#"{}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let ada_run: serde_json::Value = serde_json::from_str(&body).unwrap();
    let ada_run_id = ada_run["id"].as_i64().unwrap();
    assert_eq!(ada_run["pipeline"]["name"], "Ada");
    assert_eq!(ada_run["name"], format!("Ada-{ada_run_id}"));
    let (status, body, _) = call(
        &app,
        "POST",
        &format!("/azurz/AzurzDemo/_apis/pipelines/pipelines/{pompe_id}/runs?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some(r#"{}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let pompe_run: serde_json::Value = serde_json::from_str(&body).unwrap();
    let pompe_run_id = pompe_run["id"].as_i64().unwrap();
    assert_eq!(pompe_run["pipeline"]["name"], "Pompe");

    let (status, body, _) = call(
        &app,
        "DELETE",
        "/azurz/AzurzDemo/_apis/pipelines/pipelines?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::METHOD_NOT_ALLOWED, "{body}");
    assert!(body.contains("méthode refusée"), "{body}");
    let (status, body, _) = call(
        &app,
        "DELETE",
        "/azurz/AzurzDemo/_apis/pipelines/pipelines/999?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    assert!(body.contains("pipeline introuvable"), "{body}");
    let (status, body, _) = call(
        &app,
        "DELETE",
        &format!("/azurz/AzurzDemo/_apis/pipelines/pipelines/{pompe_id}/extra?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::METHOD_NOT_ALLOWED, "{body}");
    assert!(body.contains("méthode refusée"), "{body}");
    let (status, body, _) = call(
        &app,
        "GET",
        "/azurz/AzurzDemo/_apis/pipelines/pipelines?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let still: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(still["count"], 3, "{body}");

    let (status, body, _) = call(
        &app,
        "DELETE",
        "/azurz/AzurzDemo/_apis/pipelines/pipelines/Pompe?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT, "{body}");
    let (status, body, _) = call(
        &app,
        "DELETE",
        &format!("/azurz/AzurzDemo/_apis/pipelines/pipelines/{pompe_id}?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    assert!(body.contains("pipeline introuvable"), "{body}");

    let (status, body, _) = call(
        &app,
        "GET",
        &format!("/azurz/AzurzDemo/_apis/pipelines/pipelines/{ada_id}?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let kept: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(kept["name"], "Ada");
    assert_eq!(kept["folder"], "\\");
    let (status, body, _) = call(
        &app,
        "GET",
        "/azurz/AzurzDemo/_apis/pipelines/runs?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let runs: serde_json::Value = serde_json::from_str(&body).unwrap();
    let ada = runs["value"].as_array().unwrap().iter().find(|item| item["id"].as_i64() == Some(ada_run_id)).unwrap();
    let pompe = runs["value"].as_array().unwrap().iter().find(|item| item["id"].as_i64() == Some(pompe_run_id)).unwrap();
    assert_eq!(ada["pipeline"]["name"], "Ada");
    assert_eq!(ada["state"], "completed");
    assert_eq!(ada["result"], "succeeded");
    assert_eq!(pompe["pipeline"]["name"], "Pompe");
    assert_eq!(pompe["name"], format!("Pompe-{pompe_run_id}"));
    let (status, body, _) = call(
        &app,
        "POST",
        &format!("/azurz/AzurzDemo/_apis/pipelines/pipelines/{pompe_id}/runs?api-version=7.1"),
        Some(&auth),
        None,
        "application/json",
        Some(r#"{}"#),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    assert!(body.contains("pipeline introuvable"), "{body}");
    let (status, body, _) = call(
        &app,
        "GET",
        "/azurz/AzurzDemo/_apis/pipelines/pipelines?api-version=7.1",
        Some(&auth),
        None,
        "application/json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let listed: serde_json::Value = serde_json::from_str(&body).unwrap();
    let names: Vec<_> = listed["value"].as_array().unwrap().iter().map(|item| item["name"].as_str().unwrap().to_string()).collect();
    assert!(names.iter().any(|name| name == "Ada"), "{names:?}");
    assert!(names.iter().any(|name| name == "CI"), "{names:?}");
    assert!(!names.iter().any(|name| name == "Pompe"), "{names:?}");
}
