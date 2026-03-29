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
				"can_manage_orgs":      noDirectRelation(),
				"can_manage_settings":  noDirectRelation(),
				"can_manage_schemas":   noDirectRelation(),
				"can_manage_providers": noDirectRelation(),
				"can_view_audit":       noDirectRelation(),
				"can_manage_fga":       noDirectRelation(),
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
			"can_manage_orgs":      computed("admin"),
			"can_manage_settings":  computed("admin"),
			"can_manage_schemas":   computed("admin"),
			"can_manage_providers": computed("admin"),
			"can_view_audit":       computed("viewer"),
			"can_manage_fga":       computed("owner"),
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
			"owner":  this(),
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

// PermissionMap maps HTTP methods to FGA permissions for a given resource type.
// Used by the FGA middleware to determine which check to run.
var PermissionMap = map[string]map[string]string{
	"entity": {
		"GET":    "can_read",
		"POST":   "can_create_entity", // checked against org
		"PATCH":  "can_update",
		"PUT":    "can_update",
		"DELETE": "can_delete",
	},
	"app": {
		"GET":    "can_read",
		"POST":   "can_manage_apps", // checked against org
		"PATCH":  "can_update",
		"PUT":    "can_update",
		"DELETE": "can_delete",
	},
	"group": {
		"GET":    "can_read",
		"POST":   "can_manage_groups", // checked against org
		"PATCH":  "can_update",
		"PUT":    "can_update",
		"DELETE": "can_delete",
	},
	"org": {
		"GET":    "can_read_entity", // checked against org itself
		"POST":   "can_manage_orgs", // checked against instance
		"PATCH":  "can_update_entity",
		"PUT":    "can_update_entity",
		"DELETE": "can_delete_entity",
	},
	"settings": {
		"GET":   "can_read",
		"POST":  "can_write",
		"PATCH": "can_write",
		"PUT":   "can_write",
	},
	"schema": {
		"GET":    "can_view_audit",     // checked against instance
		"POST":   "can_manage_schemas", // checked against instance
		"PATCH":  "can_manage_schemas",
		"DELETE": "can_manage_schemas",
	},
	"provider": {
		"GET":    "can_view_audit",     // checked against instance
		"POST":   "can_manage_schemas", // checked against instance (provider management)
		"PATCH":  "can_manage_schemas",
		"DELETE": "can_manage_schemas",
	},
	"session": {
		"GET":    "can_read",
		"DELETE": "can_revoke",
	},
}

// RouteToFGAType maps API route prefixes to FGA type names.
// Built at startup from x-catalog, but these are the defaults.
var RouteToFGAType = map[string]string{
	"/v1/users":            "entity",
	"/v1/service-accounts": "entity",
	"/v1/ai-agents":        "entity",
	"/v1/apps":             "app",
	"/v1/orgs":             "org",
	"/v1/groups":           "group",
	"/v1/settings":         "settings",
	"/v1/schemas":          "schema",
	"/v1/providers":        "provider",
	"/v1/sessions":         "session",
	"/v1/rules":            "entity",
	"/v1/fga":              "schema", // FGA admin introspection — instance-level
}
