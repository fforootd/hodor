package provider

import (
	"context"
	"database/sql"
	"encoding/json"
	"errors"
	"fmt"
	"strings"

	"github.com/zitadel/zitadel/internal/schema"
)

const (
	LinkModeCreateOrLink = "create_or_link"
	LinkModeLinkOnly     = "link_only"

	LinkMatchVerifiedEmail = "verified_email"
	LinkMatchIdentifier    = "identifier"
	LinkMatchNone          = "none"
)

type Mapping struct {
	Claims map[string]string `json:"claims,omitempty"`
}

type Target struct {
	SchemaType string `json:"schema_type,omitempty"`
	SchemaID   string `json:"schema_id,omitempty"`
}

type Linking struct {
	Mode    string `json:"mode,omitempty"`
	MatchBy string `json:"match_by,omitempty"`
}

type CatalogRef struct {
	TemplateID      string   `json:"template_id,omitempty"`
	TemplateVersion string   `json:"template_version,omitempty"`
	Official        bool     `json:"official,omitempty"`
	Capabilities    []string `json:"capabilities,omitempty"`
	LogoURL         string   `json:"logo_url,omitempty"`
	DocsURL         string   `json:"docs_url,omitempty"`
}

type Provider struct {
	ID          string         `json:"id,omitempty"`
	OrgID       string         `json:"org_id,omitempty"`
	SchemaID    string         `json:"schema_id,omitempty"`
	DisplayName string         `json:"display_name"`
	Kind        string         `json:"kind,omitempty"`
	Protocol    string         `json:"protocol"`
	Connection  map[string]any `json:"connection,omitempty"`
	Mapping     Mapping        `json:"mapping,omitempty"`
	Target      Target         `json:"target,omitempty"`
	Linking     Linking        `json:"linking,omitempty"`
	Session     map[string]any `json:"session,omitempty"`
	UI          map[string]any `json:"ui,omitempty"`
	Enabled     bool           `json:"enabled"`
	CatalogRef  CatalogRef     `json:"catalog_ref,omitempty"`
	CatalogMeta map[string]any `json:"_catalog,omitempty"`
	CreatedAt   string         `json:"created_at,omitempty"`
	UpdatedAt   string         `json:"updated_at,omitempty"`
}

type Repository struct {
	db      *sql.DB
	dialect string
}

type rowQueryer interface {
	QueryRowContext(context.Context, string, ...any) *sql.Row
}

func NewRepository(db *sql.DB, dialect ...string) *Repository {
	repoDialect := "sqlite"
	if len(dialect) > 0 && strings.TrimSpace(dialect[0]) != "" {
		repoDialect = strings.TrimSpace(dialect[0])
	}
	return &Repository{db: db, dialect: repoDialect}
}

type persistenceRecord struct {
	ID               string
	OrgID            string
	Name             string
	Protocol         string
	Template         string
	ConfigJSON       string
	OverridesJSON    string
	AutoRegister     bool
	Enabled          bool
	DisplayOrder     int
	ResourceSchemaID string
	TargetSchemaID   string
	TargetSchemaType string
	MetadataJSON     string
	CreatedAt        string
	UpdatedAt        string
}

func (r *Repository) List(ctx context.Context) ([]Provider, error) {
	rows, err := r.db.QueryContext(ctx,
		`SELECT id, COALESCE(org_id,''), name, protocol, COALESCE(template,''), COALESCE(config,'{}'),
		        COALESCE(claim_overrides,'{}'), COALESCE(auto_register,1), COALESCE(enabled,1),
		        COALESCE(display_order,0), COALESCE(schema_id,''), COALESCE(target_schema_id,''), COALESCE(target_schema_type,''), COALESCE(metadata,'{}'),
		        created_at, updated_at
		 FROM providers
		 ORDER BY display_order, name`)
	if err != nil {
		return nil, err
	}
	defer rows.Close()

	var providers []Provider
	for rows.Next() {
		rec, err := scanPersistenceRecord(rows)
		if err != nil {
			return nil, err
		}
		providers = append(providers, fromPersistenceRecord(rec))
	}
	return providers, rows.Err()
}

func (r *Repository) ListEnabled(ctx context.Context) ([]Provider, error) {
	rows, err := r.db.QueryContext(ctx,
		`SELECT id, COALESCE(org_id,''), name, protocol, COALESCE(template,''), COALESCE(config,'{}'),
		        COALESCE(claim_overrides,'{}'), COALESCE(auto_register,1), COALESCE(enabled,1),
		        COALESCE(display_order,0), COALESCE(schema_id,''), COALESCE(target_schema_id,''), COALESCE(target_schema_type,''), COALESCE(metadata,'{}'),
		        created_at, updated_at
		 FROM providers
		 WHERE enabled = 1 OR enabled = true
		 ORDER BY display_order, name`)
	if err != nil {
		return nil, err
	}
	defer rows.Close()

	var providers []Provider
	for rows.Next() {
		rec, err := scanPersistenceRecord(rows)
		if err != nil {
			return nil, err
		}
		providers = append(providers, fromPersistenceRecord(rec))
	}
	return providers, rows.Err()
}

func (r *Repository) Get(ctx context.Context, id string) (*Provider, error) {
	rec, err := r.getRecord(ctx, id)
	if err != nil {
		return nil, err
	}
	prov := fromPersistenceRecord(rec)
	return &prov, nil
}

func (r *Repository) Create(ctx context.Context, id string, prov Provider) (string, error) {
	prov = Normalize(prov)
	rec, err := toPersistenceRecord(id, prov)
	if err != nil {
		return "", err
	}
	_, err = r.db.ExecContext(ctx,
		fmt.Sprintf(`INSERT INTO providers (id, org_id, name, protocol, template, config, claim_overrides, auto_register, enabled, display_order, schema_id, target_schema_id, target_schema_type, metadata, created_at, updated_at)
		 VALUES (%s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s)`,
			r.placeholder(1), r.placeholder(2), r.placeholder(3), r.placeholder(4),
			r.placeholder(5), r.placeholder(6), r.placeholder(7), r.placeholder(8),
			r.placeholder(9), r.placeholder(10), r.placeholder(11), r.placeholder(12),
			r.placeholder(13), r.placeholder(14), r.timeExpr(), r.timeExpr()),
		rec.ID, rec.OrgID, rec.Name, rec.Protocol, rec.Template, rec.ConfigJSON,
		rec.OverridesJSON, rec.AutoRegister, rec.Enabled, rec.DisplayOrder, rec.ResourceSchemaID, rec.TargetSchemaID, rec.TargetSchemaType, rec.MetadataJSON,
	)
	if err != nil {
		return "", err
	}
	return rec.ID, nil
}

func (r *Repository) Save(ctx context.Context, prov Provider) error {
	if strings.TrimSpace(prov.ID) == "" {
		return errors.New("provider id is required")
	}
	prov = Normalize(prov)
	rec, err := toPersistenceRecord(prov.ID, prov)
	if err != nil {
		return err
	}
	_, err = r.db.ExecContext(ctx,
		fmt.Sprintf(`UPDATE providers
		 SET org_id = %s, name = %s, protocol = %s, template = %s, config = %s, claim_overrides = %s,
		     auto_register = %s, enabled = %s, display_order = %s, schema_id = %s, target_schema_id = %s, target_schema_type = %s, metadata = %s, updated_at = %s
		 WHERE id = %s`,
			r.placeholder(1), r.placeholder(2), r.placeholder(3), r.placeholder(4),
			r.placeholder(5), r.placeholder(6), r.placeholder(7), r.placeholder(8),
			r.placeholder(9), r.placeholder(10), r.placeholder(11), r.placeholder(12),
			r.placeholder(13), r.timeExpr(), r.placeholder(14)),
		rec.OrgID, rec.Name, rec.Protocol, rec.Template, rec.ConfigJSON, rec.OverridesJSON,
		rec.AutoRegister, rec.Enabled, rec.DisplayOrder, rec.ResourceSchemaID, rec.TargetSchemaID, rec.TargetSchemaType, rec.MetadataJSON, rec.ID,
	)
	return err
}

func (r *Repository) Delete(ctx context.Context, id string) (int64, error) {
	result, err := r.db.ExecContext(ctx, fmt.Sprintf(`DELETE FROM providers WHERE id = %s`, r.placeholder(1)), id)
	if err != nil {
		return 0, err
	}
	return result.RowsAffected()
}

func (r *Repository) getRecord(ctx context.Context, id string) (persistenceRecord, error) {
	var rec persistenceRecord
	err := r.db.QueryRowContext(ctx,
		fmt.Sprintf(`SELECT id, COALESCE(org_id,''), name, protocol, COALESCE(template,''), COALESCE(config,'{}'),
		        COALESCE(claim_overrides,'{}'), COALESCE(auto_register,1), COALESCE(enabled,1),
		        COALESCE(display_order,0), COALESCE(schema_id,''), COALESCE(target_schema_id,''), COALESCE(target_schema_type,''), COALESCE(metadata,'{}'),
		        created_at, updated_at
		 FROM providers WHERE id = %s`, r.placeholder(1)),
		id,
	).Scan(&rec.ID, &rec.OrgID, &rec.Name, &rec.Protocol, &rec.Template, &rec.ConfigJSON,
		&rec.OverridesJSON, &rec.AutoRegister, &rec.Enabled, &rec.DisplayOrder, &rec.ResourceSchemaID, &rec.TargetSchemaID, &rec.TargetSchemaType,
		&rec.MetadataJSON, &rec.CreatedAt, &rec.UpdatedAt)
	if err != nil {
		return persistenceRecord{}, err
	}
	return rec, nil
}

func scanPersistenceRecord(scanner interface{ Scan(dest ...any) error }) (persistenceRecord, error) {
	var rec persistenceRecord
	err := scanner.Scan(&rec.ID, &rec.OrgID, &rec.Name, &rec.Protocol, &rec.Template, &rec.ConfigJSON,
		&rec.OverridesJSON, &rec.AutoRegister, &rec.Enabled, &rec.DisplayOrder, &rec.ResourceSchemaID, &rec.TargetSchemaID, &rec.TargetSchemaType,
		&rec.MetadataJSON, &rec.CreatedAt, &rec.UpdatedAt)
	if err != nil {
		return persistenceRecord{}, err
	}
	return rec, nil
}

func Normalize(prov Provider) Provider {
	if prov.Connection == nil {
		prov.Connection = map[string]any{}
	}
	if prov.Mapping.Claims == nil {
		prov.Mapping.Claims = map[string]string{}
	}
	if prov.Session == nil {
		prov.Session = map[string]any{}
	}
	if prov.UI == nil {
		prov.UI = map[string]any{}
	}
	prov.DisplayName = strings.TrimSpace(prov.DisplayName)
	prov.SchemaID = strings.TrimSpace(prov.SchemaID)
	prov.Kind = strings.TrimSpace(prov.Kind)
	prov.Protocol = strings.TrimSpace(prov.Protocol)
	prov.Target.SchemaType = strings.TrimSpace(prov.Target.SchemaType)
	prov.Target.SchemaID = strings.TrimSpace(prov.Target.SchemaID)
	prov.Linking.Mode = strings.TrimSpace(prov.Linking.Mode)
	prov.Linking.MatchBy = strings.TrimSpace(prov.Linking.MatchBy)
	if prov.Protocol == "" {
		prov.Protocol = "oidc"
	}
	if prov.Kind == "" {
		prov.Kind = legacyKindFromTemplate(prov.CatalogRef.TemplateID)
	}
	if prov.Kind == "" {
		prov.Kind = "custom"
	}
	if prov.DisplayName == "" {
		prov.DisplayName = defaultDisplayName(prov.Kind)
	}
	if prov.Target.SchemaType == "" && prov.Target.SchemaID == "" {
		prov.Target.SchemaType = "human_user"
	}
	if prov.Linking.Mode == "" {
		prov.Linking.Mode = LinkModeCreateOrLink
	}
	if prov.Linking.MatchBy == "" {
		switch prov.Protocol {
		case "oidc", "oauth2":
			prov.Linking.MatchBy = LinkMatchVerifiedEmail
		default:
			prov.Linking.MatchBy = LinkMatchNone
		}
	}
	if prov.CatalogRef.TemplateID == "" {
		prov.CatalogRef.TemplateID = prov.Kind
	}
	if prov.CatalogMeta == nil {
		prov.CatalogMeta = map[string]any{}
	}
	return prov
}

func Redacted(prov Provider) Provider {
	if prov.Connection == nil {
		return prov
	}
	connection := cloneMap(prov.Connection)
	delete(connection, "client_secret")
	prov.Connection = connection
	return prov
}

func LegacyTemplateID(prov Provider) string {
	if prov.CatalogRef.TemplateID != "" {
		return prov.CatalogRef.TemplateID
	}
	if prov.Kind != "" {
		return prov.Kind
	}
	return "custom"
}

func LegacyAutoRegister(prov Provider) bool {
	return prov.Linking.Mode != LinkModeLinkOnly
}

func DisplayOrder(prov Provider) int {
	if prov.UI == nil {
		return 0
	}
	switch value := prov.UI["display_order"].(type) {
	case int:
		return value
	case int32:
		return int(value)
	case int64:
		return int(value)
	case float64:
		return int(value)
	default:
		return 0
	}
}

func SchemaData(prov Provider) (map[string]any, error) {
	prov = Normalize(prov)

	data := map[string]any{
		"display_name": prov.DisplayName,
		"kind":         prov.Kind,
		"protocol":     prov.Protocol,
		"connection":   cloneMap(prov.Connection),
		"mapping": map[string]any{
			"claims": cloneClaimMap(prov.Mapping.Claims),
		},
		"linking": map[string]any{
			"mode":     prov.Linking.Mode,
			"match_by": prov.Linking.MatchBy,
		},
		"session": cloneMap(prov.Session),
		"ui":      cloneMap(prov.UI),
		"enabled": prov.Enabled,
	}
	if prov.Target.SchemaID != "" || prov.Target.SchemaType != "" {
		data["target"] = map[string]any{
			"schema_id":   prov.Target.SchemaID,
			"schema_type": prov.Target.SchemaType,
		}
	}
	if prov.CatalogRef.TemplateID != "" || prov.CatalogRef.TemplateVersion != "" || prov.CatalogRef.Official || len(prov.CatalogRef.Capabilities) > 0 || prov.CatalogRef.LogoURL != "" || prov.CatalogRef.DocsURL != "" {
		catalogRef := map[string]any{
			"template_id":      prov.CatalogRef.TemplateID,
			"template_version": prov.CatalogRef.TemplateVersion,
		}
		if prov.CatalogRef.Official {
			catalogRef["official"] = true
		}
		if len(prov.CatalogRef.Capabilities) > 0 {
			catalogRef["capabilities"] = append([]string(nil), prov.CatalogRef.Capabilities...)
		}
		if prov.CatalogRef.LogoURL != "" {
			catalogRef["logo_url"] = prov.CatalogRef.LogoURL
		}
		if prov.CatalogRef.DocsURL != "" {
			catalogRef["docs_url"] = prov.CatalogRef.DocsURL
		}
		data["catalog_ref"] = catalogRef
	}
	return schema.ObjectMap(data)
}

func fromPersistenceRecord(rec persistenceRecord) Provider {
	prov := Provider{
		ID:        rec.ID,
		OrgID:     rec.OrgID,
		SchemaID:  rec.ResourceSchemaID,
		CreatedAt: rec.CreatedAt,
		UpdatedAt: rec.UpdatedAt,
	}

	if metadata := strings.TrimSpace(rec.MetadataJSON); metadata != "" && metadata != "{}" {
		_ = json.Unmarshal([]byte(metadata), &prov)
	}

	if prov.DisplayName == "" {
		prov.DisplayName = rec.Name
	}
	if prov.Protocol == "" {
		prov.Protocol = rec.Protocol
	}
	if prov.Kind == "" {
		prov.Kind = legacyKindFromTemplate(rec.Template)
	}
	if prov.Connection == nil {
		_ = json.Unmarshal([]byte(rec.ConfigJSON), &prov.Connection)
	}
	if prov.Mapping.Claims == nil {
		_ = json.Unmarshal([]byte(rec.OverridesJSON), &prov.Mapping.Claims)
	}
	if prov.Target.SchemaID == "" {
		prov.Target.SchemaID = rec.TargetSchemaID
	}
	if prov.Target.SchemaType == "" {
		prov.Target.SchemaType = rec.TargetSchemaType
	}
	if prov.Linking.Mode == "" && !rec.AutoRegister {
		prov.Linking.Mode = LinkModeLinkOnly
	}
	if prov.Enabled != rec.Enabled {
		prov.Enabled = rec.Enabled
	}
	if prov.UI == nil {
		prov.UI = map[string]any{}
	}
	if _, ok := prov.UI["display_order"]; !ok && rec.DisplayOrder != 0 {
		prov.UI["display_order"] = rec.DisplayOrder
	}
	if prov.CatalogRef.TemplateID == "" && rec.Template != "" {
		prov.CatalogRef.TemplateID = rec.Template
	}
	return Normalize(prov)
}

func toPersistenceRecord(id string, prov Provider) (persistenceRecord, error) {
	prov = Normalize(prov)
	configJSON, err := json.Marshal(prov.Connection)
	if err != nil {
		return persistenceRecord{}, fmt.Errorf("marshal provider connection: %w", err)
	}
	overridesJSON, err := json.Marshal(prov.Mapping.Claims)
	if err != nil {
		return persistenceRecord{}, fmt.Errorf("marshal provider mapping: %w", err)
	}
	metadataJSON, err := json.Marshal(prov)
	if err != nil {
		return persistenceRecord{}, fmt.Errorf("marshal provider metadata: %w", err)
	}
	return persistenceRecord{
		ID:               id,
		OrgID:            prov.OrgID,
		Name:             prov.DisplayName,
		Protocol:         prov.Protocol,
		Template:         LegacyTemplateID(prov),
		ConfigJSON:       string(configJSON),
		OverridesJSON:    string(overridesJSON),
		AutoRegister:     LegacyAutoRegister(prov),
		Enabled:          prov.Enabled,
		DisplayOrder:     DisplayOrder(prov),
		ResourceSchemaID: prov.SchemaID,
		TargetSchemaID:   prov.Target.SchemaID,
		TargetSchemaType: prov.Target.SchemaType,
		MetadataJSON:     string(metadataJSON),
	}, nil
}

func (r *Repository) placeholder(n int) string {
	if r.dialect == "postgres" {
		return fmt.Sprintf("$%d", n)
	}
	return "?"
}

func (r *Repository) timeExpr() string {
	if r.dialect == "postgres" {
		return "NOW()"
	}
	return "datetime('now')"
}

func ResolveTargetSchema(ctx context.Context, db rowQueryer, target Target, dialect ...string) (string, string, error) {
	if strings.TrimSpace(target.SchemaID) != "" {
		rec, err := schema.LoadSchemaRecord(ctx, db, target.SchemaID, dialect...)
		if err != nil {
			return "", "", fmt.Errorf("load provider target schema %q: %w", target.SchemaID, err)
		}
		return rec.ID, rec.Type, nil
	}

	schemaType := strings.TrimSpace(target.SchemaType)
	if schemaType == "" {
		schemaType = "human_user"
	}
	rec, err := schema.ResolveSchemaForType(ctx, db, schemaType, "", dialect...)
	if err != nil {
		return "", "", err
	}
	return rec.ID, rec.Type, nil
}

func defaultDisplayName(kind string) string {
	switch kind {
	case "entra", "entraid":
		return "Microsoft Entra ID"
	case "custom-oidc":
		return "Custom OIDC"
	case "":
		return "Provider"
	default:
		return strings.ReplaceAll(strings.Title(strings.ReplaceAll(kind, "-", " ")), "Oidc", "OIDC")
	}
}

func legacyKindFromTemplate(templateID string) string {
	switch strings.TrimSpace(templateID) {
	case "google-oidc":
		return "google"
	case "entra-id", "entraid":
		return "entra"
	case "custom-oidc":
		return "custom"
	default:
		return strings.TrimSpace(templateID)
	}
}

func cloneMap(input map[string]any) map[string]any {
	if input == nil {
		return nil
	}
	out := make(map[string]any, len(input))
	for key, value := range input {
		out[key] = value
	}
	return out
}

func cloneClaimMap(input map[string]string) map[string]any {
	if input == nil {
		return map[string]any{}
	}
	out := make(map[string]any, len(input))
	for key, value := range input {
		out[key] = value
	}
	return out
}
