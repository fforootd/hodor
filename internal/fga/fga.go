// Package fga provides an embedded OpenFGA authorization engine.
// It shares the application's SQLite (or Postgres) database and exposes
// Check / Write / ListObjects for both internal Zitadel authorization
// and customer-facing FGA-as-a-service.
package fga

import (
	"context"
	"database/sql"
	"fmt"
	"sync"

	"github.com/zitadel/zitadel/internal/fga/modules"
	"github.com/zitadel/zitadel/internal/httputil"
	"github.com/zitadel/zitadel/internal/logging"

	openfgav1 "github.com/openfga/api/proto/openfga/v1"
	"github.com/openfga/openfga/pkg/server"
	"github.com/openfga/openfga/pkg/storage/sqlcommon"
	sqliteds "github.com/openfga/openfga/pkg/storage/sqlite"
	"google.golang.org/protobuf/types/known/wrapperspb"
)

// Service wraps an embedded OpenFGA server.
type Service struct {
	srv            *server.Server
	db             *sql.DB
	dialect        string          // "sqlite", "d1", "libsql", "postgres"
	storeID        string          // internal _system store (default instance)
	modelID        string          // current authorization model ID
	enabledModules map[string]bool // marketplace modules currently enabled

	// Per-instance FGA store cache: instance_id → store_id.
	instanceStores sync.Map
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

	svc := &Service{srv: srv, db: db, dialect: dialect, enabledModules: make(map[string]bool)}

	// 4. Ensure the internal _system store exists.
	if err := svc.ensureSystemStore(ctx); err != nil {
		return nil, fmt.Errorf("fga: ensure system store: %w", err)
	}

	logging.Printf("[fga] embedded OpenFGA ready (store=%s)", svc.storeID)

	// 5. Load the authorization model (idempotent).
	if err := svc.ensureAuthModel(ctx); err != nil {
		return nil, fmt.Errorf("fga: ensure auth model: %w", err)
	}

	// 6. Seed the default instance → store mapping.
	if err := svc.seedDefaultInstanceStore(ctx); err != nil {
		return nil, fmt.Errorf("fga: seed default instance store: %w", err)
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
	storeID, err := s.StoreForInstance(ctx)
	if err != nil {
		return false, fmt.Errorf("fga: resolve store: %w", err)
	}
	// For the default store we use the cached model ID. For per-instance
	// stores the model was written at creation time; passing empty string
	// tells OpenFGA to use the latest model in that store.
	modelID := s.modelID
	if storeID != s.storeID {
		modelID = ""
	}
	resp, err := s.srv.Check(ctx, &openfgav1.CheckRequest{
		StoreId:              storeID,
		AuthorizationModelId: modelID,
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

// WriteTuple adds a relationship tuple to the current instance's store.
func (s *Service) WriteTuple(ctx context.Context, user, relation, object string) error {
	storeID, err := s.StoreForInstance(ctx)
	if err != nil {
		return fmt.Errorf("fga: resolve store: %w", err)
	}
	_, err = s.srv.Write(ctx, &openfgav1.WriteRequest{
		StoreId: storeID,
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

// DeleteTuple removes a relationship tuple from the current instance's store.
func (s *Service) DeleteTuple(ctx context.Context, user, relation, object string) error {
	storeID, err := s.StoreForInstance(ctx)
	if err != nil {
		return fmt.Errorf("fga: resolve store: %w", err)
	}
	_, err = s.srv.Write(ctx, &openfgav1.WriteRequest{
		StoreId: storeID,
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
	storeID, err := s.StoreForInstance(ctx)
	if err != nil {
		return nil, fmt.Errorf("fga: resolve store: %w", err)
	}

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
		StoreId:  storeID,
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

// ReadAllTuples returns all tuples in the current instance's store by querying the
// underlying OpenFGA tuple table directly. The OpenFGA Read API requires
// at least one filter, so this bypasses it for the "show all" use case.
func (s *Service) ReadAllTuples(ctx context.Context) ([]map[string]string, error) {
	storeID, err := s.StoreForInstance(ctx)
	if err != nil {
		return nil, fmt.Errorf("fga: resolve store: %w", err)
	}

	rows, err := s.db.QueryContext(ctx,
		`SELECT user_object_type || ':' || user_object_id, relation, object_type || ':' || object_id
		 FROM tuple WHERE store = ? ORDER BY object_type, object_id, relation LIMIT 500`, storeID)
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
	storeID, err := s.StoreForInstance(ctx)
	if err != nil {
		return nil, fmt.Errorf("fga: resolve store: %w", err)
	}
	resp, err := s.srv.Expand(ctx, &openfgav1.ExpandRequest{
		StoreId: storeID,
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

// ──────────────────────────────────────────────────────────────────
// Per-instance store scoping
// ──────────────────────────────────────────────────────────────────

// StoreForInstance resolves the FGA store_id for the current instance
// (extracted from ctx). It checks the in-memory cache first, then the
// fga_instance_stores table, and creates a new store if none exists.
func (s *Service) StoreForInstance(ctx context.Context) (string, error) {
	instanceID := httputil.InstanceIDFromContext(ctx)

	// Fast path: check in-memory cache.
	if cached, ok := s.instanceStores.Load(instanceID); ok {
		return cached.(string), nil
	}

	// Slow path: query the database.
	var storeID string
	err := s.db.QueryRowContext(ctx,
		`SELECT store_id FROM fga_instance_stores WHERE instance_id = ?`, instanceID,
	).Scan(&storeID)
	if err == nil {
		s.instanceStores.Store(instanceID, storeID)
		return storeID, nil
	}
	if err != sql.ErrNoRows {
		return "", fmt.Errorf("fga: query instance store: %w", err)
	}

	// Not found — create a new store for this instance.
	return s.EnsureInstanceStore(ctx, instanceID)
}

// EnsureInstanceStore creates a new FGA store for the given instance,
// loads the authorization model into it, persists the mapping, and
// caches it. Returns the new store_id.
func (s *Service) EnsureInstanceStore(ctx context.Context, instanceID string) (string, error) {
	// Create a new FGA store named after the instance.
	createResp, err := s.srv.CreateStore(ctx, &openfgav1.CreateStoreRequest{
		Name: "instance_" + instanceID,
	})
	if err != nil {
		return "", fmt.Errorf("fga: create store for instance %q: %w", instanceID, err)
	}
	storeID := createResp.GetId()

	// Load the authorization model into the new store.
	modelResp, err := s.srv.WriteAuthorizationModel(ctx, &openfgav1.WriteAuthorizationModelRequest{
		StoreId:         storeID,
		SchemaVersion:   "1.1",
		TypeDefinitions: s.buildFullModel(),
	})
	if err != nil {
		return "", fmt.Errorf("fga: write auth model for instance %q: %w", instanceID, err)
	}
	_ = modelResp // model ID is per-store; the instance will use latest

	// Persist the mapping.
	_, err = s.db.ExecContext(ctx,
		`INSERT INTO fga_instance_stores (instance_id, store_id) VALUES (?, ?)
		 ON CONFLICT (instance_id) DO NOTHING`, instanceID, storeID)
	if err != nil {
		return "", fmt.Errorf("fga: insert instance store mapping: %w", err)
	}

	// Cache it.
	s.instanceStores.Store(instanceID, storeID)
	logging.Printf("[fga] created store for instance %q (store=%s)", instanceID, storeID)
	return storeID, nil
}

// seedDefaultInstanceStore ensures the "default" instance is mapped to
// the _system store created at startup. Idempotent.
func (s *Service) seedDefaultInstanceStore(ctx context.Context) error {
	_, err := s.db.ExecContext(ctx,
		`INSERT INTO fga_instance_stores (instance_id, store_id) VALUES (?, ?)
		 ON CONFLICT (instance_id) DO NOTHING`, httputil.DefaultInstanceID, s.storeID)
	if err != nil {
		return fmt.Errorf("seed default instance store: %w", err)
	}
	s.instanceStores.Store(httputil.DefaultInstanceID, s.storeID)
	return nil
}

// buildFullModel returns the full type definitions including any enabled modules.
func (s *Service) buildFullModel() []*openfgav1.TypeDefinition {
	typeDefs := ZitadelModel()
	for name := range s.enabledModules {
		if m, ok := modules.Registry[name]; ok {
			typeDefs = append(typeDefs, m.Types()...)
		}
	}
	return typeDefs
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

	// Per-instance store mapping table.
	instanceStoreDDL := `
	CREATE TABLE IF NOT EXISTS fga_instance_stores (
		instance_id TEXT PRIMARY KEY,
		store_id    TEXT NOT NULL
	);`
	if _, err := db.Exec(instanceStoreDDL); err != nil {
		return fmt.Errorf("create fga_instance_stores table: %w", err)
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
	logging.Printf("[fga] authorization model loaded (user, instance, org, group, project, app, settings, session)")
	return nil
}

// ──────────────────────────────────────────────────────────────────
// Module lifecycle (ADR-020, Layer 2)
// ──────────────────────────────────────────────────────────────────

// EnableModule installs a marketplace module by appending its types
// to the authorization model. The model is compiled in memory first;
// if compilation fails (e.g. type name conflict), the installation
// is rejected without writing to OpenFGA.
func (s *Service) EnableModule(ctx context.Context, moduleName string) error {
	mod, ok := modules.Registry[moduleName]
	if !ok {
		return fmt.Errorf("fga: unknown module %q", moduleName)
	}

	if s.enabledModules[moduleName] {
		return nil // already enabled
	}

	// Build the compiled model: core + enabled modules + new module.
	typeDefs := ZitadelModel()
	for name := range s.enabledModules {
		if m, ok := modules.Registry[name]; ok {
			typeDefs = append(typeDefs, m.Types()...)
		}
	}
	typeDefs = append(typeDefs, mod.Types()...)

	// Validate: check for duplicate type names.
	seen := make(map[string]bool)
	for _, td := range typeDefs {
		if seen[td.GetType()] {
			return fmt.Errorf("fga: module %q conflicts with existing type %q", moduleName, td.GetType())
		}
		seen[td.GetType()] = true
	}

	// Write the compiled model to the current instance's store.
	storeID, err := s.StoreForInstance(ctx)
	if err != nil {
		return fmt.Errorf("fga: resolve store for module enable: %w", err)
	}
	resp, err := s.srv.WriteAuthorizationModel(ctx, &openfgav1.WriteAuthorizationModelRequest{
		StoreId:         storeID,
		SchemaVersion:   "1.1",
		TypeDefinitions: typeDefs,
	})
	if err != nil {
		return fmt.Errorf("fga: enable module %q: %w", moduleName, err)
	}

	s.modelID = resp.GetAuthorizationModelId()
	s.enabledModules[moduleName] = true
	logging.Printf("[fga] module %q enabled (model=%s)", moduleName, s.modelID)
	return nil
}

// DisableModule removes a marketplace module by recompiling the model
// without its types. Existing tuples referencing module types become
// orphaned (check calls will return false).
func (s *Service) DisableModule(ctx context.Context, moduleName string) error {
	if !s.enabledModules[moduleName] {
		return nil // not enabled
	}

	// Rebuild without this module.
	typeDefs := ZitadelModel()
	for name := range s.enabledModules {
		if name == moduleName {
			continue
		}
		if m, ok := modules.Registry[name]; ok {
			typeDefs = append(typeDefs, m.Types()...)
		}
	}

	storeID, err := s.StoreForInstance(ctx)
	if err != nil {
		return fmt.Errorf("fga: resolve store for module disable: %w", err)
	}
	resp, err := s.srv.WriteAuthorizationModel(ctx, &openfgav1.WriteAuthorizationModelRequest{
		StoreId:         storeID,
		SchemaVersion:   "1.1",
		TypeDefinitions: typeDefs,
	})
	if err != nil {
		return fmt.Errorf("fga: disable module %q: %w", moduleName, err)
	}

	s.modelID = resp.GetAuthorizationModelId()
	delete(s.enabledModules, moduleName)
	logging.Printf("[fga] module %q disabled (model=%s)", moduleName, s.modelID)
	return nil
}

// EnabledModules returns the names of currently enabled marketplace modules.
func (s *Service) EnabledModules() []string {
	result := make([]string, 0, len(s.enabledModules))
	for name := range s.enabledModules {
		result = append(result, name)
	}
	return result
}
