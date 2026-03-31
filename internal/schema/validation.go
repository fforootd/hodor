package schema

import (
	"context"
	"database/sql"
	"encoding/json"
	"errors"
	"fmt"
	"sort"
	"strings"
	"sync"

	jsonschema "github.com/santhosh-tekuri/jsonschema/v6"
)

const (
	entitySchemaURL     = "https://zitadel.com/schemas/v1/entity"
	entityMetaSchemaURL = "https://zitadel.com/schemas/v1/entity-meta-schema"
)

var userSchemaTypes = map[string]struct{}{
	"human_user":   {},
	"service_user": {},
	"ai_agent":     {},
}

type SchemaRecord struct {
	ID     string
	Type   string
	Schema string
}

type rowQueryer interface {
	QueryRowContext(ctx context.Context, query string, args ...any) *sql.Row
}

type TableBinding struct {
	Table  string
	Filter map[string]string
}

type userPropertySpec struct {
	Format     string `json:"format"`
	Identifier bool   `json:"x-identifier"`
}

var (
	metaSchemaOnce sync.Once
	metaSchemaDoc  any
	errMetaSchema  error
	metaSchemaVal  *jsonschema.Schema
)

func IsUserSchemaType(typeName string) bool {
	_, ok := userSchemaTypes[typeName]
	return ok
}

func LoadSchemaRecord(ctx context.Context, db rowQueryer, schemaID string, dialect ...string) (*SchemaRecord, error) {
	if strings.TrimSpace(schemaID) == "" {
		return nil, errors.New("schema id is required")
	}

	var rec SchemaRecord
	err := db.QueryRowContext(ctx,
		fmt.Sprintf(`SELECT id, type, schema FROM schemas WHERE id = %s`, placeholder(dialectValue(dialect), 1)),
		schemaID,
	).Scan(&rec.ID, &rec.Type, &rec.Schema)
	if err != nil {
		if errors.Is(err, sql.ErrNoRows) {
			return nil, fmt.Errorf("schema %q not found", schemaID)
		}
		return nil, fmt.Errorf("load schema %q: %w", schemaID, err)
	}
	return &rec, nil
}

func ResolveDefaultHumanUserSchema(ctx context.Context, db rowQueryer, dialect ...string) (*SchemaRecord, error) {
	return resolveDefaultSchemaByType(ctx, db, "human_user", dialect...)
}

func ResolveSchemaForType(ctx context.Context, db rowQueryer, schemaType, schemaID string, dialect ...string) (*SchemaRecord, error) {
	if strings.TrimSpace(schemaID) == "" {
		return resolveDefaultSchemaByType(ctx, db, schemaType, dialect...)
	}

	rec, err := LoadSchemaRecord(ctx, db, schemaID, dialect...)
	if err != nil {
		return nil, err
	}
	if rec.Type != schemaType {
		return nil, fmt.Errorf("schema %q is type %q, not %q", rec.ID, rec.Type, schemaType)
	}
	return rec, nil
}

// LoadSchemaRecordCached checks the cache first, falling back to DB on miss.
func LoadSchemaRecordCached(ctx context.Context, db rowQueryer, schemaID string, cache *SchemaCache, dialect ...string) (*SchemaRecord, error) {
	if cache != nil {
		if rec, ok := cache.GetByID(schemaID); ok {
			return rec, nil
		}
	}
	rec, err := LoadSchemaRecord(ctx, db, schemaID, dialect...)
	if err != nil {
		return nil, err
	}
	if cache != nil {
		cache.PutByID(rec.ID, rec)
	}
	return rec, nil
}

// ResolveSchemaForTypeCached is the cache-aware variant of ResolveSchemaForType.
func ResolveSchemaForTypeCached(ctx context.Context, db rowQueryer, schemaType, schemaID string, cache *SchemaCache, dialect ...string) (*SchemaRecord, error) {
	if strings.TrimSpace(schemaID) == "" {
		return resolveDefaultSchemaByTypeCached(ctx, db, schemaType, cache, dialect...)
	}

	rec, err := LoadSchemaRecordCached(ctx, db, schemaID, cache, dialect...)
	if err != nil {
		return nil, err
	}
	if rec.Type != schemaType {
		return nil, fmt.Errorf("schema %q is type %q, not %q", rec.ID, rec.Type, schemaType)
	}
	return rec, nil
}

func resolveDefaultSchemaByTypeCached(ctx context.Context, db rowQueryer, schemaType string, cache *SchemaCache, dialect ...string) (*SchemaRecord, error) {
	if cache != nil {
		if rec, ok := cache.GetDefault(schemaType); ok {
			return rec, nil
		}
	}
	rec, err := resolveDefaultSchemaByType(ctx, db, schemaType, dialect...)
	if err != nil {
		return nil, err
	}
	if cache != nil {
		cache.PutDefault(schemaType, rec)
		cache.PutByID(rec.ID, rec)
	}
	return rec, nil
}

func ResolveUserSchemaForWrite(ctx context.Context, db rowQueryer, schemaID string, dialect ...string) (*SchemaRecord, error) {
	if strings.TrimSpace(schemaID) == "" {
		return ResolveDefaultHumanUserSchema(ctx, db, dialect...)
	}

	rec, err := LoadSchemaRecord(ctx, db, schemaID, dialect...)
	if err != nil {
		return nil, err
	}
	if !IsUserSchemaType(rec.Type) {
		return nil, fmt.Errorf("schema %q is type %q, not a user schema", rec.ID, rec.Type)
	}
	return rec, nil
}

func TableBindingFromSchema(schemaJSON string) (TableBinding, error) {
	var raw struct {
		Table  string         `json:"x-table"`
		Filter map[string]any `json:"x-table-filter"`
	}
	if err := json.Unmarshal([]byte(schemaJSON), &raw); err != nil {
		return TableBinding{}, fmt.Errorf("decode schema table binding: %w", err)
	}

	binding := TableBinding{
		Table:  strings.TrimSpace(raw.Table),
		Filter: map[string]string{},
	}
	for key, value := range raw.Filter {
		text := strings.TrimSpace(fmt.Sprint(value))
		if text == "" {
			continue
		}
		binding.Filter[key] = text
	}
	return binding, nil
}

func ResolveTableBinding(ctx context.Context, db rowQueryer, schemaType string, dialect ...string) (TableBinding, *SchemaRecord, error) {
	rec, err := resolveDefaultSchemaByType(ctx, db, schemaType, dialect...)
	if err != nil {
		return TableBinding{}, nil, err
	}

	binding, err := TableBindingFromSchema(rec.Schema)
	if err != nil {
		return TableBinding{}, nil, err
	}
	if binding.Table != "" {
		return binding, rec, nil
	}

	catalog, err := Catalog()
	if err != nil {
		return TableBinding{}, nil, err
	}
	entry, ok := catalog[schemaType]
	if !ok {
		return TableBinding{}, nil, fmt.Errorf("schema type %q not found in catalog", schemaType)
	}
	if entry.Ref != "" {
		embeddedSchema, loadErr := LoadSchemaFile(entry.Ref)
		if loadErr == nil {
			binding, bindErr := TableBindingFromSchema(embeddedSchema)
			if bindErr == nil && binding.Table != "" {
				return binding, rec, nil
			}
		}
	}

	return TableBinding{}, nil, fmt.Errorf("schema type %q does not declare x-table", schemaType)
}

func dialectValue(dialect []string) string {
	if len(dialect) == 0 {
		return "sqlite"
	}
	switch strings.TrimSpace(dialect[0]) {
	case "postgres":
		return "postgres"
	default:
		return "sqlite"
	}
}

func placeholder(dialect string, index int) string {
	if dialect == "postgres" {
		return fmt.Sprintf("$%d", index)
	}
	return "?"
}

func ValidateSchemaDocument(schemaJSON []byte) error {
	doc, err := decodeJSONDocument(schemaJSON)
	if err != nil {
		return err
	}

	compiled, err := compiledMetaSchema()
	if err != nil {
		return err
	}
	if err := compiled.Validate(doc); err != nil {
		return formatValidationError("schema", err)
	}
	return nil
}

func ValidateData(schemaJSON string, data map[string]any) error {
	doc, err := decodeJSONDocument([]byte(schemaJSON))
	if err != nil {
		return err
	}

	compiler, err := newCompiler()
	if err != nil {
		return err
	}
	const resourceURL = "mem://resource.json"
	if err := compiler.AddResource(resourceURL, doc); err != nil {
		return fmt.Errorf("register schema resource: %w", err)
	}

	compiled, err := compiler.Compile(resourceURL)
	if err != nil {
		return fmt.Errorf("compile resource schema: %w", err)
	}
	if err := compiled.Validate(data); err != nil {
		return formatValidationError("data", err)
	}
	return nil
}

func MaterializeUserData(schemaJSON, identifier, displayName string, metadata map[string]any) map[string]any {
	payload := cloneObjectMap(metadata)
	if payload == nil {
		payload = map[string]any{}
	}

	if displayName != "" {
		payload["display_name"] = displayName
	}

	specs := extractUserPropertySpecs(schemaJSON)
	identifierField := selectIdentifierField(identifier, specs)
	if identifierField != "" {
		existing, ok := payload[identifierField]
		if !ok || strings.TrimSpace(fmt.Sprint(existing)) == "" {
			payload[identifierField] = identifier
		}
	}

	return payload
}

func ResolveIdentifierField(schemaJSON, identifier string) string {
	return selectIdentifierField(identifier, extractUserPropertySpecs(schemaJSON))
}

func ObjectMap(value any) (map[string]any, error) {
	if value == nil {
		return map[string]any{}, nil
	}

	data, err := json.Marshal(value)
	if err != nil {
		return nil, fmt.Errorf("marshal object: %w", err)
	}
	if string(data) == "null" {
		return map[string]any{}, nil
	}

	var out map[string]any
	if err := json.Unmarshal(data, &out); err != nil {
		return nil, fmt.Errorf("decode object: %w", err)
	}
	if out == nil {
		return map[string]any{}, nil
	}
	return out, nil
}

func MergeObjectMaps(values ...any) (map[string]any, error) {
	merged := map[string]any{}
	for _, value := range values {
		obj, err := ObjectMap(value)
		if err != nil {
			return nil, err
		}
		for k, v := range obj {
			merged[k] = v
		}
	}
	return merged, nil
}

func cloneObjectMap(src map[string]any) map[string]any {
	if src == nil {
		return nil
	}
	dst := make(map[string]any, len(src))
	for k, v := range src {
		dst[k] = v
	}
	return dst
}

func decodeJSONDocument(data []byte) (any, error) {
	var doc any
	if err := json.Unmarshal(data, &doc); err != nil {
		return nil, fmt.Errorf("invalid JSON: %w", err)
	}
	return doc, nil
}

func newCompiler() (*jsonschema.Compiler, error) {
	meta, err := metaSchemaDocument()
	if err != nil {
		return nil, err
	}

	compiler := jsonschema.NewCompiler()
	compiler.DefaultDraft(jsonschema.Draft2020)
	if err := compiler.AddResource(entityMetaSchemaURL, meta); err != nil {
		return nil, fmt.Errorf("register meta schema: %w", err)
	}
	if err := compiler.AddResource(entitySchemaURL, meta); err != nil {
		return nil, fmt.Errorf("register entity schema alias: %w", err)
	}
	return compiler, nil
}

func metaSchemaDocument() (any, error) {
	metaSchemaOnce.Do(func() {
		metaSchemaDoc, errMetaSchema = decodeJSONDocument([]byte(MetaSchema))
		if errMetaSchema != nil {
			return
		}
		compiler, err := newCompilerWithoutMeta()
		if err != nil {
			errMetaSchema = err
			return
		}
		if err := compiler.AddResource(entityMetaSchemaURL, metaSchemaDoc); err != nil {
			errMetaSchema = fmt.Errorf("register meta schema: %w", err)
			return
		}
		if err := compiler.AddResource(entitySchemaURL, metaSchemaDoc); err != nil {
			errMetaSchema = fmt.Errorf("register entity schema alias: %w", err)
			return
		}
		metaSchemaVal, errMetaSchema = compiler.Compile(entityMetaSchemaURL)
	})
	if errMetaSchema != nil {
		return nil, errMetaSchema
	}
	return metaSchemaDoc, nil
}

func compiledMetaSchema() (*jsonschema.Schema, error) {
	if _, err := metaSchemaDocument(); err != nil {
		return nil, err
	}
	return metaSchemaVal, nil
}

func newCompilerWithoutMeta() (*jsonschema.Compiler, error) {
	compiler := jsonschema.NewCompiler()
	compiler.DefaultDraft(jsonschema.Draft2020)
	return compiler, nil
}

func resolveDefaultSchemaByType(ctx context.Context, db rowQueryer, schemaType string, dialect ...string) (*SchemaRecord, error) {
	defaultPlaceholder := placeholder(dialectValue(dialect), 1)
	var rec SchemaRecord
	err := db.QueryRowContext(ctx,
		fmt.Sprintf(`SELECT id, type, schema
		 FROM schemas
		 WHERE type = %s AND is_default = true
		 ORDER BY created_at ASC
		 LIMIT 1`, defaultPlaceholder),
		schemaType,
	).Scan(&rec.ID, &rec.Type, &rec.Schema)
	if err == nil {
		return &rec, nil
	}
	if err != nil && !errors.Is(err, sql.ErrNoRows) {
		return nil, fmt.Errorf("load default %s schema: %w", schemaType, err)
	}

	err = db.QueryRowContext(ctx,
		fmt.Sprintf(`SELECT id, type, schema
		 FROM schemas
		 WHERE type = %s
		 ORDER BY version DESC, created_at ASC
		 LIMIT 1`, defaultPlaceholder),
		schemaType,
	).Scan(&rec.ID, &rec.Type, &rec.Schema)
	if err != nil {
		if errors.Is(err, sql.ErrNoRows) {
			return nil, fmt.Errorf("no %s schema configured", schemaType)
		}
		return nil, fmt.Errorf("load fallback %s schema: %w", schemaType, err)
	}
	return &rec, nil
}

func extractUserPropertySpecs(schemaJSON string) map[string]userPropertySpec {
	var raw struct {
		Properties map[string]userPropertySpec `json:"properties"`
	}
	if err := json.Unmarshal([]byte(schemaJSON), &raw); err != nil {
		return nil
	}
	return raw.Properties
}

func selectIdentifierField(identifier string, specs map[string]userPropertySpec) string {
	if identifier == "" || len(specs) == 0 {
		return ""
	}

	var identifierFields []string
	for name, spec := range specs {
		if spec.Identifier {
			identifierFields = append(identifierFields, name)
		}
	}
	if len(identifierFields) == 0 {
		return ""
	}
	sort.Strings(identifierFields)
	if len(identifierFields) == 1 {
		return identifierFields[0]
	}

	if strings.Contains(identifier, "@") {
		if field := firstMatchingIdentifierField(identifierFields, specs, func(name string, spec userPropertySpec) bool {
			return name == "email" || spec.Format == "email"
		}); field != "" {
			return field
		}
	}

	if looksLikePhoneNumber(identifier) {
		if field := firstMatchingIdentifierField(identifierFields, specs, func(name string, _ userPropertySpec) bool {
			return strings.Contains(name, "phone")
		}); field != "" {
			return field
		}
	}

	if field := firstMatchingIdentifierField(identifierFields, specs, func(name string, _ userPropertySpec) bool {
		return name == "username"
	}); field != "" {
		return field
	}

	if field := firstMatchingIdentifierField(identifierFields, specs, func(name string, _ userPropertySpec) bool {
		return name == "identifier"
	}); field != "" {
		return field
	}

	return ""
}

func firstMatchingIdentifierField(fields []string, specs map[string]userPropertySpec, match func(string, userPropertySpec) bool) string {
	for _, field := range fields {
		if match(field, specs[field]) {
			return field
		}
	}
	return ""
}

func looksLikePhoneNumber(value string) bool {
	digits := 0
	for _, r := range value {
		switch {
		case r >= '0' && r <= '9':
			digits++
		case r == '+' || r == '-' || r == ' ' || r == '(' || r == ')':
		default:
			return false
		}
	}
	return digits >= 7
}

func formatValidationError(subject string, err error) error {
	var validationErr *jsonschema.ValidationError
	if errors.As(err, &validationErr) {
		return fmt.Errorf("invalid %s: %s", subject, validationErr.Error())
	}
	return fmt.Errorf("invalid %s: %w", subject, err)
}
