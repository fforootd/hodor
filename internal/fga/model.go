// Package fga model.go defines the Zitadel authorization model for OpenFGA.
//
// The model follows a layered hierarchy:
//
//	instance → org → entity/app/group/settings
//
// Instance is the isolation boundary. Orgs share the same instance and can
// access data from other orgs via instance-level admin grants.
//
// Groups are first-class citizens — customers have requested them extensively.
package fga

import (
	openfgav1 "github.com/openfga/api/proto/openfga/v1"
	schemaPkg "github.com/zitadel/zitadel/internal/schema"
)

// ZitadelModel returns the complete OpenFGA type definitions for
// Zitadel's internal authorization.
//
// Type hierarchy:
//
//	user          — actor type (any identity)
//	instance      — global scope (owner > admin > viewer)
//	org           — tenant scope (owner > admin > member > viewer)
//	entity        — identity objects (human_user, service_user, ai_agent)
//	app           — OIDC/SAML client applications
//	group         — user/app grouping with membership-based grants
//	settings      — cascading policies (password, login, etc.)
//	session       — active user sessions
func ZitadelModel() []*openfgav1.TypeDefinition {
	return []*openfgav1.TypeDefinition{
		typeUser(),
		typeInstance(),
		typeOrg(),
		typeEntity(),
		typeApp(),
		typeGroup(),
		typeSettings(),
		typeSession(),
	}
}

// ──────────────────────────────────────────────────────────────────
// type user — the actor type, represents any identity
// ──────────────────────────────────────────────────────────────────

func typeUser() *openfgav1.TypeDefinition {
	return &openfgav1.TypeDefinition{
		Type:      "user",
		Metadata:  &openfgav1.Metadata{},
		Relations: map[string]*openfgav1.Userset{},
	}
}

// ──────────────────────────────────────────────────────────────────
// type instance — global scope
// ──────────────────────────────────────────────────────────────────

func typeInstance() *openfgav1.TypeDefinition {
	return &openfgav1.TypeDefinition{
		Type: "instance",
		Metadata: &openfgav1.Metadata{
			Relations: map[string]*openfgav1.RelationMetadata{
				"owner":  directUser(),
				"admin":  noDirectRelation(),
				"viewer": directUser(),
				// Permissions
				"can_manage_orgs":        noDirectRelation(),
				"can_manage_settings":    noDirectRelation(),
				"can_manage_schemas":     noDirectRelation(),
				"can_manage_providers":   noDirectRelation(),
				"can_manage_entities":    noDirectRelation(),
				"can_manage_sessions":    noDirectRelation(),
				"can_manage_login_flows": noDirectRelation(),
				"can_manage_actions":     noDirectRelation(),
				"can_view_audit":         noDirectRelation(),
				"can_manage_fga":         noDirectRelation(),
			},
		},
		Relations: map[string]*openfgav1.Userset{
			"owner": this(),
			"admin": union(computed("owner")),
			"viewer": union(
				this(),
				computed("admin"),
			),
			// Permissions — derived from roles
			"can_manage_orgs":        computed("admin"),
			"can_manage_settings":    computed("admin"),
			"can_manage_schemas":     computed("admin"),
			"can_manage_providers":   computed("admin"),
			"can_manage_entities":    computed("admin"),
			"can_manage_sessions":    computed("admin"),
			"can_manage_login_flows": computed("admin"),
			"can_manage_actions":     computed("admin"),
			"can_view_audit":         computed("viewer"),
			"can_manage_fga":         computed("owner"),
		},
	}
}

// ──────────────────────────────────────────────────────────────────
// type org — tenant scope, inherits from instance
// ──────────────────────────────────────────────────────────────────

func typeOrg() *openfgav1.TypeDefinition {
	return &openfgav1.TypeDefinition{
		Type: "org",
		Metadata: &openfgav1.Metadata{
			Relations: map[string]*openfgav1.RelationMetadata{
				"parent": {
					DirectlyRelatedUserTypes: []*openfgav1.RelationReference{
						{Type: "instance"},
					},
				},
				"owner":  directUser(),
				"admin":  directUser(),
				"member": directUser(),
				"viewer": directUser(),
				// Permissions
				"can_create_entity":    noDirectRelation(),
				"can_read_entity":      noDirectRelation(),
				"can_update_entity":    noDirectRelation(),
				"can_delete_entity":    noDirectRelation(),
				"can_read_settings":    noDirectRelation(),
				"can_write_settings":   noDirectRelation(),
				"can_manage_providers": noDirectRelation(),
				"can_manage_rules":     noDirectRelation(),
				"can_manage_apps":      noDirectRelation(),
				"can_manage_groups":    noDirectRelation(),
				"can_manage_fga_model": noDirectRelation(),
				"can_write_fga_tuples": noDirectRelation(),
				"can_read_fga":         noDirectRelation(),
				"can_read_pii":         directUser(),
				"can_write_pii":        directUser(),
			},
		},
		Relations: map[string]*openfgav1.Userset{
			"parent": this(),
			"owner": union(
				this(),
				tupleToUserset("parent", "owner"),
			),
			"admin": union(
				this(),
				computed("owner"),
				tupleToUserset("parent", "admin"),
			),
			"member": union(
				this(),
				computed("admin"),
			),
			"viewer": union(
				this(),
				computed("member"),
			),
			// Permissions
			"can_create_entity":    computed("admin"),
			"can_read_entity":      computed("viewer"),
			"can_update_entity":    computed("admin"),
			"can_delete_entity":    computed("owner"),
			"can_read_settings":    computed("viewer"),
			"can_write_settings":   computed("admin"),
			"can_manage_providers": computed("admin"),
			"can_manage_rules":     computed("admin"),
			"can_manage_apps":      computed("admin"),
			"can_manage_groups":    computed("admin"),
			"can_manage_fga_model": computed("owner"),
			"can_write_fga_tuples": computed("admin"),
			"can_read_fga":         computed("viewer"),
			"can_read_pii": union(
				this(),
				computed("admin"),
			),
			"can_write_pii": union(
				this(),
				computed("admin"),
			),
		},
	}
}

// ──────────────────────────────────────────────────────────────────
// type entity — identity objects (human_user, service_user, ai_agent)
// ──────────────────────────────────────────────────────────────────

func typeEntity() *openfgav1.TypeDefinition {
	return &openfgav1.TypeDefinition{
		Type: "entity",
		Metadata: &openfgav1.Metadata{
			Relations: map[string]*openfgav1.RelationMetadata{
				"org": {
					DirectlyRelatedUserTypes: []*openfgav1.RelationReference{
						{Type: "org"},
					},
				},
				"owner":  directUser(),
				"editor": directUser(),
				"viewer": directUser(),
				// Permissions
				"can_read":               noDirectRelation(),
				"can_update":             noDirectRelation(),
				"can_delete":             noDirectRelation(),
				"can_manage_credentials": noDirectRelation(),
				"can_revoke_sessions":    noDirectRelation(),
			},
		},
		Relations: map[string]*openfgav1.Userset{
			"org":   this(),
			"owner": this(),
			"editor": union(
				this(),
				computed("owner"),
				tupleToUserset("org", "admin"),
			),
			"viewer": union(
				this(),
				computed("editor"),
				tupleToUserset("org", "member"),
			),
			// Permissions
			"can_read":   computed("viewer"),
			"can_update": computed("editor"),
			"can_delete": union(
				computed("owner"),
				tupleToUserset("org", "can_delete_entity"),
			),
			"can_manage_credentials": union(
				computed("owner"),
				tupleToUserset("org", "admin"),
			),
			"can_revoke_sessions": union(
				computed("owner"),
				tupleToUserset("org", "admin"),
			),
		},
	}
}

// ──────────────────────────────────────────────────────────────────
// type app — OIDC/SAML client applications
// ──────────────────────────────────────────────────────────────────

func typeApp() *openfgav1.TypeDefinition {
	return &openfgav1.TypeDefinition{
		Type: "app",
		Metadata: &openfgav1.Metadata{
			Relations: map[string]*openfgav1.RelationMetadata{
				"org": {
					DirectlyRelatedUserTypes: []*openfgav1.RelationReference{
						{Type: "org"},
					},
				},
				"owner":  directUser(),
				"editor": directUser(),
				"viewer": directUser(),
				// Permissions
				"can_read":          noDirectRelation(),
				"can_update":        noDirectRelation(),
				"can_delete":        noDirectRelation(),
				"can_rotate_secret": noDirectRelation(),
				"can_manage_grants": noDirectRelation(),
			},
		},
		Relations: map[string]*openfgav1.Userset{
			"org":   this(),
			"owner": this(),
			"editor": union(
				this(),
				computed("owner"),
				tupleToUserset("org", "admin"),
			),
			"viewer": union(
				this(),
				computed("editor"),
			),
			// Permissions
			"can_read":   computed("viewer"),
			"can_update": computed("editor"),
			"can_delete": union(
				computed("owner"),
				tupleToUserset("org", "can_delete_entity"),
			),
			"can_rotate_secret": union(
				computed("owner"),
				tupleToUserset("org", "admin"),
			),
			"can_manage_grants": computed("editor"),
		},
	}
}

// ──────────────────────────────────────────────────────────────────
// type group — grouping container for users and apps
// Customers have asked for groups extensively.
// A group with apps + users + grants IS a project.
// ──────────────────────────────────────────────────────────────────

func typeGroup() *openfgav1.TypeDefinition {
	return &openfgav1.TypeDefinition{
		Type: "group",
		Metadata: &openfgav1.Metadata{
			Relations: map[string]*openfgav1.RelationMetadata{
				"org": {
					DirectlyRelatedUserTypes: []*openfgav1.RelationReference{
						{Type: "org"},
					},
				},
				"owner":  directUser(),
				"admin":  directUser(),
				"member": directUser(),
				// Permissions
				"can_read":           noDirectRelation(),
				"can_update":         noDirectRelation(),
				"can_delete":         noDirectRelation(),
				"can_manage_members": noDirectRelation(),
			},
		},
		Relations: map[string]*openfgav1.Userset{
			"org":   this(),
			"owner": this(),
			"admin": union(
				this(),
				computed("owner"),
			),
			"member": union(
				this(),
				computed("admin"),
			),
			// Permissions
			"can_read": union(
				computed("member"),
				tupleToUserset("org", "viewer"),
			),
			"can_update": union(
				computed("admin"),
				tupleToUserset("org", "admin"),
			),
			"can_delete": union(
				computed("owner"),
				tupleToUserset("org", "can_delete_entity"),
			),
			"can_manage_members": union(
				computed("admin"),
				tupleToUserset("org", "admin"),
			),
		},
	}
}

// ──────────────────────────────────────────────────────────────────
// type settings — cascading policies (instance → org → app)
// ──────────────────────────────────────────────────────────────────

func typeSettings() *openfgav1.TypeDefinition {
	return &openfgav1.TypeDefinition{
		Type: "settings",
		Metadata: &openfgav1.Metadata{
			Relations: map[string]*openfgav1.RelationMetadata{
				"scope_org": {
					DirectlyRelatedUserTypes: []*openfgav1.RelationReference{
						{Type: "org"},
					},
				},
				"scope_instance": {
					DirectlyRelatedUserTypes: []*openfgav1.RelationReference{
						{Type: "instance"},
					},
				},
				"can_read":  noDirectRelation(),
				"can_write": noDirectRelation(),
			},
		},
		Relations: map[string]*openfgav1.Userset{
			"scope_org":      this(),
			"scope_instance": this(),
			"can_read": union(
				tupleToUserset("scope_org", "viewer"),
				tupleToUserset("scope_instance", "viewer"),
			),
			"can_write": union(
				tupleToUserset("scope_org", "admin"),
				tupleToUserset("scope_instance", "admin"),
			),
		},
	}
}

// ──────────────────────────────────────────────────────────────────
// type session — active user sessions
// ──────────────────────────────────────────────────────────────────

func typeSession() *openfgav1.TypeDefinition {
	return &openfgav1.TypeDefinition{
		Type: "session",
		Metadata: &openfgav1.Metadata{
			Relations: map[string]*openfgav1.RelationMetadata{
				"subject": directUser(),
				"org": {
					DirectlyRelatedUserTypes: []*openfgav1.RelationReference{
						{Type: "org"},
					},
				},
				"can_read":   noDirectRelation(),
				"can_revoke": noDirectRelation(),
			},
		},
		Relations: map[string]*openfgav1.Userset{
			"subject": this(),
			"org":     this(),
			"can_read": union(
				computed("subject"),
				tupleToUserset("org", "admin"),
			),
			"can_revoke": union(
				computed("subject"),
				tupleToUserset("org", "admin"),
			),
		},
	}
}

// ──────────────────────────────────────────────────────────────────
// DSL builder helpers — reduce boilerplate
// ──────────────────────────────────────────────────────────────────

func this() *openfgav1.Userset {
	return &openfgav1.Userset{Userset: &openfgav1.Userset_This{}}
}

func computed(relation string) *openfgav1.Userset {
	return &openfgav1.Userset{
		Userset: &openfgav1.Userset_ComputedUserset{
			ComputedUserset: &openfgav1.ObjectRelation{Relation: relation},
		},
	}
}

func tupleToUserset(tupleset, computedRelation string) *openfgav1.Userset {
	return &openfgav1.Userset{
		Userset: &openfgav1.Userset_TupleToUserset{
			TupleToUserset: &openfgav1.TupleToUserset{
				Tupleset:        &openfgav1.ObjectRelation{Relation: tupleset},
				ComputedUserset: &openfgav1.ObjectRelation{Relation: computedRelation},
			},
		},
	}
}

func union(children ...*openfgav1.Userset) *openfgav1.Userset {
	if len(children) == 1 {
		return children[0]
	}
	return &openfgav1.Userset{
		Userset: &openfgav1.Userset_Union{
			Union: &openfgav1.Usersets{Child: children},
		},
	}
}

func directUser() *openfgav1.RelationMetadata {
	return &openfgav1.RelationMetadata{
		DirectlyRelatedUserTypes: []*openfgav1.RelationReference{
			{Type: "user"},
		},
	}
}

func noDirectRelation() *openfgav1.RelationMetadata {
	return &openfgav1.RelationMetadata{}
}

// ──────────────────────────────────────────────────────────────────
// AuthZ configuration — catalog-driven, replaces hardcoded maps
// ──────────────────────────────────────────────────────────────────

// AuthZConfig holds the full authz configuration for one FGA type.
// Collection (list/create) and resource (get/update/delete) may need
// different permissions and different FGA objects to check against.
type AuthZConfig struct {
	FGAType string // OpenFGA object type (e.g. "entity", "org")
	Scope   string // Default scope: "instance" or "org"

	// ResourceScope overrides Scope for resource-level ops (GET/{id}, PATCH, DELETE).
	// If empty, uses Scope. Set to "resource" to check against {fga_type}:{id}.
	ResourceScope string

	// CollectionPerms maps HTTP methods to FGA permissions for collection-level ops.
	CollectionPerms map[string]string

	// ResourcePerms maps HTTP methods to FGA permissions for resource-level ops.
	ResourcePerms map[string]string
}

// BuildAuthZFromCatalog generates the AuthZ config and route map from x-catalog.
// This is the single source of truth for all FGA middleware routing.
func BuildAuthZFromCatalog() (map[string]AuthZConfig, map[string]string) {
	configs := make(map[string]AuthZConfig)
	routes := make(map[string]string)

	// Try to load from catalog; fall back to defaults if unavailable.
	catalog, err := loadCatalog()
	if err == nil {
		for _, entry := range catalog {
			if entry.FGAType == "" || entry.Path == "" {
				continue
			}
			prefix := "/v1/" + entry.Path
			routes[prefix] = entry.FGAType

			// Only build config once per FGA type (many catalog entries share the same type).
			if _, exists := configs[entry.FGAType]; !exists {
				configs[entry.FGAType] = defaultPermsForScope(entry.FGAType, entry.FGAScope)
			}
		}
	}

	// Apply overrides for types with non-standard permission rules.
	applyOverrides(configs)

	// Non-catalog routes.
	routes["/v1/fga"] = "fga"
	if _, exists := configs["fga"]; !exists {
		configs["fga"] = defaultPermsForScope("fga", "instance")
	}

	return configs, routes
}

// loadCatalog wraps schema.Catalog() to avoid an import cycle.
// We use a function variable so tests can stub it.
var loadCatalog = func() (map[string]catalogEntry, error) {
	// We re-parse the embedded JSON directly to avoid importing schema package
	// (which would create a cycle if schema ever imports fga).
	// Instead, we import it — no cycle exists today.
	return loadCatalogFromSchema()
}

// catalogEntry is a minimal projection of schema.CatalogEntry for FGA.
type catalogEntry struct {
	Path     string
	FGAType  string
	FGAScope string
}

func loadCatalogFromSchema() (map[string]catalogEntry, error) {
	cat, err := schemaPkg.Catalog()
	if err != nil {
		return nil, err
	}
	result := make(map[string]catalogEntry, len(cat))
	for k, v := range cat {
		result[k] = catalogEntry{
			Path:     v.Path,
			FGAType:  v.FGAType,
			FGAScope: v.FGAScope,
		}
	}
	return result, nil
}

// defaultPermsForScope returns the standard permission set for a given scope.
func defaultPermsForScope(fgaType, scope string) AuthZConfig {
	switch scope {
	case "instance":
		managePerm := "can_manage_" + fgaType + "s"
		return AuthZConfig{
			FGAType: fgaType,
			Scope:   "instance",
			CollectionPerms: map[string]string{
				"GET":  "can_view_audit",
				"POST": managePerm,
			},
			ResourcePerms: map[string]string{
				"GET":    "can_view_audit",
				"PATCH":  managePerm,
				"PUT":    managePerm,
				"DELETE": managePerm,
			},
		}
	case "org":
		return AuthZConfig{
			FGAType: fgaType,
			Scope:   "org",
			CollectionPerms: map[string]string{
				"GET":  "can_read_entity",
				"POST": "can_create_entity",
			},
			ResourcePerms: map[string]string{
				"GET":    "can_read_entity",
				"PATCH":  "can_update_entity",
				"PUT":    "can_update_entity",
				"DELETE": "can_delete_entity",
			},
		}
	default:
		// No FGA — allow all
		return AuthZConfig{FGAType: fgaType, Scope: "instance"}
	}
}

// applyOverrides patches config for types that don't fit the defaults.
func applyOverrides(configs map[string]AuthZConfig) {
	// Org: collection ops check instance, resource ops check org:{id}.
	// "owner" relation inherits from parent instance via tupleToUserset,
	// so instance owners can read/manage any org.
	if _, ok := configs["org"]; ok {
		configs["org"] = AuthZConfig{
			FGAType: "org",
			Scope:   "instance",
			CollectionPerms: map[string]string{
				"GET":  "can_manage_orgs",
				"POST": "can_manage_orgs",
			},
			ResourcePerms: map[string]string{
				"GET":    "owner",
				"PATCH":  "owner",
				"PUT":    "owner",
				"DELETE": "owner",
			},
			ResourceScope: "resource", // check against org:{id}
		}
	}

	// Settings: instance-scoped management
	if _, ok := configs["settings"]; ok {
		configs["settings"] = AuthZConfig{
			FGAType: "settings",
			Scope:   "instance",
			CollectionPerms: map[string]string{
				"GET":  "can_view_audit",
				"POST": "can_manage_settings",
			},
			ResourcePerms: map[string]string{
				"GET":   "can_view_audit",
				"PATCH": "can_manage_settings",
				"PUT":   "can_manage_settings",
			},
		}
	}

	// Entity: instance-scoped (users live at instance level)
	if cfg, ok := configs["entity"]; ok {
		cfg.CollectionPerms["GET"] = "can_manage_entities"
		cfg.CollectionPerms["POST"] = "can_manage_entities"
		cfg.ResourcePerms["GET"] = "can_manage_entities"
		cfg.ResourcePerms["PATCH"] = "can_manage_entities"
		cfg.ResourcePerms["PUT"] = "can_manage_entities"
		cfg.ResourcePerms["DELETE"] = "can_manage_entities"
		configs["entity"] = cfg
	}
}

