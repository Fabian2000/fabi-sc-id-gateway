// Fabi-SC ID Gateway Admin UI JavaScript

// Routes page functionality
let currentRouteId = null;

function showAddModal() {
    currentRouteId = null;
    document.getElementById('modal-title').textContent = 'Add Route';
    document.getElementById('route-form').reset();
    document.getElementById('route-allowed-users').value = '';
    updateAllowedUsersVisibility();
    document.getElementById('route-modal').classList.remove('hidden');
}

function showEditModal(route) {
    currentRouteId = route.id;
    document.getElementById('modal-title').textContent = 'Edit Route';
    document.getElementById('route-id').value = route.id;
    document.getElementById('route-name').value = route.name;
    document.getElementById('route-host').value = route.host;
    document.getElementById('route-upstream').value = route.upstream_url;
    document.getElementById('route-auth').checked = route.requires_auth;
    document.getElementById('route-allowed-users').value = (route.allowed_users || []).join(', ');
    updateAllowedUsersVisibility();
    document.getElementById('route-modal').classList.remove('hidden');
}

function updateAllowedUsersVisibility() {
    const authCheckbox = document.getElementById('route-auth');
    const allowedUsersGroup = document.getElementById('allowed-users-group');
    if (authCheckbox && allowedUsersGroup) {
        allowedUsersGroup.style.display = authCheckbox.checked ? 'block' : 'none';
    }
}

function hideModal() {
    document.getElementById('route-modal').classList.add('hidden');
}

async function loadRoutes() {
    try {
        const response = await fetch('/_admin/api/routes');
        const routes = await response.json();
        renderRoutes(routes);
    } catch (error) {
        console.error('Failed to load routes:', error);
        document.getElementById('routes-list').innerHTML =
            '<p class="loading">Failed to load routes</p>';
    }
}

function renderRoutes(routes) {
    const container = document.getElementById('routes-list');

    if (!routes || routes.length === 0) {
        container.innerHTML = '<p class="loading">No routes configured yet</p>';
        return;
    }

    container.innerHTML = routes.map(route => `
        <div class="route-item" data-id="${route.id}">
            <div class="route-info">
                <div class="route-name">${escapeHtml(route.name)}</div>
                <div class="route-path">${escapeHtml(route.host)} → ${escapeHtml(route.upstream_url)}</div>
            </div>
            <div class="route-badges">
                ${route.requires_auth ? '<span class="badge badge-auth">Auth</span>' : ''}
                ${!route.enabled ? '<span class="badge badge-disabled">Disabled</span>' : ''}
            </div>
            <div class="route-actions">
                <button class="btn" onclick='showEditModal(${JSON.stringify(route)})'>Edit</button>
                <button class="btn btn-danger" onclick="deleteRoute('${route.id}')">Delete</button>
            </div>
        </div>
    `).join('');
}

async function saveRoute(event) {
    event.preventDefault();

    const allowedUsersValue = document.getElementById('route-allowed-users').value;
    const allowedUsers = allowedUsersValue
        ? allowedUsersValue.split(',').map(u => u.trim()).filter(u => u)
        : [];

    const data = {
        name: document.getElementById('route-name').value,
        host: document.getElementById('route-host').value,
        upstream_url: document.getElementById('route-upstream').value,
        requires_auth: document.getElementById('route-auth').checked,
        allowed_users: allowedUsers,
    };

    try {
        let response;
        if (currentRouteId) {
            response = await fetch(`/_admin/api/routes/${currentRouteId}`, {
                method: 'PUT',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify(data),
            });
        } else {
            response = await fetch('/_admin/api/routes', {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify(data),
            });
        }

        if (response.ok) {
            hideModal();
            loadRoutes();
            // Reload routes in proxy
            await fetch('/_admin/api/reload', { method: 'POST' });
        } else {
            const error = await response.json();
            alert('Error: ' + (error.error || 'Unknown error'));
        }
    } catch (error) {
        console.error('Failed to save route:', error);
        alert('Failed to save route');
    }
}

async function deleteRoute(id) {
    if (!confirm('Are you sure you want to delete this route?')) {
        return;
    }

    try {
        const response = await fetch(`/_admin/api/routes/${id}`, {
            method: 'DELETE',
        });

        if (response.ok) {
            loadRoutes();
            // Reload routes in proxy
            await fetch('/_admin/api/reload', { method: 'POST' });
        } else {
            const error = await response.json();
            alert('Error: ' + (error.error || 'Unknown error'));
        }
    } catch (error) {
        console.error('Failed to delete route:', error);
        alert('Failed to delete route');
    }
}

// Settings page functionality
async function loadSettings() {
    try {
        const response = await fetch('/_admin/api/settings/id');
        const config = await response.json();

        document.getElementById('server-url').value = config.server_url || '';
        document.getElementById('app-id').value = config.app_id || '';
        document.getElementById('admin-origin').value = config.admin_origin || '';
    } catch (error) {
        console.error('Failed to load settings:', error);
    }
}

async function saveSettings(event) {
    event.preventDefault();

    const data = {
        server_url: document.getElementById('server-url').value,
        app_id: document.getElementById('app-id').value,
        admin_origin: document.getElementById('admin-origin').value,
    };

    const apiKey = document.getElementById('api-key').value;
    if (apiKey) {
        data.api_key = apiKey;
    }

    try {
        const response = await fetch('/_admin/api/settings/id', {
            method: 'PUT',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify(data),
        });

        if (response.ok) {
            alert('Settings saved successfully');
        } else {
            const error = await response.json();
            alert('Error: ' + (error.error || 'Unknown error'));
        }
    } catch (error) {
        console.error('Failed to save settings:', error);
        alert('Failed to save settings');
    }
}

async function reloadRoutes() {
    try {
        const response = await fetch('/_admin/api/reload', { method: 'POST' });
        if (response.ok) {
            alert('Routes reloaded successfully');
        } else {
            alert('Failed to reload routes');
        }
    } catch (error) {
        console.error('Failed to reload routes:', error);
        alert('Failed to reload routes');
    }
}

// Utility functions
function escapeHtml(text) {
    const div = document.createElement('div');
    div.textContent = text;
    return div.innerHTML;
}

// Initialize page
document.addEventListener('DOMContentLoaded', () => {
    // Routes page
    const routeForm = document.getElementById('route-form');
    if (routeForm) {
        routeForm.addEventListener('submit', saveRoute);
        loadRoutes();

        // Toggle allowed users visibility when auth checkbox changes
        const authCheckbox = document.getElementById('route-auth');
        if (authCheckbox) {
            authCheckbox.addEventListener('change', updateAllowedUsersVisibility);
        }
    }

    // Settings page
    const settingsForm = document.getElementById('id-config-form');
    if (settingsForm) {
        settingsForm.addEventListener('submit', saveSettings);
        loadSettings();
    }

    // Close modal on outside click
    const modal = document.getElementById('route-modal');
    if (modal) {
        modal.addEventListener('click', (e) => {
            if (e.target === modal) {
                hideModal();
            }
        });
    }

    // Close modal on escape key
    document.addEventListener('keydown', (e) => {
        if (e.key === 'Escape') {
            hideModal();
        }
    });
});
