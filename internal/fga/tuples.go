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
//   - user:{adminID} → owner → instance:default
//   - org:{orgID} → parent → instance:default
//   - user:{adminID} → owner → org:{orgID}
func (s *Service) OnBootstrap(ctx context.Context, adminID, orgID string) error {
	logging.Printf("[fga] bootstrapping tuples: admin=%s org=%s", adminID, orgID)
	return s.WriteTuples(ctx,
		[3]string{"user:" + adminID, "owner", "instance:default"},
		[3]string{"instance:default", "parent", "org:" + orgID},
		[3]string{"user:" + adminID, "owner", "org:" + orgID},
		// Global org — grants access when org_id is unknown/nullable.
		[3]string{"instance:default", "parent", "org:_global"},
		[3]string{"user:" + adminID, "owner", "org:_global"},
	)
}

// OnResourceCreated writes tuples when a new entity is created:
//   - entity:{id} ← org relation → org:{orgID}
//   - user:{creatorID} → owner → entity:{id}
func (s *Service) OnResourceCreated(ctx context.Context, userID, creatorID, orgID string) error {
	return s.WriteTuples(ctx,
		[3]string{"org:" + orgID, "org", "entity:" + userID},
		[3]string{"user:" + creatorID, "owner", "entity:" + userID},
	)
}

// OnResourceDeleted removes all tuples where the entity is the object.
func (s *Service) OnResourceDeleted(ctx context.Context, userID string) error {
	// Read all tuples for this entity and delete them.
	resp, err := s.srv.Read(ctx, &openfgav1.ReadRequest{
		StoreId: s.storeID,
		TupleKey: &openfgav1.ReadRequestTupleKey{
			Object: "entity:" + userID,
		},
	})
	if err != nil {
		return fmt.Errorf("fga: read tuples for entity %s: %w", userID, err)
	}

	if len(resp.GetTuples()) == 0 {
		return nil
	}

	keys := make([]*openfgav1.TupleKeyWithoutCondition, len(resp.GetTuples()))
	for i, t := range resp.GetTuples() {
		keys[i] = &openfgav1.TupleKeyWithoutCondition{
			User:     t.GetKey().GetUser(),
			Relation: t.GetKey().GetRelation(),
			Object:   t.GetKey().GetObject(),
		}
	}

	_, err = s.srv.Write(ctx, &openfgav1.WriteRequest{
		StoreId: s.storeID,
		Deletes: &openfgav1.WriteRequestDeletes{
			TupleKeys: keys,
		},
	})
	return err
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
		[3]string{"instance:default", "parent", "org:" + orgID},
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
