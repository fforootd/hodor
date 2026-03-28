// Package fga provides an embedded OpenFGA authorization engine.
// It shares the application's SQLite (or Postgres) database and exposes
// Check / Write / ListObjects for both internal Zitadel authorization
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
	modelID string // current authorization model ID
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
		MaxTuplesPerWriteField: 100,
		MaxTypesPerModelField:  100,
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
		StoreId:              s.storeID,
		AuthorizationModelId: s.modelID,
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

// ReadTuples reads relationship tuples matching the given filter.
// Any of user, relation, object can be empty to act as a wildcard.
func (s *Service) ReadTuples(ctx context.Context, user, relation, object string) ([]map[string]string, error) {
	tupleKey := &openfgav1.ReadRequestTupleKey{}
	if user != "" {
		tupleKey.User = user
	}
	if relation != "" {
		tupleKey.Relation = relation
	}
	if object != "" {
		tupleKey.Object = object
	}

	resp, err := s.srv.Read(ctx, &openfgav1.ReadRequest{
		StoreId:  s.storeID,
		TupleKey: tupleKey,
	})
	if err != nil {
		return nil, fmt.Errorf("fga: read tuples: %w", err)
	}

	var result []map[string]string
	for _, t := range resp.GetTuples() {
		result = append(result, map[string]string{
			"user":     t.GetKey().GetUser(),
			"relation": t.GetKey().GetRelation(),
			"object":   t.GetKey().GetObject(),
		})
	}
	return result, nil
}

// ReadAllTuples returns all tuples in the system store by querying the
// underlying OpenFGA tuple table directly. The OpenFGA Read API requires
// at least one filter, so this bypasses it for the "show all" use case.
func (s *Service) ReadAllTuples(ctx context.Context) ([]map[string]string, error) {
	rows, err := s.db.QueryContext(ctx,
		`SELECT user_object_type || ':' || user_object_id, relation, object_type || ':' || object_id
		 FROM tuple WHERE store = ? ORDER BY object_type, object_id, relation LIMIT 500`, s.storeID)
	if err != nil {
		return nil, fmt.Errorf("fga: read all tuples: %w", err)
	}
	defer rows.Close()

	result := make([]map[string]string, 0)
	for rows.Next() {
		var user, relation, object string
		if err := rows.Scan(&user, &relation, &object); err != nil {
			continue
		}
		result = append(result, map[string]string{
			"user":     user,
			"relation": relation,
			"object":   object,
		})
	}
	if err := rows.Err(); err != nil {
		return nil, fmt.Errorf("fga: read all tuples iter: %w", err)
	}
	return result, nil
}

// Expand returns the userset tree for a relation on an object.
func (s *Service) Expand(ctx context.Context, relation, object string) (any, error) {
	resp, err := s.srv.Expand(ctx, &openfgav1.ExpandRequest{
		StoreId: s.storeID,
		TupleKey: &openfgav1.ExpandRequestTupleKey{
			Relation: relation,
			Object:   object,
		},
	})
	if err != nil {
		return nil, fmt.Errorf("fga: expand: %w", err)
	}
	return resp.GetTree(), nil
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

	// Create the goose version table expected by OpenFGA's readiness check.
	// OpenFGA's datastore verifies goose_db_version >= 5 before allowing operations.
	gooseDDL := `
	CREATE TABLE IF NOT EXISTS goose_db_version (
		id INTEGER PRIMARY KEY AUTOINCREMENT,
		version_id INTEGER NOT NULL,
		is_applied INTEGER NOT NULL,
		tstamp TIMESTAMP DEFAULT CURRENT_TIMESTAMP
	);`
	if _, err := db.Exec(gooseDDL); err != nil {
		return fmt.Errorf("create goose_db_version table: %w", err)
	}

	// Insert version 5 if not already present (idempotent).
	var count int
	_ = db.QueryRow("SELECT COUNT(*) FROM goose_db_version WHERE version_id = 5").Scan(&count)
	if count == 0 {
		_, err := db.Exec("INSERT INTO goose_db_version (version_id, is_applied) VALUES (5, 1)")
		if err != nil {
			return fmt.Errorf("insert goose version: %w", err)
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

// ensureAuthModel writes the Zitadel authorization model (idempotent).
// Uses the full model from model.go: user, instance, org, entity, app,
// group, settings, session with hierarchical role inheritance.
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
		s.modelID = models.GetAuthorizationModels()[0].GetId()
		return nil // model already loaded
	}

	// Write the full Zitadel authorization model.
	resp, err := s.srv.WriteAuthorizationModel(ctx, &openfgav1.WriteAuthorizationModelRequest{
		StoreId:         s.storeID,
		SchemaVersion:   "1.1",
		TypeDefinitions: ZitadelModel(),
	})
	if err != nil {
		return fmt.Errorf("write auth model: %w", err)
	}

	s.modelID = resp.GetAuthorizationModelId()
	log.Printf("[fga] authorization model loaded (user, instance, org, entity, app, group, settings, session)")
	return nil
}
