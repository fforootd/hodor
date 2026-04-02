package tenantaudit

import (
	"go/ast"
	"go/parser"
	"go/token"
	"path/filepath"
	"slices"
	"strconv"
	"strings"
	"testing"
)

var auditedDirs = []string{
	"internal/analytics",
	"internal/api",
	"internal/auth",
	"internal/crypto",
	"internal/events",
	"internal/login",
	"internal/loginflow",
	"internal/notify",
	"internal/oidcop",
	"internal/provider",
	"internal/risk",
	"internal/session",
	"internal/settings",
	"internal/ui",
	"internal/uniqueness",
}

var tenantTables = []string{
	"actions",
	"apps",
	"auth_states",
	"cache",
	"consumer_cursors",
	"credentials",
	"domains",
	"events",
	"fingerprints",
	"groups",
	"jobs",
	"linked_identities",
	"login_flow_assets",
	"login_flows",
	"memberships",
	"notification_requests",
	"orgs",
	"projects",
	"providers",
	"retention_policies",
	"saved_queries",
	"secrets",
	"sessions",
	"settings",
	"tokens",
	"unique_fields",
	"users",
}

func TestTenantScopedSQLIncludesInstanceID(t *testing.T) {
	root := repoRoot(t)
	fset := token.NewFileSet()
	var failures []string

	for _, dir := range auditedDirs {
		matches, err := filepath.Glob(filepath.Join(root, dir, "*.go"))
		if err != nil {
			t.Fatalf("glob %s: %v", dir, err)
		}
		for _, path := range matches {
			if strings.HasSuffix(path, "_test.go") {
				continue
			}
			file, err := parser.ParseFile(fset, path, nil, parser.AllErrors)
			if err != nil {
				t.Fatalf("parse %s: %v", path, err)
			}
			ast.Inspect(file, func(n ast.Node) bool {
				lit, ok := n.(*ast.BasicLit)
				if !ok || lit.Kind != token.STRING {
					return true
				}
				value, err := strconv.Unquote(lit.Value)
				if err != nil {
					return true
				}
				if !looksLikeSQL(value) {
					return true
				}
				table := matchedTenantTable(value)
				if table == "" {
					return true
				}
				if strings.Contains(strings.ToLower(value), "instance_id") {
					return true
				}
				pos := fset.Position(lit.Pos())
				failures = append(failures, pos.String()+" touches tenant table "+table+" without instance_id")
				return true
			})
		}
	}

	if len(failures) > 0 {
		slices.Sort(failures)
		t.Fatalf("tenant-scoped SQL must include instance_id:\n%s", strings.Join(failures, "\n"))
	}
}

func looksLikeSQL(value string) bool {
	sql := strings.ToLower(value)
	if !strings.Contains(sql, "select ") &&
		!strings.Contains(sql, "insert ") &&
		!strings.Contains(sql, "update ") &&
		!strings.Contains(sql, "delete ") {
		return false
	}
	return strings.Contains(sql, " from ") ||
		strings.Contains(sql, " into ") ||
		strings.Contains(sql, "update ")
}

func matchedTenantTable(value string) string {
	sql := strings.ToLower(value)
	for _, table := range tenantTables {
		if strings.Contains(sql, " "+table+" ") ||
			strings.Contains(sql, " "+table+"\n") ||
			strings.Contains(sql, " "+table+")") ||
			strings.Contains(sql, " "+table+",") {
			return table
		}
	}
	return ""
}

func repoRoot(t *testing.T) string {
	t.Helper()
	dir, err := filepath.Abs("../..")
	if err != nil {
		t.Fatalf("abs repo root: %v", err)
	}
	return dir
}
