// Package fga provides an embedded OpenFGA authorization engine.
// It shares the application's SQLite (or Postgres) database and exposes
// Check / Write / ListObjects for both internal ZITADEL authorization
// and customer-facing FGA-as-a-service.
package fga

import (
	"context"
	"database/sql"
	"fmt"
	"log"

	openfgav1 "github.com/openfga/api/proto/openfga/v1"
	"github.com/openfga/openfga/pkg/server"
	"github.com/openfga/openfga/pkg/storage/sqlcommon"
	sqliteds "github.com/openfga/openfga/pkg/storage/sqlite"
	"google.golang.org/protobuf/types/known/wrapperspb"
)

// Service wraps an embedded OpenFGA server.
type Service struct {
	srv     *server.Server
	db      *sql.DB
	storeID string // internal _system store
}

// New initialises the OpenFGA engine on the provided *sql.DB.
//   - Runs schema migrations (goose, idempotent).
//   - Creates an in-process OpenFGA server.
//   - Ensures the internal "_system" store exists.
func New(ctx context.Context, db *sql.DB, dialect string) (*Service, error) {
	// 1. Run OpenFGA migrations via goose.
	if err := runMigrations(db, dialect); err != nil {
		return nil, fmt.Errorf("fga: run migrations: %w", err)
	}

	// 2. Create the SQLite/Postgres datastore.
	ds, err := sqliteds.NewWithDB(db, &sqlcommon.Config{
		MaxTypesPerModelField: 100,
	})
	if err != nil {
		return nil, fmt.Errorf("fga: create datastore: %w", err)
	}

	// 3. Create the embedded server.
	srv, err := server.NewServerWithOpts(
		server.WithDatastore(ds),
	)
	if err != nil {
		return nil, fmt.Errorf("fga: create server: %w", err)
	}

	svc := &Service{srv: srv, db: db}

	// 4. Ensure the internal _system store exists.
	if err := svc.ensureSystemStore(ctx); err != nil {
		return nil, fmt.Errorf("fga: ensure system store: %w", err)
	}

	log.Printf("[fga] embedded OpenFGA ready (store=%s)", svc.storeID)

	// 5. Load the authorization model (idempotent).
	if err := svc.ensureAuthModel(ctx); err != nil {
		return nil, fmt.Errorf("fga: ensure auth model: %w", err)
	}

	return svc, nil
}

// Server returns the underlying OpenFGA server for direct API calls.
func (s *Service) Server() *server.Server {
	return s.srv
}

// SystemStoreID returns the ID of the internal authorization store.
func (s *Service) SystemStoreID() string {
	return s.storeID
}

// Check evaluates whether (user, relation, object) is authorised.
func (s *Service) Check(ctx context.Context, user, relation, object string) (bool, error) {
	resp, err := s.srv.Check(ctx, &openfgav1.CheckRequest{
		StoreId: s.storeID,
		TupleKey: &openfgav1.CheckRequestTupleKey{
			User:     user,
			Relation: relation,
			Object:   object,
		},
	})
	if err != nil {
		return false, err
	}
	return resp.GetAllowed(), nil
}

// WriteTuple adds a relationship tuple to the system store.
func (s *Service) WriteTuple(ctx context.Context, user, relation, object string) error {
	_, err := s.srv.Write(ctx, &openfgav1.WriteRequest{
		StoreId: s.storeID,
		Writes: &openfgav1.WriteRequestWrites{
			TupleKeys: []*openfgav1.TupleKey{
				{
					User:     user,
					Relation: relation,
					Object:   object,
				},
			},
		},
	})
	return err
}

// DeleteTuple removes a relationship tuple from the system store.
func (s *Service) DeleteTuple(ctx context.Context, user, relation, object string) error {
	_, err := s.srv.Write(ctx, &openfgav1.WriteRequest{
		StoreId: s.storeID,
		Deletes: &openfgav1.WriteRequestDeletes{
			TupleKeys: []*openfgav1.TupleKeyWithoutCondition{
				{
					User:     user,
					Relation: relation,
					Object:   object,
				},
			},
		},
	})
	return err
}

// ---- internal helpers ----

// runMigrations applies OpenFGA schema tables directly via SQL DDL.
// We avoid goose here because goose global state conflicts with our
// app-level migrations. OpenFGA only has one migration (5 tables).
func runMigrations(db *sql.DB, dialect string) error {
	ddl := `
	CREATE TABLE IF NOT EXISTS store (
		id CHAR(26) PRIMARY KEY,
		name VARCHAR(64) NOT NULL,
		created_at TIMESTAMP NOT NULL,
		updated_at TIMESTAMP,
		deleted_at TIMESTAMP
	);
	CREATE TABLE IF NOT EXISTS authorization_model (
		store CHAR(26) NOT NULL,
		authorization_model_id CHAR(26) NOT NULL,
		schema_version VARCHAR(5) NOT NULL DEFAULT '1.1',
		serialized_protobuf BLOB NOT NULL,
		PRIMARY KEY (store, authorization_model_id)
	);
	CREATE TABLE IF NOT EXISTS tuple (
		store CHAR(26) NOT NULL,
		object_type VARCHAR(128) NOT NULL,
		object_id VARCHAR(128) NOT NULL,
		relation VARCHAR(50) NOT NULL,
		user_object_type VARCHAR(128) NOT NULL,
		user_object_id VARCHAR(128) NOT NULL,
		user_relation VARCHAR(50) NOT NULL,
		user_type VARCHAR(7) NOT NULL,
		ulid CHAR(26) NOT NULL,
		inserted_at TIMESTAMP NOT NULL,
		condition_name VARCHAR(256),
		condition_context BLOB,
		PRIMARY KEY (store, object_type, object_id, relation, user_object_type, user_object_id, user_relation)
	);
	CREATE TABLE IF NOT EXISTS assertion (
		store CHAR(26) NOT NULL,
		authorization_model_id CHAR(26) NOT NULL,
		assertions BLOB,
		PRIMARY KEY (store, authorization_model_id)
	);
	CREATE TABLE IF NOT EXISTS changelog (
		store CHAR(26) NOT NULL,
		object_type VARCHAR(256) NOT NULL,
		object_id VARCHAR(256) NOT NULL,
		relation VARCHAR(50) NOT NULL,
		user_object_type VARCHAR(128) NOT NULL,
		user_object_id VARCHAR(128) NOT NULL,
		user_relation VARCHAR(50) NOT NULL,
		operation INTEGER NOT NULL,
		ulid CHAR(26) NOT NULL,
		inserted_at TIMESTAMP NOT NULL,
		condition_name VARCHAR(256),
		condition_context BLOB,
		PRIMARY KEY (store, ulid, object_type)
	);`

	if _, err := db.Exec(ddl); err != nil {
		return fmt.Errorf("create openfga tables: %w", err)
	}

	// Create indexes if they don't already exist.
	indexes := []string{
		`CREATE UNIQUE INDEX IF NOT EXISTS idx_tuple_ulid ON tuple (ulid)`,
		`CREATE INDEX IF NOT EXISTS idx_reverse_lookup_user ON tuple (store, object_type, relation, user_object_type, user_object_id, user_relation)`,
		`CREATE INDEX IF NOT EXISTS idx_tuple_partial_user ON tuple (store, object_type, object_id, relation, user_object_type, user_object_id, user_relation) WHERE user_type = 'user'`,
		`CREATE INDEX IF NOT EXISTS idx_tuple_partial_userset ON tuple (store, object_type, object_id, relation, user_object_type, user_object_id, user_relation) WHERE user_type = 'userset'`,
	}
	for _, idx := range indexes {
		if _, err := db.Exec(idx); err != nil {
			return fmt.Errorf("create index: %w", err)
		}
	}

	return nil
}

// ensureSystemStore finds or creates the _system store.
func (s *Service) ensureSystemStore(ctx context.Context) error {
	// List existing stores and look for "_system".
	resp, err := s.srv.ListStores(ctx, &openfgav1.ListStoresRequest{})
	if err != nil {
		return err
	}
	for _, store := range resp.GetStores() {
		if store.GetName() == "_system" {
			s.storeID = store.GetId()
			return nil
		}
	}

	// Create the _system store.
	createResp, err := s.srv.CreateStore(ctx, &openfgav1.CreateStoreRequest{
		Name: "_system",
	})
	if err != nil {
		return fmt.Errorf("create _system store: %w", err)
	}
	s.storeID = createResp.GetId()
	return nil
}

// ensureAuthModel writes the ZITADEL authorization model (idempotent).
// Model defines: user, org (admin/member), identity (owner, org-admin permissions).
func (s *Service) ensureAuthModel(ctx context.Context) error {
	// Check if a model already exists.
	models, err := s.srv.ReadAuthorizationModels(ctx, &openfgav1.ReadAuthorizationModelsRequest{
		StoreId:  s.storeID,
		PageSize: wrapperspb.Int32(1),
	})
	if err != nil {
		return fmt.Errorf("read auth models: %w", err)
	}
	if len(models.GetAuthorizationModels()) > 0 {
		return nil // model already loaded
	}

	// Write the ZITADEL authorization model.
	_, err = s.srv.WriteAuthorizationModel(ctx, &openfgav1.WriteAuthorizationModelRequest{
		StoreId:       s.storeID,
		SchemaVersion: "1.1",
		TypeDefinitions: []*openfgav1.TypeDefinition{
			// type user — represents any identity acting as a subject
			{
				Type:      "user",
				Metadata:  &openfgav1.Metadata{},
				Relations: map[string]*openfgav1.Userset{},
			},
			// type org — organization with admin and member roles
			{
				Type: "org",
				Metadata: &openfgav1.Metadata{
					Relations: map[string]*openfgav1.RelationMetadata{
						"admin": {
							DirectlyRelatedUserTypes: []*openfgav1.RelationReference{
								{Type: "user"},
							},
						},
						"member": {
							DirectlyRelatedUserTypes: []*openfgav1.RelationReference{
								{Type: "user"},
							},
						},
					},
				},
				Relations: map[string]*openfgav1.Userset{
					"admin": {Userset: &openfgav1.Userset_This{}},
					"member": {
						Userset: &openfgav1.Userset_Union{
							Union: &openfgav1.Usersets{
								Child: []*openfgav1.Userset{
									{Userset: &openfgav1.Userset_This{}},
									{Userset: &openfgav1.Userset_ComputedUserset{
										ComputedUserset: &openfgav1.ObjectRelation{Relation: "admin"},
									}},
								},
							},
						},
					},
				},
			},
			// type identity — the core resource with owner/admin/org relations
			{
				Type: "identity",
				Metadata: &openfgav1.Metadata{
					Relations: map[string]*openfgav1.RelationMetadata{
						"owner": {
							DirectlyRelatedUserTypes: []*openfgav1.RelationReference{
								{Type: "user"},
							},
						},
						"org": {
							DirectlyRelatedUserTypes: []*openfgav1.RelationReference{
								{Type: "org"},
							},
						},
						"admin":              {},
						"can_read":           {},
						"can_edit_profile":   {},
						"can_revoke_session": {},
						"can_delete":         {},
					},
				},
				Relations: map[string]*openfgav1.Userset{
					"owner": {Userset: &openfgav1.Userset_This{}},
					"org":   {Userset: &openfgav1.Userset_This{}},
					"admin": {
						Userset: &openfgav1.Userset_TupleToUserset{
							TupleToUserset: &openfgav1.TupleToUserset{
								Tupleset:        &openfgav1.ObjectRelation{Relation: "org"},
								ComputedUserset: &openfgav1.ObjectRelation{Relation: "admin"},
							},
						},
					},
					"can_read": {
						Userset: &openfgav1.Userset_Union{
							Union: &openfgav1.Usersets{
								Child: []*openfgav1.Userset{
									{Userset: &openfgav1.Userset_ComputedUserset{
										ComputedUserset: &openfgav1.ObjectRelation{Relation: "owner"},
									}},
									{Userset: &openfgav1.Userset_ComputedUserset{
										ComputedUserset: &openfgav1.ObjectRelation{Relation: "admin"},
									}},
								},
							},
						},
					},
					"can_edit_profile": {
						Userset: &openfgav1.Userset_Union{
							Union: &openfgav1.Usersets{
								Child: []*openfgav1.Userset{
									{Userset: &openfgav1.Userset_ComputedUserset{
										ComputedUserset: &openfgav1.ObjectRelation{Relation: "owner"},
									}},
									{Userset: &openfgav1.Userset_ComputedUserset{
										ComputedUserset: &openfgav1.ObjectRelation{Relation: "admin"},
									}},
								},
							},
						},
					},
					"can_revoke_session": {
						Userset: &openfgav1.Userset_Union{
							Union: &openfgav1.Usersets{
								Child: []*openfgav1.Userset{
									{Userset: &openfgav1.Userset_ComputedUserset{
										ComputedUserset: &openfgav1.ObjectRelation{Relation: "owner"},
									}},
									{Userset: &openfgav1.Userset_ComputedUserset{
										ComputedUserset: &openfgav1.ObjectRelation{Relation: "admin"},
									}},
								},
							},
						},
					},
					"can_delete": {
						Userset: &openfgav1.Userset_ComputedUserset{
							ComputedUserset: &openfgav1.ObjectRelation{Relation: "admin"},
						},
					},
				},
			},
		},
	})
	if err != nil {
		return fmt.Errorf("write auth model: %w", err)
	}

	log.Printf("[fga] authorization model loaded (org → admin/member, identity → owner/admin/can_*)")
	return nil
}
