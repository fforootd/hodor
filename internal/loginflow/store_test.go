package loginflow

import (
	"context"
	"testing"
	"time"

	"github.com/zitadel/zitadel/internal/database"
	"github.com/zitadel/zitadel/internal/testutil/storagetest"
)

func TestStoreCRUDPromoteAndAssets(t *testing.T) {
	storagetest.RunBackends(t, func(t *testing.T, db *database.DB, ctx context.Context) {
		store := NewStore(db)
		now := time.Now().UTC().Format(time.RFC3339)

		record, err := store.Create(ctx, WriteParams{
			ID:              "flow-a",
			OrgID:           "1",
			SchemaID:        "login_flow_v1",
			Name:            "Flow A",
			Strategy:        "identifier_first",
			State:           "draft",
			Enabled:         true,
			Priority:        10,
			AudienceJSON:    `{"org_ids":["1"]}`,
			AuthMethodsJSON: `{"password":{"enabled":true}}`,
			ConfigJSON:      `{"branding":{"heading":"Hello"}}`,
			MetadataJSON:    `{}`,
			CreatedAt:       now,
			UpdatedAt:       now,
		})
		if err != nil {
			t.Fatalf("create flow: %v", err)
		}
		if record.Name != "Flow A" {
			t.Fatalf("name = %q, want Flow A", record.Name)
		}

		list, err := store.List(ctx, "")
		if err != nil {
			t.Fatalf("list flows: %v", err)
		}
		if len(list) != 1 {
			t.Fatalf("len(list) = %d, want 1", len(list))
		}

		updated, err := store.Update(ctx, WriteParams{
			ID:              "flow-a",
			OrgID:           "1",
			SchemaID:        "login_flow_v1",
			Name:            "Flow A Updated",
			Strategy:        "identifier_first",
			State:           "draft",
			Enabled:         true,
			Priority:        20,
			AudienceJSON:    `{"org_ids":["1"]}`,
			AuthMethodsJSON: `{"password":{"enabled":true}}`,
			ConfigJSON:      `{"branding":{"heading":"Updated"}}`,
			MetadataJSON:    `{}`,
			UpdatedAt:       time.Now().UTC().Format(time.RFC3339),
		})
		if err != nil {
			t.Fatalf("update flow: %v", err)
		}
		if updated.Name != "Flow A Updated" || updated.Priority != 20 {
			t.Fatalf("updated flow = %#v", updated)
		}

		prev, next, err := store.Promote(ctx, "flow-a")
		if err != nil {
			t.Fatalf("promote to testing: %v", err)
		}
		if prev != "draft" || next != "testing" {
			t.Fatalf("promote states = %q -> %q, want draft -> testing", prev, next)
		}
		_, next, err = store.Promote(ctx, "flow-a")
		if err != nil {
			t.Fatalf("promote to active: %v", err)
		}
		if next != "active" {
			t.Fatalf("next state = %q, want active", next)
		}

		asset, err := store.ReplaceAsset(ctx, "flow-a", "logo_url", "logo.png", "image/png", []byte("png-bytes"))
		if err != nil {
			t.Fatalf("replace asset: %v", err)
		}
		if asset.URL == "" {
			t.Fatal("expected asset URL")
		}

		loaded, err := store.Get(ctx, "flow-a")
		if err != nil {
			t.Fatalf("get flow after asset: %v", err)
		}
		config, ok := loaded.Config.(map[string]any)
		if !ok {
			t.Fatalf("config type = %T, want map[string]any", loaded.Config)
		}
		branding, _ := config["branding"].(map[string]any)
		if branding["logo_url"] != asset.URL {
			t.Fatalf("branding.logo_url = %v, want %s", branding["logo_url"], asset.URL)
		}

		assetData, err := store.GetAsset(ctx, asset.ID)
		if err != nil {
			t.Fatalf("get asset: %v", err)
		}
		if string(assetData.Payload) != "png-bytes" {
			t.Fatalf("asset payload = %q, want png-bytes", string(assetData.Payload))
		}

		slot, err := store.DeleteAsset(ctx, "flow-a", asset.ID)
		if err != nil {
			t.Fatalf("delete asset: %v", err)
		}
		if slot != "logo_url" {
			t.Fatalf("deleted slot = %q, want logo_url", slot)
		}

		loaded, err = store.Get(ctx, "flow-a")
		if err != nil {
			t.Fatalf("get flow after asset delete: %v", err)
		}
		config, _ = loaded.Config.(map[string]any)
		branding, _ = config["branding"].(map[string]any)
		if branding != nil {
			if _, ok := branding["logo_url"]; ok {
				t.Fatalf("branding.logo_url still present after delete: %#v", branding)
			}
		}

		if err := store.Archive(ctx, "flow-a"); err != nil {
			t.Fatalf("archive flow: %v", err)
		}

		if _, err := store.Get(storagetest.Context("instance_other"), "flow-a"); err == nil {
			t.Fatal("foreign tenant should not read the flow")
		}
	})
}
