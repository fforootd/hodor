package catalog

import (
	"context"
	"database/sql"
	"encoding/json"
	"errors"
	"fmt"
	"sort"
	"strings"

	providers "github.com/zitadel/zitadel/internal/provider"
	"github.com/zitadel/zitadel/internal/resourcedata"
	"github.com/zitadel/zitadel/internal/schema"
)

// UpgradeReport is the result of a schema upgrade preview.
type UpgradeReport struct {
	SchemaType    string         `json:"schema_type"`
	TotalEntities int            `json:"total_entities"`
	Sampled       int            `json:"sampled"`
	Impact        ImpactSummary  `json:"impact"`
	FieldChanges  []FieldChange  `json:"field_changes"`
	SampleResults []EntityResult `json:"sample_entities"`
}

// ImpactSummary summarizes how many entities would be valid, warned, or broken.
type ImpactSummary struct {
	Valid    int `json:"valid"`
	Warnings int `json:"warnings"`
	Breaking int `json:"breaking"`
}

// FieldChange describes a structural difference between old and new schemas.
type FieldChange struct {
	Path             string `json:"path"`
	Change           string `json:"change"` // field_added, field_removed, type_changed, required_added, required_removed
	Description      string `json:"description"`
	Severity         string `json:"severity"` // info, warning, breaking
	AffectedEstimate int    `json:"affected_estimate,omitempty"`
}

// EntityResult shows how a specific entity would be affected.
type EntityResult struct {
	ID          string         `json:"id"`
	DisplayName string         `json:"display_name"`
	Status      string         `json:"status"` // valid, warning, breaking
	Changes     []EntityChange `json:"changes,omitempty"`
}

// EntityChange describes a specific issue with one entity under the new schema.
type EntityChange struct {
	Path         string `json:"path"`
	Issue        string `json:"issue"`
	CurrentValue any    `json:"current_value"`
	Suggestion   string `json:"suggestion,omitempty"`
}

type previewEntity struct {
	ID          string
	DisplayName string
	Data        map[string]any
}

// PreviewUpgrade analyzes the impact of changing a schema on existing entities.
// It validates real rows from the table declared by the schema type's x-table binding.
func PreviewUpgrade(ctx context.Context, db *sql.DB, schemaType string, newSchema map[string]any, sampleSize int, dialect ...string) (*UpgradeReport, error) {
	if sampleSize <= 0 {
		sampleSize = 10
	}
	if sampleSize > 100 {
		sampleSize = 100
	}

	dialectName := previewDialect(dialect)
	binding, currentRec, err := schema.ResolveTableBinding(ctx, db, schemaType, dialectName)
	if err != nil {
		if isMissingPreviewBinding(err) {
			return &UpgradeReport{
				SchemaType:    schemaType,
				TotalEntities: 0,
				Sampled:       0,
				Impact:        ImpactSummary{},
				FieldChanges:  diffSchemas(nil, newSchema),
				SampleResults: nil,
			}, nil
		}
		return nil, err
	}

	var currentSchema map[string]any
	if currentRec != nil && strings.TrimSpace(currentRec.Schema) != "" {
		_ = json.Unmarshal([]byte(currentRec.Schema), &currentSchema)
	}
	fieldChanges := diffSchemas(currentSchema, newSchema)

	totalEntities, err := countPreviewEntities(ctx, db, schemaType, binding, dialectName)
	if err != nil {
		return nil, fmt.Errorf("count entities: %w", err)
	}

	sampleEntities, err := loadPreviewEntities(ctx, db, schemaType, binding, sampleSize, true, dialectName)
	if err != nil {
		return nil, fmt.Errorf("sample entities: %w", err)
	}

	results := make([]EntityResult, 0, len(sampleEntities))
	impact := ImpactSummary{}
	for _, entity := range sampleEntities {
		result := validateEntityAgainstSchema(entity.ID, entity.DisplayName, entity.Data, newSchema, fieldChanges)
		switch result.Status {
		case "valid":
			impact.Valid++
		case "warning":
			impact.Warnings++
		case "breaking":
			impact.Breaking++
		}
		results = append(results, result)
	}

	for i := range fieldChanges {
		fieldChanges[i].AffectedEstimate = estimateAffected(ctx, db, schemaType, binding, fieldChanges[i], totalEntities, dialectName)
	}

	return &UpgradeReport{
		SchemaType:    schemaType,
		TotalEntities: totalEntities,
		Sampled:       len(results),
		Impact:        impact,
		FieldChanges:  fieldChanges,
		SampleResults: results,
	}, nil
}

func isMissingPreviewBinding(err error) bool {
	if err == nil {
		return false
	}
	if errors.Is(err, sql.ErrNoRows) {
		return true
	}
	msg := err.Error()
	return strings.Contains(msg, "no ") && strings.Contains(msg, " schema configured") ||
		strings.Contains(msg, "not found in catalog") ||
		strings.Contains(msg, "does not declare x-table")
}

func previewDialect(dialect []string) string {
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

func previewPlaceholder(dialect string, index int) string {
	if dialect == "postgres" {
		return fmt.Sprintf("$%d", index)
	}
	return "?"
}

func countPreviewEntities(ctx context.Context, db *sql.DB, schemaType string, binding schema.TableBinding, dialect string) (int, error) {
	where, args, _ := previewWhereArgs(schemaType, binding, dialect, 1)
	query := fmt.Sprintf(`SELECT COUNT(*) FROM %s r JOIN schemas s ON r.schema_id = s.id WHERE %s`, binding.Table, where)

	var total int
	if err := db.QueryRowContext(ctx, query, args...).Scan(&total); err != nil {
		return 0, err
	}
	return total, nil
}

func loadPreviewEntities(ctx context.Context, db *sql.DB, schemaType string, binding schema.TableBinding, limit int, random bool, dialect string) ([]previewEntity, error) {
	switch binding.Table {
	case "users":
		return loadPreviewUsers(ctx, db, schemaType, binding, limit, random, dialect)
	case "apps":
		return loadPreviewApps(ctx, db, schemaType, binding, limit, random, dialect)
	case "orgs":
		return loadPreviewOrgs(ctx, db, schemaType, binding, limit, random, dialect)
	case "groups":
		return loadPreviewGroups(ctx, db, schemaType, binding, limit, random, dialect)
	case "projects":
		return loadPreviewProjects(ctx, db, schemaType, binding, limit, random, dialect)
	case "actions":
		return loadPreviewActions(ctx, db, schemaType, binding, limit, random, dialect)
	case "login_flows":
		return loadPreviewLoginFlows(ctx, db, schemaType, binding, limit, random, dialect)
	case "providers":
		return loadPreviewProviders(ctx, db, schemaType, binding, limit, random, dialect)
	default:
		return nil, fmt.Errorf("preview not implemented for table %q", binding.Table)
	}
}

func previewWhereArgs(schemaType string, binding schema.TableBinding, dialect string, start int) (string, []any, int) {
	parts := []string{fmt.Sprintf("s.type = %s", previewPlaceholder(dialect, start))}
	args := []any{schemaType}
	next := start + 1

	keys := make([]string, 0, len(binding.Filter))
	for key := range binding.Filter {
		keys = append(keys, key)
	}
	sort.Strings(keys)

	for _, key := range keys {
		parts = append(parts, fmt.Sprintf("COALESCE(r.%s,'') = %s", key, previewPlaceholder(dialect, next)))
		args = append(args, binding.Filter[key])
		next++
	}
	return strings.Join(parts, " AND "), args, next
}

func applyPreviewLimit(query string, limit int, random bool, dialect string, args []any, next int) (string, []any) {
	orderBy := " ORDER BY r.id ASC"
	if random {
		orderBy = " ORDER BY RANDOM()"
	}
	query += orderBy
	if limit > 0 {
		query += " LIMIT " + previewPlaceholder(dialect, next)
		args = append(args, limit)
	}
	return query, args
}

func loadPreviewUsers(ctx context.Context, db *sql.DB, schemaType string, binding schema.TableBinding, limit int, random bool, dialect string) ([]previewEntity, error) {
	where, args, next := previewWhereArgs(schemaType, binding, dialect, 1)
	query := fmt.Sprintf(`SELECT r.id, COALESCE(r.display_name,''), COALESCE(r.identifier,''), COALESCE(r.metadata,'{}'), COALESCE(s.schema,'{}')
		FROM users r
		JOIN schemas s ON r.schema_id = s.id
		WHERE %s`, where)
	query, args = applyPreviewLimit(query, limit, random, dialect, args, next)

	rows, err := db.QueryContext(ctx, query, args...)
	if err != nil {
		return nil, err
	}
	defer rows.Close()

	var entities []previewEntity
	for rows.Next() {
		var id, displayName, identifier, metadataJSON, schemaJSON string
		if err := rows.Scan(&id, &displayName, &identifier, &metadataJSON, &schemaJSON); err != nil {
			return nil, err
		}
		metadata := resourcedata.DecodeObjectString(metadataJSON)
		entities = append(entities, previewEntity{
			ID:          id,
			DisplayName: firstDisplayName(displayName, identifier),
			Data:        schema.MaterializeUserData(schemaJSON, identifier, displayName, metadata),
		})
	}
	return entities, rows.Err()
}

func loadPreviewApps(ctx context.Context, db *sql.DB, schemaType string, binding schema.TableBinding, limit int, random bool, dialect string) ([]previewEntity, error) {
	where, args, next := previewWhereArgs(schemaType, binding, dialect, 1)
	query := fmt.Sprintf(`SELECT r.id, COALESCE(r.name,''), COALESCE(r.app_type,''), COALESCE(r.redirect_uris,'[]'),
			COALESCE(r.grant_types,'[]'), COALESCE(r.response_types,'[]'), COALESCE(r.metadata,'{}')
		FROM apps r
		JOIN schemas s ON r.schema_id = s.id
		WHERE %s`, where)
	query, args = applyPreviewLimit(query, limit, random, dialect, args, next)

	rows, err := db.QueryContext(ctx, query, args...)
	if err != nil {
		return nil, err
	}
	defer rows.Close()

	var entities []previewEntity
	for rows.Next() {
		var id, name, appType, redirectJSON, grantJSON, responseJSON, metadataJSON string
		if err := rows.Scan(&id, &name, &appType, &redirectJSON, &grantJSON, &responseJSON, &metadataJSON); err != nil {
			return nil, err
		}
		metadata := resourcedata.DecodeObjectString(metadataJSON)
		entities = append(entities, previewEntity{
			ID:          id,
			DisplayName: firstDisplayName(name, id),
			Data: resourcedata.AppCanonicalData(
				name,
				resourcedata.StringFromAny(metadata["description"]),
				appType,
				resourcedata.StringSliceFromAny(redirectJSON),
				resourcedata.StringSliceFromAny(metadata["post_logout_redirect_uris"]),
				resourcedata.StringSliceFromAny(grantJSON),
				resourcedata.StringSliceFromAny(responseJSON),
				resourcedata.StringFromAny(metadata["logo_uri"]),
				metadata,
			),
		})
	}
	return entities, rows.Err()
}

func loadPreviewOrgs(ctx context.Context, db *sql.DB, schemaType string, binding schema.TableBinding, limit int, random bool, dialect string) ([]previewEntity, error) {
	where, args, next := previewWhereArgs(schemaType, binding, dialect, 1)
	query := fmt.Sprintf(`SELECT r.id, COALESCE(r.name,''), COALESCE(r.metadata,'{}')
		FROM orgs r
		JOIN schemas s ON r.schema_id = s.id
		WHERE %s`, where)
	query, args = applyPreviewLimit(query, limit, random, dialect, args, next)

	rows, err := db.QueryContext(ctx, query, args...)
	if err != nil {
		return nil, err
	}
	defer rows.Close()

	var entities []previewEntity
	for rows.Next() {
		var id, name, metadataJSON string
		if err := rows.Scan(&id, &name, &metadataJSON); err != nil {
			return nil, err
		}
		metadata := resourcedata.DecodeObjectString(metadataJSON)
		entities = append(entities, previewEntity{
			ID:          id,
			DisplayName: firstDisplayName(name, id),
			Data:        resourcedata.OrgCanonicalData(name, metadata),
		})
	}
	return entities, rows.Err()
}

func loadPreviewGroups(ctx context.Context, db *sql.DB, schemaType string, binding schema.TableBinding, limit int, random bool, dialect string) ([]previewEntity, error) {
	where, args, next := previewWhereArgs(schemaType, binding, dialect, 1)
	query := fmt.Sprintf(`SELECT r.id, COALESCE(r.name,''), COALESCE(r.description,''), COALESCE(r.metadata,'{}')
		FROM groups r
		JOIN schemas s ON r.schema_id = s.id
		WHERE %s`, where)
	query, args = applyPreviewLimit(query, limit, random, dialect, args, next)

	rows, err := db.QueryContext(ctx, query, args...)
	if err != nil {
		return nil, err
	}
	defer rows.Close()

	var entities []previewEntity
	for rows.Next() {
		var id, name, description, metadataJSON string
		if err := rows.Scan(&id, &name, &description, &metadataJSON); err != nil {
			return nil, err
		}
		metadata := resourcedata.DecodeObjectString(metadataJSON)
		entities = append(entities, previewEntity{
			ID:          id,
			DisplayName: firstDisplayName(name, id),
			Data:        resourcedata.GroupCanonicalData(name, description, metadata),
		})
	}
	return entities, rows.Err()
}

func loadPreviewProjects(ctx context.Context, db *sql.DB, schemaType string, binding schema.TableBinding, limit int, random bool, dialect string) ([]previewEntity, error) {
	where, args, next := previewWhereArgs(schemaType, binding, dialect, 1)
	query := fmt.Sprintf(`SELECT r.id, COALESCE(r.name,''), COALESCE(r.description,''), COALESCE(r.metadata,'{}')
		FROM projects r
		JOIN schemas s ON r.schema_id = s.id
		WHERE %s`, where)
	query, args = applyPreviewLimit(query, limit, random, dialect, args, next)

	rows, err := db.QueryContext(ctx, query, args...)
	if err != nil {
		return nil, err
	}
	defer rows.Close()

	var entities []previewEntity
	for rows.Next() {
		var id, name, description, metadataJSON string
		if err := rows.Scan(&id, &name, &description, &metadataJSON); err != nil {
			return nil, err
		}
		metadata := resourcedata.DecodeObjectString(metadataJSON)
		entities = append(entities, previewEntity{
			ID:          id,
			DisplayName: firstDisplayName(name, id),
			Data:        resourcedata.ProjectCanonicalData(name, description, metadata),
		})
	}
	return entities, rows.Err()
}

func loadPreviewActions(ctx context.Context, db *sql.DB, schemaType string, binding schema.TableBinding, limit int, random bool, dialect string) ([]previewEntity, error) {
	where, args, next := previewWhereArgs(schemaType, binding, dialect, 1)
	query := fmt.Sprintf(`SELECT r.id, COALESCE(r.name,''), COALESCE(r.hook,''), COALESCE(r.action_type,''), COALESCE(r.trigger_expr,''),
			COALESCE(r.config,'{}'), COALESCE(r.priority,0), COALESCE(r.enabled,1), COALESCE(r.fail_open,0), COALESCE(r.timeout_ms,5000), COALESCE(r.metadata,'{}')
		FROM actions r
		JOIN schemas s ON r.schema_id = s.id
		WHERE %s`, where)
	query, args = applyPreviewLimit(query, limit, random, dialect, args, next)

	rows, err := db.QueryContext(ctx, query, args...)
	if err != nil {
		return nil, err
	}
	defer rows.Close()

	var entities []previewEntity
	for rows.Next() {
		var id, name, hook, actionType, triggerExpr, configJSON, metadataJSON string
		var priority int
		var enabled, failOpen bool
		var timeoutMS int
		if err := rows.Scan(&id, &name, &hook, &actionType, &triggerExpr, &configJSON, &priority, &enabled, &failOpen, &timeoutMS, &metadataJSON); err != nil {
			return nil, err
		}
		metadata := resourcedata.DecodeObjectString(metadataJSON)
		config := resourcedata.DecodeObjectString(configJSON)
		data, _, err := resourcedata.BuildActionSchemaData(name, hook, actionType, triggerExpr, priority, enabled, config, metadata)
		if err != nil {
			return nil, err
		}
		data["fail_open"] = failOpen
		data["timeout_ms"] = timeoutMS
		entities = append(entities, previewEntity{
			ID:          id,
			DisplayName: firstDisplayName(name, id),
			Data:        data,
		})
	}
	return entities, rows.Err()
}

func loadPreviewLoginFlows(ctx context.Context, db *sql.DB, schemaType string, binding schema.TableBinding, limit int, random bool, dialect string) ([]previewEntity, error) {
	where, args, next := previewWhereArgs(schemaType, binding, dialect, 1)
	query := fmt.Sprintf(`SELECT r.id, COALESCE(r.name,''), COALESCE(r.strategy,''), COALESCE(r.is_default,0), COALESCE(r.state,''),
			COALESCE(r.priority,0), COALESCE(r.audience,'{}'), COALESCE(r.auth_methods,'{}'), COALESCE(r.config,'{}'), COALESCE(r.metadata,'{}')
		FROM login_flows r
		JOIN schemas s ON r.schema_id = s.id
		WHERE %s`, where)
	query, args = applyPreviewLimit(query, limit, random, dialect, args, next)

	rows, err := db.QueryContext(ctx, query, args...)
	if err != nil {
		return nil, err
	}
	defer rows.Close()

	var entities []previewEntity
	for rows.Next() {
		var id, name, strategy, state, audienceJSON, authMethodsJSON, configJSON, metadataJSON string
		var isDefault bool
		var priority int
		if err := rows.Scan(&id, &name, &strategy, &isDefault, &state, &priority, &audienceJSON, &authMethodsJSON, &configJSON, &metadataJSON); err != nil {
			return nil, err
		}
		metadata := resourcedata.DecodeObjectString(metadataJSON)
		audience := resourcedata.DecodeObjectString(audienceJSON)
		authMethods := resourcedata.DecodeObjectString(authMethodsJSON)
		config := resourcedata.DecodeObjectString(configJSON)
		data, _, err := resourcedata.BuildLoginFlowSchemaData(name, strategy, isDefault, state, priority, audience, authMethods, config, metadata)
		if err != nil {
			return nil, err
		}
		entities = append(entities, previewEntity{
			ID:          id,
			DisplayName: firstDisplayName(name, id),
			Data:        data,
		})
	}
	return entities, rows.Err()
}

func loadPreviewProviders(ctx context.Context, db *sql.DB, schemaType string, binding schema.TableBinding, limit int, random bool, dialect string) ([]previewEntity, error) {
	where, args, next := previewWhereArgs(schemaType, binding, dialect, 1)
	query := fmt.Sprintf(`SELECT r.id, COALESCE(r.name,''), COALESCE(r.protocol,''), COALESCE(r.template,''), COALESCE(r.config,'{}'),
			COALESCE(r.claim_overrides,'{}'), COALESCE(r.auto_register,1), COALESCE(r.enabled,1), COALESCE(r.display_order,0),
			COALESCE(r.target_schema_id,''), COALESCE(r.target_schema_type,''), COALESCE(r.metadata,'{}')
		FROM providers r
		JOIN schemas s ON r.schema_id = s.id
		WHERE %s`, where)
	query, args = applyPreviewLimit(query, limit, random, dialect, args, next)

	rows, err := db.QueryContext(ctx, query, args...)
	if err != nil {
		return nil, err
	}
	defer rows.Close()

	var entities []previewEntity
	for rows.Next() {
		var id, name, protocol, templateID, configJSON, claimsJSON, targetSchemaID, targetSchemaType, metadataJSON string
		var autoRegister, enabled bool
		var displayOrder int
		if err := rows.Scan(&id, &name, &protocol, &templateID, &configJSON, &claimsJSON, &autoRegister, &enabled, &displayOrder, &targetSchemaID, &targetSchemaType, &metadataJSON); err != nil {
			return nil, err
		}

		var prov providers.Provider
		if strings.TrimSpace(metadataJSON) != "" && metadataJSON != "{}" {
			_ = json.Unmarshal([]byte(metadataJSON), &prov)
		}
		if prov.DisplayName == "" {
			prov.DisplayName = name
		}
		if prov.Protocol == "" {
			prov.Protocol = protocol
		}
		if prov.CatalogRef.TemplateID == "" {
			prov.CatalogRef.TemplateID = templateID
		}
		if prov.Connection == nil {
			prov.Connection = resourcedata.DecodeObjectString(configJSON)
		}
		if prov.Mapping.Claims == nil {
			prov.Mapping.Claims = map[string]string{}
			for key, value := range resourcedata.DecodeObjectString(claimsJSON) {
				if text, ok := value.(string); ok {
					prov.Mapping.Claims[key] = text
				}
			}
		}
		if prov.Target.SchemaID == "" {
			prov.Target.SchemaID = targetSchemaID
		}
		if prov.Target.SchemaType == "" {
			prov.Target.SchemaType = targetSchemaType
		}
		if prov.Linking.Mode == "" && !autoRegister {
			prov.Linking.Mode = providers.LinkModeLinkOnly
		}
		prov.Enabled = enabled
		if prov.UI == nil {
			prov.UI = map[string]any{}
		}
		if _, ok := prov.UI["display_order"]; !ok && displayOrder != 0 {
			prov.UI["display_order"] = displayOrder
		}
		data, err := providers.SchemaData(prov)
		if err != nil {
			return nil, err
		}
		entities = append(entities, previewEntity{
			ID:          id,
			DisplayName: firstDisplayName(prov.DisplayName, id),
			Data:        data,
		})
	}
	return entities, rows.Err()
}

func firstDisplayName(values ...string) string {
	for _, value := range values {
		if strings.TrimSpace(value) != "" {
			return strings.TrimSpace(value)
		}
	}
	return ""
}

// diffSchemas compares old and new JSON schemas to find structural changes.
func diffSchemas(oldSchema, newSchema map[string]any) []FieldChange {
	var changes []FieldChange

	oldProps := extractProperties(oldSchema)
	newProps := extractProperties(newSchema)
	oldRequired := extractRequired(oldSchema)
	newRequired := extractRequired(newSchema)

	// Find added fields.
	for name, prop := range newProps {
		if _, exists := oldProps[name]; !exists {
			severity := "info"
			desc := fmt.Sprintf("New optional field '%s'", name)

			if newRequired[name] {
				severity = "breaking"
				desc = fmt.Sprintf("New required field '%s' added - existing entities without this field will be invalid", name)
			}

			propMap, _ := prop.(map[string]any)
			if defaultVal, ok := propMap["default"]; ok {
				desc += fmt.Sprintf(" (default: %v)", defaultVal)
				if severity == "breaking" {
					severity = "warning"
					desc = fmt.Sprintf("New required field '%s' with default '%v' - will apply automatically", name, defaultVal)
				}
			}

			changes = append(changes, FieldChange{
				Path:        "properties." + name,
				Change:      "field_added",
				Description: desc,
				Severity:    severity,
			})
		}
	}

	// Find removed fields.
	for name := range oldProps {
		if _, exists := newProps[name]; !exists {
			changes = append(changes, FieldChange{
				Path:        "properties." + name,
				Change:      "field_removed",
				Description: fmt.Sprintf("Field '%s' removed - existing data for this field will be orphaned", name),
				Severity:    "warning",
			})
		}
	}

	// Find type changes and newly required fields.
	for name, newProp := range newProps {
		oldProp, exists := oldProps[name]
		if !exists {
			continue
		}

		oldPropMap, _ := oldProp.(map[string]any)
		newPropMap, _ := newProp.(map[string]any)

		oldType, _ := oldPropMap["type"].(string)
		newType, _ := newPropMap["type"].(string)
		if oldType != "" && newType != "" && oldType != newType {
			changes = append(changes, FieldChange{
				Path:        "properties." + name,
				Change:      "type_changed",
				Description: fmt.Sprintf("Field '%s' type changed from '%s' to '%s'", name, oldType, newType),
				Severity:    "breaking",
			})
		}

		if newRequired[name] && !oldRequired[name] {
			changes = append(changes, FieldChange{
				Path:        "properties." + name,
				Change:      "required_added",
				Description: fmt.Sprintf("Field '%s' is now required but was optional", name),
				Severity:    "breaking",
			})
		}

		if !newRequired[name] && oldRequired[name] {
			changes = append(changes, FieldChange{
				Path:        "properties." + name,
				Change:      "required_removed",
				Description: fmt.Sprintf("Field '%s' is now optional (was required)", name),
				Severity:    "info",
			})
		}
	}

	return changes
}

// validateEntityAgainstSchema checks a single entity against the proposed schema.
func validateEntityAgainstSchema(id, displayName string, data, schema map[string]any, fieldChanges []FieldChange) EntityResult {
	result := EntityResult{
		ID:          id,
		DisplayName: displayName,
		Status:      "valid",
	}

	required := extractRequired(schema)
	props := extractProperties(schema)

	for name := range required {
		val, exists := data[name]
		if !exists || val == nil || val == "" {
			result.Changes = append(result.Changes, EntityChange{
				Path:         name,
				Issue:        "required field missing",
				CurrentValue: val,
				Suggestion:   "Set a default value or make the field optional",
			})
			result.Status = "breaking"
		}
	}

	for name, propDef := range props {
		val, exists := data[name]
		if !exists || val == nil {
			continue
		}

		propMap, _ := propDef.(map[string]any)
		expectedType, _ := propMap["type"].(string)
		if expectedType == "" {
			continue
		}

		if !isTypeCompatible(val, expectedType) {
			result.Changes = append(result.Changes, EntityChange{
				Path:         name,
				Issue:        fmt.Sprintf("type mismatch: expected %s", expectedType),
				CurrentValue: val,
				Suggestion:   fmt.Sprintf("Convert value to %s or update schema", expectedType),
			})
			if result.Status != "breaking" {
				result.Status = "warning"
			}
		}
	}

	return result
}

// estimateAffected estimates how many entities are affected by a specific field change.
func estimateAffected(ctx context.Context, db *sql.DB, schemaType string, binding schema.TableBinding, fc FieldChange, total int, dialect string) int {
	if total == 0 {
		return 0
	}

	switch fc.Change {
	case "required_added", "field_added", "field_removed":
	default:
		return total
	}

	entities, err := loadPreviewEntities(ctx, db, schemaType, binding, 0, false, dialect)
	if err != nil {
		return total
	}

	count := 0
	for _, entity := range entities {
		if fieldChangeAffectsEntity(fc, entity.Data) {
			count++
		}
	}
	return count
}

func fieldChangeAffectsEntity(fc FieldChange, data map[string]any) bool {
	fieldName := strings.TrimPrefix(fc.Path, "properties.")
	value, exists := data[fieldName]

	switch fc.Change {
	case "required_added", "field_added":
		if fc.Severity != "breaking" {
			return false
		}
		return !exists || value == nil || value == ""
	case "field_removed":
		return exists && value != nil
	default:
		return true
	}
}

// extractProperties gets the properties map from a JSON schema.
func extractProperties(schema map[string]any) map[string]any {
	if schema == nil {
		return nil
	}
	props, _ := schema["properties"].(map[string]any)
	return props
}

// extractRequired gets the required fields as a set.
func extractRequired(schema map[string]any) map[string]bool {
	result := make(map[string]bool)
	if schema == nil {
		return result
	}
	required, _ := schema["required"].([]any)
	for _, r := range required {
		if name, ok := r.(string); ok {
			result[name] = true
		}
	}
	return result
}

// isTypeCompatible checks if a value matches the expected JSON schema type.
func isTypeCompatible(val any, expectedType string) bool {
	switch expectedType {
	case "string":
		_, ok := val.(string)
		return ok
	case "integer":
		switch v := val.(type) {
		case float64:
			return v == float64(int64(v))
		case int, int64:
			return true
		case json.Number:
			_, err := v.Int64()
			return err == nil
		}
		return false
	case "number":
		switch val.(type) {
		case float64, float32, int, int64, json.Number:
			return true
		}
		return false
	case "boolean":
		_, ok := val.(bool)
		return ok
	case "object":
		_, ok := val.(map[string]any)
		return ok
	case "array":
		_, ok := val.([]any)
		return ok
	default:
		return true
	}
}
