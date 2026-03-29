// Package modules provides marketplace FGA module definitions (ADR-020, Layer 2).
//
// Each module is a set of OpenFGA type definitions that extend the immutable
// core model. Modules are installed by appending their types to the compiled
// model and flipping a feature flag.
package modules

import (
	openfgav1 "github.com/openfga/api/proto/openfga/v1"
)

// Module represents a marketplace authorization module.
type Module struct {
	// Name is the unique identifier (e.g. "rbac", "abac", "teams").
	Name string

	// Description explains what this module provides.
	Description string

	// Types returns the OpenFGA type definitions this module adds.
	Types func() []*openfgav1.TypeDefinition
}

// Registry holds all available marketplace modules.
var Registry = map[string]Module{
	"rbac":  rbacModule,
	"abac":  abacModule,
	"teams": teamsModule,
}

// ──────────────────────────────────────────────────────────────────
// RBAC Module — role + permission types
// ──────────────────────────────────────────────────────────────────

var rbacModule = Module{
	Name:        "rbac",
	Description: "Role-Based Access Control: define roles with permissions, assign to users and groups",
	Types:       rbacTypes,
}

func rbacTypes() []*openfgav1.TypeDefinition {
	return []*openfgav1.TypeDefinition{
		{
			Type: "role",
			Metadata: &openfgav1.Metadata{
				Relations: map[string]*openfgav1.RelationMetadata{
					"org": {
						DirectlyRelatedUserTypes: []*openfgav1.RelationReference{
							{Type: "org"},
						},
					},
					"project": {
						DirectlyRelatedUserTypes: []*openfgav1.RelationReference{
							{Type: "project"},
						},
					},
					"assignee": {
						DirectlyRelatedUserTypes: []*openfgav1.RelationReference{
							{Type: "user"},
							{Type: "group", RelationOrWildcard: &openfgav1.RelationReference_Relation{Relation: "member"}},
						},
					},
					"can_use": noDirectRelation(),
				},
			},
			Relations: map[string]*openfgav1.Userset{
				"org":      thisUserset(),
				"project":  thisUserset(),
				"assignee": thisUserset(),
				"can_use":  computedUserset("assignee"),
			},
		},
		{
			Type: "permission",
			Metadata: &openfgav1.Metadata{
				Relations: map[string]*openfgav1.RelationMetadata{
					"role": {
						DirectlyRelatedUserTypes: []*openfgav1.RelationReference{
							{Type: "role"},
						},
					},
					"granted": noDirectRelation(),
				},
			},
			Relations: map[string]*openfgav1.Userset{
				"role": thisUserset(),
				"granted": &openfgav1.Userset{
					Userset: &openfgav1.Userset_TupleToUserset{
						TupleToUserset: &openfgav1.TupleToUserset{
							Tupleset:        &openfgav1.ObjectRelation{Relation: "role"},
							ComputedUserset: &openfgav1.ObjectRelation{Relation: "assignee"},
						},
					},
				},
			},
		},
	}
}

// ──────────────────────────────────────────────────────────────────
// ABAC Module — policy type for attribute-based evaluation
// ──────────────────────────────────────────────────────────────────

var abacModule = Module{
	Name:        "abac",
	Description: "Attribute-Based Access Control: define policies with expr-lang conditions",
	Types:       abacTypes,
}

func abacTypes() []*openfgav1.TypeDefinition {
	return []*openfgav1.TypeDefinition{
		{
			Type: "policy",
			Metadata: &openfgav1.Metadata{
				Relations: map[string]*openfgav1.RelationMetadata{
					"org": {
						DirectlyRelatedUserTypes: []*openfgav1.RelationReference{
							{Type: "org"},
						},
					},
					"evaluator": {
						DirectlyRelatedUserTypes: []*openfgav1.RelationReference{
							{Type: "user"},
						},
					},
				},
			},
			Relations: map[string]*openfgav1.Userset{
				"org":       thisUserset(),
				"evaluator": thisUserset(),
			},
		},
	}
}

// ──────────────────────────────────────────────────────────────────
// Teams Module — hierarchical team membership
// ──────────────────────────────────────────────────────────────────

var teamsModule = Module{
	Name:        "teams",
	Description: "Hierarchical Teams: nested team membership with inheritance",
	Types:       teamsTypes,
}

func teamsTypes() []*openfgav1.TypeDefinition {
	return []*openfgav1.TypeDefinition{
		{
			Type: "team",
			Metadata: &openfgav1.Metadata{
				Relations: map[string]*openfgav1.RelationMetadata{
					"org": {
						DirectlyRelatedUserTypes: []*openfgav1.RelationReference{
							{Type: "org"},
						},
					},
					"parent": {
						DirectlyRelatedUserTypes: []*openfgav1.RelationReference{
							{Type: "team"},
						},
					},
					"lead": {
						DirectlyRelatedUserTypes: []*openfgav1.RelationReference{
							{Type: "user"},
						},
					},
					"member": {
						DirectlyRelatedUserTypes: []*openfgav1.RelationReference{
							{Type: "user"},
							{Type: "team", RelationOrWildcard: &openfgav1.RelationReference_Relation{Relation: "member"}},
						},
					},
				},
			},
			Relations: map[string]*openfgav1.Userset{
				"org":    thisUserset(),
				"parent": thisUserset(),
				"lead":   thisUserset(),
				"member": &openfgav1.Userset{
					Userset: &openfgav1.Userset_Union{
						Union: &openfgav1.Usersets{
							Child: []*openfgav1.Userset{
								thisUserset(),
								computedUserset("lead"),
								{
									Userset: &openfgav1.Userset_TupleToUserset{
										TupleToUserset: &openfgav1.TupleToUserset{
											Tupleset:        &openfgav1.ObjectRelation{Relation: "parent"},
											ComputedUserset: &openfgav1.ObjectRelation{Relation: "member"},
										},
									},
								},
							},
						},
					},
				},
			},
		},
	}
}

// ── helpers (local to modules, avoid import cycle with fga package) ──

func thisUserset() *openfgav1.Userset {
	return &openfgav1.Userset{Userset: &openfgav1.Userset_This{}}
}

func computedUserset(relation string) *openfgav1.Userset {
	return &openfgav1.Userset{
		Userset: &openfgav1.Userset_ComputedUserset{
			ComputedUserset: &openfgav1.ObjectRelation{Relation: relation},
		},
	}
}

func noDirectRelation() *openfgav1.RelationMetadata {
	return &openfgav1.RelationMetadata{}
}
