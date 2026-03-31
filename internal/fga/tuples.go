package fga

import (
	"context"
	"fmt"
	"github.com/zitadel/zitadel/internal/logging"

	openfgav1 "github.com/openfga/api/proto/openfga/v1"
)

// WriteTuples writes multiple relationship tuples to the system store in a single request.
func (s *Service) WriteTuples(ctx context.Context, tuples ...[3]string) error {
	if len(tuples) == 0 {
		return nil
	}

	keys := make([]*openfgav1.TupleKey, len(tuples))
	for i, t := range tuples {
		keys[i] = &openfgav1.TupleKey{
			User:     t[0],
			Relation: t[1],
			Object:   t[2],
		}
	}

	_, err := s.srv.Write(ctx, &openfgav1.WriteRequest{
		StoreId: s.storeID,
		Writes: &openfgav1.WriteRequestWrites{
			TupleKeys: keys,
		},
	})
	if err != nil {
		return fmt.Errorf("fga: write tuples: %w", err)
	}
	return nil
}

// DeleteTuples removes multiple relationship tuples from the system store.
func (s *Service) DeleteTuples(ctx context.Context, tuples ...[3]string) error {
	if len(tuples) == 0 {
		return nil
	}

	keys := make([]*openfgav1.TupleKeyWithoutCondition, len(tuples))
	for i, t := range tuples {
		keys[i] = &openfgav1.TupleKeyWithoutCondition{
			User:     t[0],
			Relation: t[1],
			Object:   t[2],
		}
	}

	_, err := s.srv.Write(ctx, &openfgav1.WriteRequest{
		StoreId: s.storeID,
		Deletes: &openfgav1.WriteRequestDeletes{
			TupleKeys: keys,
		},
	})
	if err != nil {
		return fmt.Errorf("fga: delete tuples: %w", err)
	}
	return nil
}

// ListObjects returns all objects of a given type that the user has a relation to.
func (s *Service) ListObjects(ctx context.Context, user, relation, objectType string) ([]string, error) {
	resp, err := s.srv.ListObjects(ctx, &openfgav1.ListObjectsRequest{
		StoreId:  s.storeID,
		User:     user,
		Relation: relation,
		Type:     objectType,
	})
	if err != nil {
		return nil, fmt.Errorf("fga: list objects: %w", err)
	}
	return resp.GetObjects(), nil
}

// ──────────────────────────────────────────────────────────────────
// Lifecycle helpers — called by API handlers on entity CRUD
// ──────────────────────────────────────────────────────────────────

// OnBootstrap writes the initial FGA tuples for the admin user:
//   - user:{adminID} → owner → instance:self
//   - org:_global → parent → instance:self
//   - user:{adminID} → owner → org:_global
//
// If a management secret is configured, the _mgmt user also gets ownership.
// This function is idempotent: it checks if 'instance:self' already has an owner.
func (s *Service) OnBootstrap(ctx context.Context, adminID string) error {
	// 1. Check if bootstrap has already happened by looking for instance:self owner.
	existing, err := s.ReadTuples(ctx, "", "owner", "instance:self")
	if err == nil && len(existing) > 0 {
		logging.Printf("[fga] bootstrap skipped: instance:self already has %d owner(s)", len(existing))
		return nil
	}

	logging.Printf("[fga] bootstrapping tuples: admin=%s", adminID)
	return s.EnsureInstanceOwner(ctx, adminID)
}

// EnsureInstanceOwner writes the instance owner invariants for a specific user.
// Unlike OnBootstrap, it does not assume the instance is unclaimed and it only
// writes missing tuples, making it safe for break-glass recovery flows.
func (s *Service) EnsureInstanceOwner(ctx context.Context, userID string) error {
	required := [][3]string{
		{"user:" + userID, "owner", "instance:self"},
		{"instance:self", "parent", "org:_global"},
		{"user:" + userID, "owner", "org:_global"},
		{"user:_mgmt", "owner", "instance:self"},
	}

	missing := make([][3]string, 0, len(required))
	for _, tuple := range required {
		existing, err := s.ReadTuples(ctx, tuple[0], tuple[1], tuple[2])
		if err != nil {
			return fmt.Errorf("fga: ensure instance owner read %v: %w", tuple, err)
		}
		if len(existing) == 0 {
			missing = append(missing, tuple)
		}
	}
	if len(missing) == 0 {
		logging.Printf("[fga] instance owner already ensured: user=%s", userID)
		return nil
	}

	if err := s.WriteTuples(ctx, missing...); err != nil {
		return fmt.Errorf("fga: ensure instance owner write: %w", err)
	}
	logging.Printf("[fga] ensured instance owner: user=%s tuples=%d", userID, len(missing))
	return nil
}

// OnResourceCreated writes tuples when a new resource (identity) is created:
//   - user:{id} ← org relation → org:{orgID}  (note: identity FGA uses “user” type)
//   - user:{creatorID} → owner → user:{id}
//
// Since ADR-020 removed the generic “entity” FGA type, identities are scoped
// via org-level permissions (can_create_resource, can_read_resource, etc.)
// rather than resource-level entity:* checks.
func (s *Service) OnResourceCreated(ctx context.Context, userID, creatorID, orgID string) error {
	// Add the user as an org member so org-level checks work.
	return s.AddOrgMember(ctx, userID, orgID)
}

// OnResourceDeleted removes all tuples where user:{id} is the subject.
func (s *Service) OnResourceDeleted(ctx context.Context, userID string) error {
	userKey := "user:" + userID

	// Zitadel object types that might have relations to this user.
	// Since ADR-020, we don't have a generic "entity" type.
	types := []string{"org", "project", "app", "group", "session", "user", "instance"}

	var tuples [][3]string
	for _, t := range types {
		existing, err := s.ReadTuples(ctx, userKey, "", t+":")
		if err != nil {
			// If a type doesn't exist in the model yet (e.g. module types), Read might fail.
			continue
		}
		for _, entry := range existing {
			tuples = append(tuples, [3]string{userKey, entry["relation"], entry["object"]})
		}
	}

	if len(tuples) == 0 {
		return nil
	}

	logging.Printf("[fga] deleting %d tuples for user %s", len(tuples), userID)
	return s.DeleteTuples(ctx, tuples...)
}

// OnAppCreated writes tuples when a new app is created.
func (s *Service) OnAppCreated(ctx context.Context, appID, creatorID, orgID string) error {
	return s.WriteTuples(ctx,
		[3]string{"org:" + orgID, "org", "app:" + appID},
		[3]string{"user:" + creatorID, "owner", "app:" + appID},
	)
}

// OnGroupCreated writes tuples when a new group is created.
func (s *Service) OnGroupCreated(ctx context.Context, groupID, creatorID, orgID string) error {
	return s.WriteTuples(ctx,
		[3]string{"org:" + orgID, "org", "group:" + groupID},
		[3]string{"user:" + creatorID, "owner", "group:" + groupID},
	)
}

// OnOrgCreated writes tuples when a new org is created.
func (s *Service) OnOrgCreated(ctx context.Context, orgID, creatorID string) error {
	return s.WriteTuples(ctx,
		[3]string{"instance:self", "parent", "org:" + orgID},
		[3]string{"user:" + creatorID, "owner", "org:" + orgID},
	)
}

// AddOrgMember grants a user membership in an org.
func (s *Service) AddOrgMember(ctx context.Context, userID, orgID string) error {
	return s.WriteTuple(ctx, "user:"+userID, "member", "org:"+orgID)
}

// AddOrgAdmin grants a user admin privileges in an org.
func (s *Service) AddOrgAdmin(ctx context.Context, userID, orgID string) error {
	return s.WriteTuple(ctx, "user:"+userID, "admin", "org:"+orgID)
}

// RemoveOrgMember removes a user's membership from an org.
func (s *Service) RemoveOrgMember(ctx context.Context, userID, orgID string) error {
	return s.DeleteTuple(ctx, "user:"+userID, "member", "org:"+orgID)
}

// AddGroupMember adds a user to a group.
func (s *Service) AddGroupMember(ctx context.Context, userID, groupID string) error {
	return s.WriteTuple(ctx, "user:"+userID, "member", "group:"+groupID)
}

// RemoveGroupMember removes a user from a group.
func (s *Service) RemoveGroupMember(ctx context.Context, userID, groupID string) error {
	return s.DeleteTuple(ctx, "user:"+userID, "member", "group:"+groupID)
}

// OnSessionCreated writes tuples for a new session.
func (s *Service) OnSessionCreated(ctx context.Context, sessionID, userID, orgID string) error {
	return s.WriteTuples(ctx,
		[3]string{"user:" + userID, "subject", "session:" + sessionID},
		[3]string{"org:" + orgID, "org", "session:" + sessionID},
	)
}

// ──────────────────────────────────────────────────────────────────
// Project lifecycle helpers (ADR-020: project is a sealed primitive)
// ──────────────────────────────────────────────────────────────────

// OnProjectCreated writes tuples when a new project is created.
func (s *Service) OnProjectCreated(ctx context.Context, projectID, creatorID, orgID string) error {
	return s.WriteTuples(ctx,
		[3]string{"org:" + orgID, "org", "project:" + projectID},
		[3]string{"user:" + creatorID, "owner", "project:" + projectID},
	)
}

// AddProjectMember adds a user to a project.
func (s *Service) AddProjectMember(ctx context.Context, userID, projectID string) error {
	return s.WriteTuple(ctx, "user:"+userID, "member", "project:"+projectID)
}

// RemoveProjectMember removes a user from a project.
func (s *Service) RemoveProjectMember(ctx context.Context, userID, projectID string) error {
	return s.DeleteTuple(ctx, "user:"+userID, "member", "project:"+projectID)
}
