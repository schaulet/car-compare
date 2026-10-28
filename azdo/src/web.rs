pub fn login_page(org: &str, rejected: bool) -> String {
    let error = if rejected {
        r#"<p class="error" role="alert">Jeton d'accès personnel refusé.</p>"#
    } else {
        ""
    };
    format!(
        r#"<!DOCTYPE html>
<html lang="fr">
<head>
  <meta charset="utf-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1" />
  <title>Connexion — Azure DevOps</title>
  <style>
    :root {{ color-scheme: light; }}
    * {{ box-sizing: border-box; }}
    body {{
      margin: 0; min-height: 100vh; display: grid; place-items: center;
      background: #f3f2f1; color: #323130;
      font-family: "Segoe UI", "Segoe UI Web", sans-serif;
    }}
    .card {{
      width: min(420px, calc(100% - 32px)); background: #fff;
      border: 1px solid #edebe9; box-shadow: 0 8px 24px rgba(0,0,0,.08);
      padding: 36px 32px 28px;
    }}
    .mark {{ width: 40px; height: 40px; margin-bottom: 16px; background:
      linear-gradient(135deg, #0078d4 0 50%, #50e6ff 50% 100%); }}
    h1 {{ margin: 0 0 4px; font-size: 24px; font-weight: 600; }}
    p {{ margin: 0 0 18px; color: #605e5c; }}
    label {{ display: block; font-size: 14px; font-weight: 600; margin: 14px 0 6px; }}
    input {{
      width: 100%; height: 36px; border: 1px solid #8a8886; padding: 0 8px; font: inherit;
    }}
    input:focus {{ outline: 2px solid #0078d4; border-color: #0078d4; }}
    button {{
      margin-top: 22px; height: 36px; width: 100%; border: 0; background: #0078d4;
      color: #fff; font: inherit; font-weight: 600; cursor: pointer;
    }}
    button:hover {{ background: #106ebe; }}
    .error {{ color: #a4262c; font-weight: 600; }}
  </style>
</head>
<body>
  <main class="card">
    <div class="mark" aria-hidden="true"></div>
    <h1>Azure DevOps</h1>
    <p>Organisation <strong>{org}</strong>. Le mot de passe est le jeton d'accès personnel. Le nom d'utilisateur peut rester vide.</p>
    {error}
    <form method="post" action="/_signin">
      <label for="username">Nom d'utilisateur</label>
      <input id="username" name="username" autocomplete="username" />
      <label for="password">Mot de passe</label>
      <input id="password" name="password" type="password" autocomplete="current-password" />
      <button type="submit">Se connecter</button>
    </form>
  </main>
</body>
</html>"#
    )
}

pub fn shell_page(org: &str, project: &str, user: &str) -> String {
    include_str!("../assets/shell.html")
        .replace("__ORG__", org)
        .replace("__PROJECT__", project)
        .replace("__USER__", user)
}

