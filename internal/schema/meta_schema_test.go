package schema

import (
	"testing"
)

func TestValidateCatalog(t *testing.T) {
	errs := ValidateCatalog()
	for _, e := range errs {
		if e.Level == "error" {
			t.Errorf("%s", e)
		} else {
			t.Logf("%s", e)
		}
	}
	if t.Failed() {
		t.Fatalf("catalog validation found errors")
	}
	t.Logf("catalog validation passed (%d warnings)", len(errs))
}

func TestCatalog(t *testing.T) {
	catalog, err := Catalog()
	if err != nil {
		t.Fatalf("Catalog() error: %v", err)
	}
	if len(catalog) == 0 {
		t.Fatal("catalog is empty")
	}

	// Every entry with a $ref should resolve.
	for typeName, entry := range catalog {
		if entry.Ref == "" {
			continue
		}
		data, err := LoadSchemaFile(entry.Ref)
		if err != nil {
			t.Errorf("%s: $ref %q does not resolve: %v", typeName, entry.Ref, err)
		}
		if len(data) == 0 {
			t.Errorf("%s: schema file is empty", typeName)
		}
	}
}

func TestGroups(t *testing.T) {
	groups, err := Groups()
	if err != nil {
		t.Fatalf("Groups() error: %v", err)
	}
	if len(groups) == 0 {
		t.Fatal("groups is empty")
	}
	// Should have at least identities, applications, configure, observability, settings, system
	expected := []string{"identities", "applications", "configure", "observability", "settings", "system"}
	for _, g := range expected {
		if _, ok := groups[g]; !ok {
			t.Errorf("missing group %q", g)
		}
	}
}
