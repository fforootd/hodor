package ui

import (
	"fmt"
	"html/template"
	"net/http"
	"strings"

	"github.com/zitadel/zitadel/internal/api"
)

// --- CSS ---

func (u *UI) handleCSS(w http.ResponseWriter, r *http.Request) {
	w.Header().Set("Content-Type", "text/css")
	w.Header().Set("Cache-Control", "no-cache, no-store, must-revalidate")
	fmt.Fprint(w, cssContent)
}

const cssContent = `
:root {
  --z-primary: #6366f1;
  --z-primary-hover: #4f46e5;
  --z-primary-light: rgba(99,102,241,0.08);
  --z-bg: #f8f9fb;
  --z-surface: #ffffff;
  --z-surface-hover: #f4f5f7;
  --z-text: #1a1a2e;
  --z-text-muted: #6b7280;
  --z-text-light: #9ca3af;
  --z-border: #e5e7eb;
  --z-border-light: #f0f1f3;
  --z-error: #ef4444;
  --z-success: #10b981;
  --z-warning: #f59e0b;
  --z-radius: 12px;
  --z-radius-sm: 8px;
  --z-shadow-sm: 0 1px 2px rgba(0,0,0,0.04);
  --z-shadow: 0 1px 3px rgba(0,0,0,0.06), 0 1px 2px rgba(0,0,0,0.04);
  --z-shadow-md: 0 4px 6px rgba(0,0,0,0.05), 0 2px 4px rgba(0,0,0,0.03);
  --z-shadow-lg: 0 10px 25px rgba(0,0,0,0.08);
  --z-font: 'Inter', -apple-system, BlinkMacSystemFont, system-ui, sans-serif;
  --z-sidebar-w: 240px;
}

* { margin: 0; padding: 0; box-sizing: border-box; }

body {
  font-family: var(--z-font);
  background: var(--z-bg);
  color: var(--z-text);
  min-height: 100vh;
  line-height: 1.6;
  -webkit-font-smoothing: antialiased;
}

a { color: var(--z-primary); text-decoration: none; }
a:hover { text-decoration: underline; }

.login-page {
  display: flex;
  align-items: center;
  justify-content: center;
  min-height: 100vh;
  padding: 2rem;
  background: linear-gradient(135deg, #f0f2ff 0%, #fafbff 50%, #f5f3ff 100%);
}

.login-card {
  background: var(--z-surface);
  border: 1px solid var(--z-border);
  border-radius: 16px;
  padding: 2.5rem;
  width: 100%;
  max-width: 400px;
  box-shadow: var(--z-shadow-lg);
}

.login-card h1 { font-size: 1.5rem; font-weight: 700; color: var(--z-text); margin-bottom: 0.25rem; }
.login-card .subtitle { color: var(--z-text-muted); font-size: 0.875rem; margin-bottom: 2rem; }
.login-logo { font-size: 1.25rem; font-weight: 800; letter-spacing: -0.03em; color: var(--z-text); margin-bottom: 1.5rem; }

.form-group { margin-bottom: 1.25rem; }
.form-group label { display: block; font-size: 0.8125rem; font-weight: 500; color: var(--z-text); margin-bottom: 0.375rem; }

.form-group input, .form-group select, .form-group textarea {
  width: 100%;
  padding: 0.5rem 0.75rem;
  background: var(--z-surface);
  border: 1px solid var(--z-border);
  border-radius: var(--z-radius-sm);
  color: var(--z-text);
  font-size: 0.875rem;
  font-family: var(--z-font);
  transition: border-color 0.2s, box-shadow 0.2s;
}
.form-group input:focus, .form-group select:focus { outline: none; border-color: var(--z-primary); box-shadow: 0 0 0 3px var(--z-primary-light); }
.form-group input::placeholder { color: var(--z-text-light); }
.form-hint { font-size: 0.75rem; color: var(--z-text-light); margin-top: 0.25rem; }

.btn {
  display: inline-flex; align-items: center; justify-content: center;
  width: 100%; padding: 0.5rem 1.25rem;
  background: var(--z-text); color: white; border: none;
  border-radius: var(--z-radius-sm); font-size: 0.875rem; font-weight: 600;
  font-family: var(--z-font); cursor: pointer;
  transition: background 0.15s, transform 0.1s, box-shadow 0.15s;
}
.btn:hover { background: #2d2d4e; box-shadow: var(--z-shadow-sm); }
.btn:active { transform: scale(0.98); }

.btn-sm {
  display: inline-flex; align-items: center; padding: 0.35rem 0.75rem;
  border-radius: 6px; font-size: 0.8125rem; font-weight: 500;
  font-family: var(--z-font); cursor: pointer; transition: all 0.15s;
  text-decoration: none; border: 1px solid var(--z-border);
  background: var(--z-surface); color: var(--z-text-muted);
}
.btn-sm:hover { background: var(--z-surface-hover); color: var(--z-text); text-decoration: none; border-color: #d1d5db; }

.btn-primary-sm { background: var(--z-text); color: white; border-color: var(--z-text); }
.btn-primary-sm:hover { background: #2d2d4e; color: white; }

.btn-danger {
  display: inline-flex; align-items: center; padding: 0.35rem 0.75rem;
  border-radius: 6px; font-size: 0.8125rem; font-weight: 500;
  font-family: var(--z-font); cursor: pointer;
  border: 1px solid rgba(239,68,68,0.25); background: rgba(239,68,68,0.04); color: var(--z-error);
  transition: all 0.15s;
}
.btn-danger:hover { background: rgba(239,68,68,0.1); border-color: rgba(239,68,68,0.4); }

.btn-primary { background:var(--z-text);color:white;border:none;cursor:pointer;padding:0.4rem 0.8rem;border-radius:6px;font-size:0.8rem;font-weight:500;font-family:var(--z-font) }
.btn-primary:hover { background:#2d2d4e }
.btn-secondary { background:var(--z-surface);color:var(--z-text-muted);border:1px solid var(--z-border);cursor:pointer;padding:0.4rem 0.8rem;border-radius:6px;font-size:0.8rem;font-weight:500;font-family:var(--z-font) }
.btn-secondary:hover { background:var(--z-surface-hover);color:var(--z-text) }

.error-banner {
  background: rgba(239,68,68,0.06); border: 1px solid rgba(239,68,68,0.2);
  border-radius: var(--z-radius-sm); padding: 0.625rem 0.875rem;
  color: var(--z-error); font-size: 0.875rem; margin-bottom: 1.25rem;
}

.admin-layout { display: flex; min-height: 100vh; }

.admin-sidebar {
  width: var(--z-sidebar-w); background: var(--z-surface);
  border-right: 1px solid var(--z-border); padding: 1.25rem 0;
  flex-shrink: 0; position: fixed; top: 0; left: 0; bottom: 0;
  overflow-y: auto; z-index: 10;
}

.admin-sidebar .logo {
  padding: 0.5rem 1.25rem 1.5rem; font-size: 1.125rem; font-weight: 800;
  letter-spacing: -0.03em; color: var(--z-text);
  display: flex; align-items: center; gap: 0.5rem;
}
.admin-sidebar .logo::before {
  content: ""; display: inline-block; width: 24px; height: 24px;
  background: var(--z-text); border-radius: 6px;
}

.admin-sidebar nav { padding: 0 0.5rem; }
.admin-sidebar nav a {
  display: flex; align-items: center; gap: 0.75rem;
  padding: 0.5rem 0.75rem; color: var(--z-text-muted);
  font-size: 0.875rem; font-weight: 500; text-decoration: none;
  transition: all 0.15s; border-radius: 6px; margin-bottom: 2px;
}
.admin-sidebar nav a:hover { color: var(--z-text); background: var(--z-surface-hover); }

.admin-main { flex: 1; margin-left: var(--z-sidebar-w); padding: 2rem 2.5rem; max-width: 1200px; }

.admin-header { display: flex; justify-content: space-between; align-items: center; margin-bottom: 1.75rem; }
.admin-header h1 { font-size: 1.5rem; font-weight: 700; color: var(--z-text); }
.admin-header .user-info { display: flex; align-items: center; gap: 0.75rem; color: var(--z-text-muted); font-size: 0.8125rem; }
.admin-header .user-info a { color: var(--z-text-muted); font-size: 0.8125rem; }

.stats-grid { display: grid; grid-template-columns: repeat(auto-fit, minmax(180px, 1fr)); gap: 1rem; margin-bottom: 2rem; }
.stat-card {
  background: var(--z-surface); border: 1px solid var(--z-border);
  border-radius: var(--z-radius); padding: 1.25rem;
  box-shadow: var(--z-shadow-sm); transition: box-shadow 0.15s;
}
.stat-card:hover { box-shadow: var(--z-shadow); }
.stat-card .label { font-size: 0.75rem; color: var(--z-text-muted); font-weight: 500; margin-bottom: 0.25rem; text-transform: uppercase; letter-spacing: 0.04em; }
.stat-card .value { font-size: 1.75rem; font-weight: 700; color: var(--z-text); }

.data-table {
  width: 100%; border-collapse: collapse; background: var(--z-surface);
  border: 1px solid var(--z-border); border-radius: var(--z-radius);
  overflow: hidden; box-shadow: var(--z-shadow-sm);
}
.data-table th { text-align: left; padding: 0.65rem 1rem; font-size: 0.72rem; font-weight: 600; text-transform: uppercase; letter-spacing: 0.05em; color: var(--z-text-muted); background: var(--z-bg); border-bottom: 1px solid var(--z-border); }
.data-table td { padding: 0.65rem 1rem; font-size: 0.8125rem; border-bottom: 1px solid var(--z-border-light); color: var(--z-text); }
.data-table tr:last-child td { border-bottom: none; }
.data-table tr:hover td { background: var(--z-surface-hover); }

.badge { display: inline-flex; align-items: center; gap: 0.35rem; padding: 0.15rem 0.55rem; border-radius: 9999px; font-size: 0.72rem; font-weight: 500; }
.badge::before { content: ""; display: inline-block; width: 6px; height: 6px; border-radius: 50%; }
.badge-active { background: rgba(16,185,129,0.08); color: #059669; }
.badge-active::before { background: var(--z-success); }
.badge-deactivated { background: rgba(239,68,68,0.06); color: var(--z-error); }
.badge-deactivated::before { background: var(--z-error); }
.badge-revoked { background: rgba(239,68,68,0.06); color: var(--z-error); }
.badge-revoked::before { background: var(--z-error); }

.actions-cell { display: flex; gap: 0.5rem; align-items: center; }
.toolbar { display: flex; justify-content: space-between; align-items: center; margin-bottom: 1rem; }

.event-badge { display:inline-block;padding:0.15rem 0.5rem;border-radius:4px;font-size:0.72rem;font-family:monospace;font-weight:500 }
.event-badge-identity { background:rgba(99,102,241,0.08);color:#4f46e5 }
.event-badge-session { background:rgba(245,158,11,0.08);color:#d97706 }
.event-badge-auth { background:rgba(239,68,68,0.06);color:#dc2626 }
.event-badge-event { background:rgba(107,114,128,0.08);color:#6b7280 }

.status-badge { display:inline-block;padding:0.15rem 0.5rem;border-radius:4px;font-size:0.72rem;font-weight:500 }
.status-idle { background:rgba(107,114,128,0.08);color:#6b7280 }
.status-running { background:rgba(59,130,246,0.08);color:#2563eb }
.status-success { background:rgba(16,185,129,0.08);color:#059669 }
.status-error { background:rgba(239,68,68,0.06);color:#dc2626 }

.form-card {
  background: var(--z-surface); border: 1px solid var(--z-border);
  border-radius: var(--z-radius); padding: 2rem; max-width: 560px;
  box-shadow: var(--z-shadow-sm);
}
.form-card h2 { font-size: 1.25rem; font-weight: 700; margin-bottom: 1.5rem; }
.form-card .btn { margin-top: 0.5rem; }
.form-actions { display: flex; gap: 0.75rem; margin-top: 1.5rem; }
.form-actions .btn { width: auto; }

.card { transition: box-shadow 0.15s, border-color 0.15s; }
.card:hover { box-shadow: var(--z-shadow); border-color: #d1d5db; }
`

// --- HTML Renderers ---

const baseHead = `<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>{{.Title}} — Zitadel</title>
  <link rel="preconnect" href="https://fonts.googleapis.com">
  <link rel="preconnect" href="https://fonts.gstatic.com" crossorigin>
  <link href="https://fonts.googleapis.com/css2?family=Inter:wght@400;500;600;700;800&display=swap" rel="stylesheet">
  <link rel="stylesheet" href="/static/style.css">
</head>
<body>`

const baseFoot = `</body></html>`

// Login page templates.
var loginTmpl = template.Must(template.New("login").Parse(baseHead + `
<div class="login-page">
  <div class="login-card">
    <div class="login-logo">Zitadel</div>
    <h1>Sign in</h1>
    <p class="subtitle">Enter your credentials to continue</p>
    {{if .Error}}
    <div class="error-banner">{{.Error}}</div>
    {{end}}
    <form method="POST" action="/login">
      <input type="hidden" name="redirect_to" value="{{.RedirectTo}}">
      <div class="form-group">
        <label for="identifier">Username</label>
        <input type="text" id="identifier" name="identifier" placeholder="admin@zitadel.local" autocomplete="username" autofocus>
      </div>
      <div class="form-group">
        <label for="password">Password</label>
        <input type="password" id="password" name="password" placeholder="••••••••" autocomplete="current-password">
      </div>
      <button type="submit" class="btn">Continue</button>
    </form>
  </div>
</div>
` + baseFoot))

func renderLoginPage(w http.ResponseWriter, errorMsg, redirectTo string) {
	w.Header().Set("Content-Type", "text/html; charset=utf-8")
	loginTmpl.Execute(w, map[string]string{
		"Title":      "Sign In",
		"Error":      errorMsg,
		"RedirectTo": redirectTo,
	})
}

// Admin dashboard template.
var dashTmpl = template.Must(template.New("dash").Parse(baseHead + adminSidebar + `
  <div class="admin-main">
    <div class="admin-header">
      <h1>Dashboard</h1>
      <div class="user-info">
        {{.Ident.DisplayName}} ({{.Ident.Identifier}})
        · <a href="/logout">Sign out</a>
      </div>
    </div>
    <div class="stats-grid">
      <div class="stat-card">
        <div class="label">Identities</div>
        <div class="value">{{.IdentityCount}}</div>
      </div>
      <div class="stat-card">
        <div class="label">Active Sessions</div>
        <div class="value">{{.SessionCount}}</div>
      </div>
      <div class="stat-card">
        <div class="label">Events</div>
        <div class="value">{{.EventCount}}</div>
      </div>
    </div>
  </div>
</div>
` + baseFoot))

const adminSidebar = `
<div class="admin-layout">
  <aside class="admin-sidebar">
    <div class="logo">Zitadel</div>
    <nav>
      <a href="/admin">◆ Dashboard</a>
      <a href="/admin/entities">◇ Entities</a>
      <a href="/admin/schemas">◇ Schemas</a>
      <a href="/admin/sessions">◇ Sessions</a>
      <a href="/admin/events">◇ Events</a>
      <a href="/admin/jobs">◇ Jobs</a>
    </nav>
  </aside>
`

func renderAdminDashboard(w http.ResponseWriter, ident *IdentityContext, identityCount, sessionCount, eventCount int) {
	w.Header().Set("Content-Type", "text/html; charset=utf-8")
	dashTmpl.Execute(w, map[string]any{
		"Title":         "Dashboard",
		"Ident":         ident,
		"IdentityCount": identityCount,
		"SessionCount":  sessionCount,
		"EventCount":    eventCount,
	})
}

// Admin identities list template.
var entitiesListTmpl = template.Must(template.New("entities").Parse(baseHead + adminSidebar + `
  <div class="admin-main">
    <div class="admin-header">
      <h1>Identities</h1>
      <div class="user-info">
        {{.Ident.DisplayName}} · <a href="/logout">Sign out</a>
      </div>
    </div>
    <div class="toolbar">
      <div></div>
      <a href="/admin/entities/new" class="btn-sm btn-primary-sm">+ New Entity</a>
    </div>
    <table class="data-table">
      <thead>
        <tr>
          <th>ID</th>
          <th>Identifier</th>
          <th>Display Name</th>
          <th>State</th>
          <th>Created</th>
          <th>Actions</th>
        </tr>
      </thead>
      <tbody>
        {{range .Identities}}
        <tr>
          <td>{{.ID}}</td>
          <td>{{.Identifier}}</td>
          <td>{{.DisplayName}}</td>
          <td><span class="badge badge-{{.State}}">{{.State}}</span></td>
          <td>{{.CreatedAt}}</td>
          <td class="actions-cell">
            <a href="/admin/entities/{{.ID}}" class="btn-sm">Edit</a>
            <form method="POST" action="/admin/entities/{{.ID}}/delete" style="display:inline" onsubmit="return confirm('Delete this entity?')">
              <button type="submit" class="btn-danger">Delete</button>
            </form>
          </td>
        </tr>
        {{else}}
        <tr><td colspan="6" style="text-align:center;color:var(--z-text-muted)">No entities found</td></tr>
        {{end}}
      </tbody>
    </table>
  </div>
</div>
` + baseFoot))

func renderAdminEntities(w http.ResponseWriter, ident *IdentityContext, identities any) {
	w.Header().Set("Content-Type", "text/html; charset=utf-8")
	entitiesListTmpl.Execute(w, map[string]any{
		"Title":      "Entities",
		"Ident":      ident,
		"Entities": identities,
	})
}

// Admin identity create/edit form template.
var identityFormTmpl = template.Must(template.New("identity-form").Parse(baseHead + adminSidebar + `
  <div class="admin-main">
    <div class="admin-header">
      <h1>{{if .IsEdit}}Edit Identity{{else}}New Entity{{end}}</h1>
      <div class="user-info">
        {{.Ident.DisplayName}} · <a href="/logout">Sign out</a>
      </div>
    </div>
    {{if .Error}}
    <div class="error-banner" style="max-width:640px;margin-bottom:1rem">{{.Error}}</div>
    {{end}}
    <div class="form-card" style="max-width:640px">
      <form method="POST" action="{{.FormAction}}" id="identityForm">

        {{if not .IsEdit}}
        <div class="form-group">
          <label for="schema_id">Identity Type</label>
          <select id="schema_id" name="schema_id" onchange="onSchemaChange()" style="font-size:1rem;padding:0.6rem 0.8rem">
            <option value="">— Select a type —</option>
            {{range .Schemas}}
            <option value="{{.ID}}" data-type="{{.Type}}" data-fields="{{.FieldsJSON}}" data-auth="{{.AuthMethods}}">{{.Type}} ({{.ID}})</option>
            {{end}}
          </select>
        </div>
        {{else}}
        <input type="hidden" name="schema_id" value="{{.SchemaID}}">
        <div class="form-group">
          <label>Identity Type</label>
          <div style="padding:0.6rem 0;color:var(--muted);font-size:0.9rem">{{.SchemaID}}</div>
        </div>
        {{end}}

        <div class="form-group">
          <label for="identifier">Identifier (email / username)</label>
          <input type="text" id="identifier" name="identifier" value="{{.Identifier}}" placeholder="user@example.com" {{if .IsEdit}}disabled{{end}} autocomplete="off">
          {{if .IsEdit}}<p class="form-hint">Identifier cannot be changed after creation.</p>{{end}}
        </div>

        <div id="schemaFields">
          {{range .DataFields}}
          <div class="form-group">
            <label for="data_{{.Name}}">{{.Label}}{{if .Required}} <span style="color:var(--primary)">*</span>{{end}}</label>
            <input type="{{.InputType}}" id="data_{{.Name}}" name="data_{{.Name}}" value="{{.Value}}" placeholder="{{.Placeholder}}" {{if .Required}}required{{end}}>
            {{if .Description}}<p class="form-hint">{{.Description}}</p>{{end}}
          </div>
          {{end}}
        </div>

        {{if .IsEdit}}
        <div class="form-group">
          <label for="state">State</label>
          <select id="state" name="state">
            <option value="active" {{if eq .State "active"}}selected{{end}}>Active</option>
            <option value="deactivated" {{if eq .State "deactivated"}}selected{{end}}>Deactivated</option>
          </select>
        </div>
        {{end}}

        <div id="authSection">
          <div class="form-group" style="border-top:1px solid var(--border);padding-top:1rem;margin-top:1rem">
            <label style="margin-bottom:0.5rem">Authentication Methods</label>
            <div id="authMethods" style="display:flex;flex-wrap:wrap;gap:0.8rem">
              {{range .AuthOptions}}
              <label style="display:flex;align-items:center;gap:0.4rem;cursor:pointer;font-size:0.9rem">
                <input type="checkbox" name="auth_methods" value="{{.Value}}" {{if .Checked}}checked{{end}}>
                {{.Label}}
              </label>
              {{end}}
            </div>
          </div>
        </div>

        <div class="form-group" id="passwordGroup" {{if not .ShowPassword}}style="display:none"{{end}}>
          <label for="password">{{if .IsEdit}}New Password (leave blank to keep){{else}}Password{{end}}</label>
          <input type="password" id="password" name="password" placeholder="{{if .IsEdit}}••••••••{{else}}Set a password{{end}}" autocomplete="new-password">
        </div>

        <div class="form-actions">
          <button type="submit" class="btn">{{if .IsEdit}}Save Changes{{else}}Create Identity{{end}}</button>
          <a href="/admin/entities" class="btn-sm" style="align-self:center">Cancel</a>
        </div>
      </form>
    </div>
  </div>
</div>

{{if not .IsEdit}}
<script>
// Schema-driven form: swap fields when schema type changes.
const authByType = {
  'human_user':    [{v:'password',l:'🔑 Password'},{v:'passkey',l:'🔐 Passkey'}],
  'service_user':  [{v:'api_key',l:'🔒 API Key'}],
  'app':           [{v:'client_credentials',l:'🔑 Client Credentials'}],
  'ai_agent':      [{v:'delegation',l:'🤖 Delegation Token'}],
};

function onSchemaChange() {
  const sel = document.getElementById('schema_id');
  const opt = sel.options[sel.selectedIndex];
  const type = opt.dataset.type || '';
  const fieldsJSON = opt.dataset.fields || '[]';
  const fields = JSON.parse(fieldsJSON);

  // Render schema fields.
  const container = document.getElementById('schemaFields');
  container.innerHTML = '';
  fields.forEach(f => {
    const div = document.createElement('div');
    div.className = 'form-group';
    const req = f.required ? ' <span style="color:var(--primary)">*</span>' : '';
    if (f.enum && f.enum.length > 0) {
      const opts = f.enum.map(e => '<option value="'+e+'">'+e+'</option>').join('');
      div.innerHTML = '<label for="data_'+f.name+'">'+f.name.replace(/_/g,' ')+req+'</label>'
        + '<select id="data_'+f.name+'" name="data_'+f.name+'" style="font-size:1rem;padding:0.6rem 0.8rem"><option value="">— Select —</option>'+opts+'</select>';
    } else {
      const inputType = f.format === 'email' ? 'email' : f.format === 'uri' ? 'url' : f.type === 'integer' ? 'number' : 'text';
      div.innerHTML = '<label for="data_'+f.name+'">'+f.name.replace(/_/g,' ')+req+'</label>'
        + '<input type="'+inputType+'" id="data_'+f.name+'" name="data_'+f.name+'" placeholder="'+(f.description||'')+'" '+(f.required?'required':'')+'>';
    }
    if (f.description) {
      div.innerHTML += '<p class="form-hint">'+f.description+'</p>';
    }
    container.appendChild(div);
  });

  // Render auth methods.
  const authContainer = document.getElementById('authMethods');
  const methods = authByType[type] || [];
  authContainer.innerHTML = '';
  methods.forEach(m => {
    const label = document.createElement('label');
    label.style = 'display:flex;align-items:center;gap:0.4rem;cursor:pointer;font-size:0.9rem';
    label.innerHTML = '<input type="checkbox" name="auth_methods" value="'+m.v+'" '+(m.v==='password'?'checked':'')+'>'+m.l;
    authContainer.appendChild(label);
  });

  // Show/hide password field.
  const pwGroup = document.getElementById('passwordGroup');
  pwGroup.style.display = methods.some(m => m.v === 'password') ? '' : 'none';
}
</script>
{{end}}
` + baseFoot))

// FormFieldData holds field info for rendering in the form.
type FormFieldData struct {
	Name        string
	Label       string
	Value       string
	InputType   string
	Placeholder string
	Description string
	Required    bool
	Options     []string
}

// AuthOption is a checkbox option for auth methods.
type AuthOption struct {
	Value   string
	Label   string
	Checked bool
}

// SchemaOption holds schema data for the dropdown.
type SchemaOption struct {
	ID          string
	Type        string
	FieldsJSON  string
	AuthMethods string
}

func renderAdminIdentityForm(w http.ResponseWriter, ident *IdentityContext, identity *api.IdentityResponse, errorMsg string, schemas []SchemaOption) {
	w.Header().Set("Content-Type", "text/html; charset=utf-8")

	data := map[string]any{
		"Title":        "New Entity",
		"Ident":        ident,
		"IsEdit":       false,
		"FormAction":   "/admin/entities/new",
		"Identifier":   "",
		"DisplayName":  "",
		"State":        "active",
		"SchemaID":     "",
		"Schemas":      schemas,
		"DataFields":   []FormFieldData{},
		"AuthOptions":  []AuthOption{},
		"ShowPassword": false,
		"Error":        errorMsg,
	}

	if identity != nil {
		data["Title"] = "Edit " + identity.Identifier
		data["IsEdit"] = true
		data["FormAction"] = fmt.Sprintf("/admin/entities/%d", identity.ID)
		data["Identifier"] = identity.Identifier
		data["State"] = identity.State

		// Build data fields from identity profile.
		var fields []FormFieldData
		if dataMap, ok := identity.Profile.(map[string]any); ok {
			for k, v := range dataMap {
				val := ""
				if v != nil {
					val = fmt.Sprintf("%v", v)
				}
				fields = append(fields, FormFieldData{
					Name:      k,
					Label:     strings.ReplaceAll(k, "_", " "),
					Value:     val,
					InputType: "text",
				})
			}
		}
		data["DataFields"] = fields
		data["ShowPassword"] = true

		// Auth options for edit.
		authOpts := []AuthOption{
			{Value: "password", Label: "🔑 Password", Checked: true},
		}
		data["AuthOptions"] = authOpts
	}

	identityFormTmpl.Execute(w, data)
}

// Admin sessions template.
var sessionsListTmpl = template.Must(template.New("sessions").Parse(baseHead + adminSidebar + `
  <div class="admin-main">
    <div class="admin-header">
      <h1>Active Sessions</h1>
      <div class="user-info">
        {{.Ident.DisplayName}} · <a href="/logout">Sign out</a>
      </div>
    </div>
    <table class="data-table">
      <thead>
        <tr>
          <th>ID</th>
          <th>Identity</th>
          <th>User Agent</th>
          <th>IP Address</th>
          <th>Created</th>
          <th>Expires</th>
        </tr>
      </thead>
      <tbody>
        {{range .Sessions}}
        <tr>
          <td>{{.ID}}</td>
          <td>{{.Identifier}}</td>
          <td>{{.UserAgent}}</td>
          <td>{{.IPAddress}}</td>
          <td>{{.CreatedAt}}</td>
          <td>{{.ExpiresAt}}</td>
        </tr>
        {{else}}
        <tr><td colspan="6" style="text-align:center;color:var(--z-text-muted)">No active sessions</td></tr>
        {{end}}
      </tbody>
    </table>
  </div>
</div>
` + baseFoot))

func renderAdminSessions(w http.ResponseWriter, ident *IdentityContext, sessions any) {
	w.Header().Set("Content-Type", "text/html; charset=utf-8")
	sessionsListTmpl.Execute(w, map[string]any{
		"Title":    "Sessions",
		"Ident":    ident,
		"Sessions": sessions,
	})
}

// --- Events viewer ---

var eventsListTmpl = template.Must(template.New("events").Parse(baseHead + `
<div class="admin-container">
  <div class="admin-header">
    <div>
      <h1>Events</h1>
      <p class="text-muted">Audit trail — all state mutations and reads</p>
    </div>
  </div>

  <div style="margin-bottom:1rem;display:flex;gap:0.5rem;flex-wrap:wrap">
    <a href="/admin/events" class="btn {{if not .Filter}}btn-primary{{else}}btn-secondary{{end}}" style="text-decoration:none;padding:0.4rem 0.8rem;font-size:0.8rem;border-radius:6px">All</a>
    <a href="/admin/events?type=auth." class="btn {{if eq .Filter "auth."}}btn-primary{{else}}btn-secondary{{end}}" style="text-decoration:none;padding:0.4rem 0.8rem;font-size:0.8rem;border-radius:6px">🔐 Auth</a>
    <a href="/admin/events?type=identity." class="btn {{if eq .Filter "identity."}}btn-primary{{else}}btn-secondary{{end}}" style="text-decoration:none;padding:0.4rem 0.8rem;font-size:0.8rem;border-radius:6px">👤 Identity</a>
    <a href="/admin/events?type=session." class="btn {{if eq .Filter "session."}}btn-primary{{else}}btn-secondary{{end}}" style="text-decoration:none;padding:0.4rem 0.8rem;font-size:0.8rem;border-radius:6px">🎫 Session</a>
    <a href="/admin/events?type=event." class="btn {{if eq .Filter "event."}}btn-primary{{else}}btn-secondary{{end}}" style="text-decoration:none;padding:0.4rem 0.8rem;font-size:0.8rem;border-radius:6px">📋 Meta</a>
  </div>

  <div class="card" style="overflow-x:auto">
    <table>
      <thead>
        <tr>
          <th>Type</th>
          <th>Actor</th>
          <th>Aggregate</th>
          <th>Payload</th>
          <th>Trace</th>
          <th>Session</th>
          <th>Time</th>
        </tr>
      </thead>
      <tbody>
        {{range .Events}}
        <tr>
          <td><span class="event-badge event-badge-{{.AggregateType}}">{{.EventType}}</span></td>
          <td style="font-family:monospace;font-size:0.75rem">{{.ActorID}}</td>
          <td><span style="color:var(--z-text-muted);font-size:0.75rem">{{.AggregateType}}/</span>{{.AggregateID}}</td>
          <td style="max-width:300px;overflow:hidden;text-overflow:ellipsis;white-space:nowrap;font-family:monospace;font-size:0.7rem;color:var(--z-text-muted)">{{.Payload}}</td>
          <td style="font-family:monospace;font-size:0.7rem;color:var(--z-text-muted)">{{if .TraceID}}{{.TraceID}}{{else}}—{{end}}</td>
          <td style="font-family:monospace;font-size:0.75rem">{{if .SessionID}}{{.SessionID}}{{else}}—{{end}}</td>
          <td style="font-size:0.8rem;white-space:nowrap">{{.CreatedAt}}</td>
        </tr>
        {{else}}
        <tr><td colspan="7" style="text-align:center;color:var(--z-text-muted)">No events</td></tr>
        {{end}}
      </tbody>
    </table>
  </div>
</div>
` + baseFoot))

func renderAdminEvents(w http.ResponseWriter, ident *IdentityContext, events any, filter string) {
	w.Header().Set("Content-Type", "text/html; charset=utf-8")
	eventsListTmpl.Execute(w, map[string]any{
		"Title":  "Events",
		"Ident":  ident,
		"Events": events,
		"Filter": filter,
	})
}

// --- Jobs viewer ---

var jobsListTmpl = template.Must(template.New("jobs").Parse(baseHead + `
<div class="admin-container">
  <div class="admin-header">
    <div>
      <h1>Jobs</h1>
      <p class="text-muted">Background workers and their schedules</p>
    </div>
  </div>

  <div class="card" style="overflow-x:auto">
    <table>
      <thead>
        <tr>
          <th>Job</th>
          <th>Schedule</th>
          <th>Status</th>
          <th>Last Run</th>
          <th>Next Run</th>
          <th>Runs</th>
          <th>Actions</th>
        </tr>
      </thead>
      <tbody>
        {{range .Jobs}}
        <tr>
          <td>
            <strong>{{.DisplayName}}</strong>
            <div style="font-size:0.75rem;color:var(--z-text-muted)">{{.Description}}</div>
          </td>
          <td style="font-family:monospace;font-size:0.8rem">{{.Cron}}</td>
          <td>
            <span class="status-badge status-{{.LastStatus}}">{{.LastStatus}}</span>
            {{if .LastError}}<div style="font-size:0.7rem;color:#f87171;max-width:200px;overflow:hidden;text-overflow:ellipsis">{{.LastError}}</div>{{end}}
          </td>
          <td style="font-size:0.8rem;white-space:nowrap">{{.LastRunAt}}</td>
          <td style="font-size:0.8rem;white-space:nowrap">{{.NextRunAt}}</td>
          <td style="font-family:monospace">{{.RunCount}}</td>
          <td>
            <form method="POST" action="/admin/jobs/{{.Name}}/toggle" style="display:inline">
              {{if .Enabled}}
              <button type="submit" class="btn btn-secondary" style="font-size:0.75rem;padding:0.3rem 0.6rem">Disable</button>
              {{else}}
              <button type="submit" class="btn btn-primary" style="font-size:0.75rem;padding:0.3rem 0.6rem">Enable</button>
              {{end}}
            </form>
          </td>
        </tr>
        {{else}}
        <tr><td colspan="7" style="text-align:center;color:var(--z-text-muted)">No jobs</td></tr>
        {{end}}
      </tbody>
    </table>
  </div>

  <div style="margin-top:2rem">
    <h2>Retention Policies</h2>
    <p class="text-muted" style="margin-bottom:1rem">Per-event-type TTLs for OLTP buffer and lake storage</p>
    <div class="card" style="overflow-x:auto">
      <table>
        <thead>
          <tr>
            <th>Event Pattern</th>
            <th>OLTP TTL</th>
            <th>Lake TTL</th>
            <th>Priority</th>
          </tr>
        </thead>
        <tbody>
          {{range .Policies}}
          <tr>
            <td style="font-family:monospace">{{.EventPattern}}</td>
            <td>{{.OLTPTTL}}</td>
            <td>{{if eq .LakeTTL "0"}}forever{{else}}{{.LakeTTL}}{{end}}</td>
            <td style="font-family:monospace">{{.Priority}}</td>
          </tr>
          {{else}}
          <tr><td colspan="4" style="text-align:center;color:var(--z-text-muted)">No policies</td></tr>
          {{end}}
        </tbody>
      </table>
    </div>
  </div>
</div>
` + baseFoot))

func renderAdminJobs(w http.ResponseWriter, ident *IdentityContext, jobs any, policies any) {
	w.Header().Set("Content-Type", "text/html; charset=utf-8")
	jobsListTmpl.Execute(w, map[string]any{
		"Title":    "Jobs",
		"Ident":    ident,
		"Jobs":     jobs,
		"Policies": policies,
	})
}

// Admin schemas list template.
var schemasListTmpl = template.Must(template.New("schemas").Parse(baseHead + adminSidebar + `
  <div class="admin-main">
    <div class="admin-header">
      <h1>Identity Schemas</h1>
      <div class="user-info">
        {{.Ident.DisplayName}} · <a href="/logout">Sign out</a>
      </div>
    </div>

    <p style="color:var(--muted);margin-bottom:1.5rem">Schemas define the data shape for each identity type. Each schema is a JSON Schema document that validates the <code>data</code> field on entities.</p>

    <div style="display:grid;grid-template-columns:repeat(auto-fill,minmax(380px,1fr));gap:1.2rem">
      {{range .Schemas}}
      <div class="card" style="border:1px solid var(--border);border-radius:12px;padding:1.4rem;background:var(--surface)">
        <div style="display:flex;justify-content:space-between;align-items:center;margin-bottom:0.8rem">
          <div>
            <span style="font-size:1.1rem;font-weight:600;color:var(--text)">{{.Type}}</span>
            <span style="font-size:0.75rem;color:var(--muted);margin-left:0.5rem">v{{.Version}}</span>
          </div>
          <span style="font-size:0.7rem;padding:0.2rem 0.6rem;background:var(--primary);color:#fff;border-radius:99px">{{.ID}}</span>
        </div>

        <div style="margin-bottom:0.8rem">
          <div style="font-size:0.75rem;color:var(--muted);margin-bottom:0.4rem;text-transform:uppercase;letter-spacing:0.05em">Fields</div>
          <div style="display:flex;flex-wrap:wrap;gap:0.3rem">
            {{range .Fields}}
            <span style="font-size:0.75rem;padding:0.15rem 0.5rem;background:var(--bg);border:1px solid var(--border);border-radius:6px;{{if .Required}}font-weight:600;border-color:var(--primary){{end}}">{{.Name}}<span style="color:var(--muted);margin-left:0.3rem">{{.Type}}</span></span>
            {{end}}
          </div>
        </div>

        <div style="display:flex;justify-content:space-between;align-items:center">
          <span style="font-size:0.7rem;color:var(--muted)">{{.RequiredCount}} required · {{.FieldCount}} fields</span>
          <span style="font-size:0.7rem;color:var(--muted)">{{.CreatedAt}}</span>
        </div>
      </div>
      {{else}}
      <p style="color:var(--muted)">No schemas registered yet.</p>
      {{end}}
    </div>
  </div>
</div>
` + baseFoot))

// SchemaCard holds data for rendering a schema card in the admin UI.
type SchemaCard struct {
	ID            string
	Type          string
	Version       int
	Fields        []SchemaField
	FieldCount    int
	RequiredCount int
	CreatedAt     string
}

// SchemaField is a single field extracted from a JSON Schema.
type SchemaField struct {
	Name     string
	Type     string
	Required bool
}

func renderAdminSchemas(w http.ResponseWriter, ident *IdentityContext, schemas []SchemaCard) {
	w.Header().Set("Content-Type", "text/html; charset=utf-8")
	schemasListTmpl.Execute(w, map[string]any{
		"Title":   "Schemas",
		"Ident":   ident,
		"Schemas": schemas,
	})
}
