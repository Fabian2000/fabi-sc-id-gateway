//! HTML templates for the admin UI.

/// Base HTML layout.
fn layout(title: &str, content: &str) -> String {
    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>{title} - Gateway Admin</title>
    <link rel="stylesheet" href="/_static/css/main.css?v=5">
</head>
<body>
    {content}
    <script src="/_static/js/main.js?v=4"></script>
</body>
</html>"#
    )
}

/// Navigation component.
fn nav(active: &str) -> String {
    let items = [
        ("/_admin", "Dashboard", "dashboard"),
        ("/_admin/routes", "Routes", "routes"),
        ("/_admin/settings", "Settings", "settings"),
    ];

    let links: String = items
        .iter()
        .map(|(href, label, id)| {
            let class = if *id == active { "active" } else { "" };
            format!(r#"<a href="{href}" class="{class}">{label}</a>"#)
        })
        .collect::<Vec<_>>()
        .join("\n            ");

    format!(
        r#"<nav class="sidebar">
        <div class="logo">
            <h1>Gateway</h1>
        </div>
        <div class="nav-links">
            {links}
        </div>
        <div class="nav-footer">
            <form action="/_admin/logout" method="POST">
                <button type="submit" class="btn-logout">Logout</button>
            </form>
        </div>
    </nav>"#
    )
}

/// Dashboard page.
pub fn dashboard(username: &str) -> String {
    layout(
        "Dashboard",
        &format!(
            r#"<div class="admin-layout">
        {nav}
        <main class="content">
            <header class="page-header">
                <h2>Dashboard</h2>
                <span class="user-info">Welcome, {username}</span>
            </header>
            <div class="dashboard-cards">
                <div class="card">
                    <h3>Routes</h3>
                    <p class="card-value" id="route-count">-</p>
                    <a href="/_admin/routes" class="card-link">Manage Routes</a>
                </div>
                <div class="card">
                    <h3>Status</h3>
                    <p class="card-value status-ok">Running</p>
                </div>
            </div>
        </main>
    </div>
    <script>
        fetch('/_admin/api/routes')
            .then(r => r.json())
            .then(routes => {{
                document.getElementById('route-count').textContent = routes.length;
            }});
    </script>"#,
            nav = nav("dashboard")
        ),
    )
}

/// Login page.
pub fn login(consent_url: &str) -> String {
    layout(
        "Login",
        &format!(
            r#"<div class="login-container">
        <div class="login-card">
            <h1>Gateway Admin</h1>
            <p>Sign in with your Fabi-SC ID account to continue.</p>
            <a href="{consent_url}" class="btn btn-primary">
                Sign in with Fabi-SC ID
            </a>
        </div>
    </div>"#
        ),
    )
}

/// Setup page.
pub fn setup() -> String {
    layout(
        "Setup",
        r#"<div class="setup-container">
        <div class="setup-card">
            <h1>Gateway Setup</h1>
            <p>Configure your Fabi-SC ID integration to get started.</p>
            <div class="setup-info">
                <p>Create an application at <a href="https://id.fabi-sc.com/docs/developers/register-app.html" target="_blank">id.fabi-sc.com</a>.</p>
                <p>Required scopes: <code>openid</code>, <code>username</code></p>
                <p>Callback URL: <code>https://&lt;your-admin-origin&gt;/_admin/callback</code></p>
            </div>
            <form action="/_admin/setup" method="POST" class="setup-form">
                <div class="form-group">
                    <label for="app_id">Application ID</label>
                    <input type="text" id="app_id" name="app_id" required>
                </div>
                <div class="form-group">
                    <label for="api_key">API Key</label>
                    <input type="password" id="api_key" name="api_key" required>
                </div>
                <div class="form-group">
                    <label for="admin_origin">Gateway URL</label>
                    <input type="url" id="admin_origin" name="admin_origin" placeholder="https://gateway.example.com" required>
                    <small>The public URL of this gateway (used for login popups)</small>
                </div>
                <div class="form-group">
                    <label for="admin_users">Admin Users</label>
                    <input type="text" id="admin_users" name="admin_users" required>
                    <small>Comma-separated Fabi-SC ID usernames</small>
                </div>
                <button type="submit" class="btn btn-primary">Save & Continue</button>
            </form>
        </div>
    </div>"#,
    )
}

/// Routes management page.
pub fn routes() -> String {
    layout(
        "Routes",
        &format!(
            r#"<div class="admin-layout">
        {nav}
        <main class="content">
            <header class="page-header">
                <h2>Routes</h2>
                <button class="btn btn-primary" onclick="showAddModal()">Add Route</button>
            </header>
            <div class="routes-list" id="routes-list">
                <p class="loading">Loading routes...</p>
            </div>
        </main>
    </div>

    <div id="route-modal" class="modal-backdrop hidden">
        <div class="modal">
            <div class="modal-header">
                <h3 class="modal-title" id="modal-title">Add Route</h3>
                <button type="button" class="modal-close" onclick="hideModal()">&times;</button>
            </div>
            <form id="route-form">
                <input type="hidden" id="route-id">
                <div class="modal-body">
                    <div class="form-group">
                        <label class="form-label" for="route-name">Name</label>
                        <input type="text" class="form-input" id="route-name" required>
                    </div>
                    <div class="form-group">
                        <label class="form-label" for="route-host">Host</label>
                        <input type="text" class="form-input" id="route-host" placeholder="app.example.com" required>
                    </div>
                    <div class="form-group">
                        <label class="form-label" for="route-upstream">Upstream URL</label>
                        <input type="url" class="form-input" id="route-upstream" placeholder="http://localhost:3000" required>
                    </div>
                    <div class="form-group">
                        <label class="checkbox-label">
                            <input type="checkbox" class="form-checkbox" id="route-auth">
                            <span class="checkbox-text">Require Authentication</span>
                        </label>
                    </div>
                    <div class="form-group" id="allowed-users-group" style="display: none;">
                        <label class="form-label" for="route-allowed-users">Allowed Users</label>
                        <input type="text" class="form-input" id="route-allowed-users" placeholder="user1, user2, user3">
                        <span class="form-hint">Comma-separated Fabi-SC ID usernames (leave empty for all authenticated users)</span>
                    </div>
                </div>
                <div class="modal-footer">
                    <button type="button" class="btn btn-secondary" onclick="hideModal()">Cancel</button>
                    <button type="submit" class="btn btn-primary">Save</button>
                </div>
            </form>
        </div>
    </div>"#,
            nav = nav("routes")
        ),
    )
}

/// Error page.
pub fn error_page(status: u16, title: &str) -> String {
    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>{status} - {title}</title>
    <style>
        * {{ margin: 0; padding: 0; box-sizing: border-box; }}
        body {{
            font-family: system-ui, -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
            background: #f5f7fa;
            min-height: 100vh;
            display: flex;
            align-items: center;
            justify-content: center;
        }}
        .error-container {{
            text-align: center;
            padding: 2rem;
        }}
        .error-code {{
            font-size: 6rem;
            font-weight: 700;
            color: #1a365d;
            line-height: 1;
        }}
        .error-title {{
            font-size: 1.5rem;
            color: #333;
            margin: 1rem 0;
        }}
    </style>
</head>
<body>
    <div class="error-container">
        <div class="error-code">{status}</div>
        <h1 class="error-title">{title}</h1>
    </div>
</body>
</html>"#
    )
}

/// Settings page.
pub fn settings() -> String {
    layout(
        "Settings",
        &format!(
            r#"<div class="admin-layout">
        {nav}
        <main class="content">
            <header class="page-header">
                <h2>Settings</h2>
            </header>
            <div class="settings-section">
                <h3>Fabi-SC ID Configuration</h3>
                <form id="id-config-form" class="settings-form">
                    <div class="form-group">
                        <label class="form-label" for="server-url">Server URL</label>
                        <input type="url" class="form-input" id="server-url" required>
                    </div>
                    <div class="form-group">
                        <label class="form-label" for="app-id">Application ID</label>
                        <input type="text" class="form-input" id="app-id" required>
                    </div>
                    <div class="form-group">
                        <label class="form-label" for="api-key">API Key</label>
                        <input type="password" class="form-input" id="api-key" placeholder="(unchanged)">
                        <span class="form-hint">Leave empty to keep current key</span>
                    </div>
                    <div class="form-group">
                        <label class="form-label" for="admin-origin">Gateway URL</label>
                        <input type="url" class="form-input" id="admin-origin" placeholder="https://gateway.example.com" required>
                        <span class="form-hint">The public URL of this gateway (used for login popups)</span>
                    </div>
                    <button type="submit" class="btn btn-primary">Save Changes</button>
                </form>
            </div>
            <div class="settings-section">
                <h3>Actions</h3>
                <button class="btn" onclick="reloadRoutes()">Reload Routes</button>
            </div>
        </main>
    </div>"#,
            nav = nav("settings")
        ),
    )
}
