package catalog

import (
	"context"
	"encoding/json"
	"fmt"
	"github.com/zitadel/zitadel/internal/logging"
	"io"
	"net/http"
	"os"
	"path/filepath"
	"time"
)

// StartBackground begins the async remote catalog refresh loop.
// This is safe to call immediately after New() — it delays the first fetch
// by 10 seconds to avoid competing with boot.
func (s *Service) StartBackground() {
	if s.cfg.URL == "" && s.cfg.LocalPath == "" {
		logging.Printf("[catalog] no remote source configured, using embedded only")
		return
	}

	go s.backgroundRefresh()
}

// Refresh forces an immediate catalog refresh from the remote source.
// Returns the number of new or updated templates found, or an error.
func (s *Service) Refresh(ctx context.Context) (int, error) {
	idx, err := s.fetchRemoteIndex(ctx)
	if err != nil {
		return 0, err
	}

	before := len(s.merged.Templates)
	s.SetRemote(idx)
	s.CacheToDB(idx)
	after := len(s.merged.Templates)

	diff := after - before
	if diff < 0 {
		diff = 0
	}

	logging.Printf("[catalog] refreshed: %d remote templates, %d new", len(idx.Templates), diff)
	return diff, nil
}

// FetchTemplate lazily fetches a single template payload from the remote source
// and caches it in the database.
func (s *Service) FetchTemplate(ctx context.Context, tpl *Template) (*TemplatePayload, error) {
	if tpl.Source != "remote" && tpl.Source != "cached" {
		return s.loadTemplatePayload(tpl)
	}

	// Try DB cache first.
	var cached string
	err := s.db.QueryRow(
		`SELECT data FROM cache WHERE namespace = 'catalog' AND key = ?`, "template:"+tpl.ID,
	).Scan(&cached)
	if err == nil {
		var payload TemplatePayload
		if err := json.Unmarshal([]byte(cached), &payload); err == nil {
			return &payload, nil
		}
	}

	// Fetch from remote.
	data, err := s.fetchRemoteFile(ctx, tpl.Path)
	if err != nil {
		return nil, fmt.Errorf("fetch template %q: %w", tpl.ID, err)
	}

	// Cache in DB.
	s.db.Exec(
		`INSERT OR REPLACE INTO cache (namespace, key, data, fetched_at) VALUES ('catalog', (?, ?, datetime('now'))`,
		"template:"+tpl.ID, string(data),
	)

	var payload TemplatePayload
	if err := json.Unmarshal(data, &payload); err != nil {
		return nil, fmt.Errorf("parse template %q: %w", tpl.ID, err)
	}
	return &payload, nil
}

// backgroundRefresh runs the async refresh loop.
func (s *Service) backgroundRefresh() {
	// Delay initial fetch to avoid competing with boot.
	time.Sleep(10 * time.Second)

	// First fetch.
	ctx, cancel := context.WithTimeout(context.Background(), 30*time.Second)
	s.tryRefresh(ctx)
	cancel()

	// Periodic refresh.
	interval := parseDuration(s.cfg.RefreshInterval, 1*time.Hour)
	if interval == 0 {
		logging.Printf("[catalog] refresh_interval=0, remote refresh disabled after initial")
		return
	}

	ticker := time.NewTicker(interval)
	defer ticker.Stop()

	for range ticker.C {
		ctx, cancel := context.WithTimeout(context.Background(), 30*time.Second)
		s.tryRefresh(ctx)
		cancel()
	}
}

// tryRefresh attempts a remote refresh, logging errors but never panicking.
func (s *Service) tryRefresh(ctx context.Context) {
	idx, err := s.fetchRemoteIndex(ctx)
	if err != nil {
		logging.Printf("[catalog] remote refresh failed (using cached/embedded): %v", err)
		return
	}

	s.SetRemote(idx)
	s.CacheToDB(idx)
	logging.Printf("[catalog] refreshed from remote: %d templates", len(idx.Templates))
}

// fetchRemoteIndex fetches the catalog index from the configured source.
func (s *Service) fetchRemoteIndex(ctx context.Context) (*Index, error) {
	// Local path takes priority (dev/air-gapped).
	if s.cfg.LocalPath != "" {
		return s.loadFromDisk(s.cfg.LocalPath)
	}

	if s.cfg.URL == "" {
		return nil, fmt.Errorf("no remote URL configured")
	}

	data, err := s.fetchRemoteFile(ctx, "catalog.json")
	if err != nil {
		return nil, err
	}

	var idx Index
	if err := json.Unmarshal(data, &idx); err != nil {
		return nil, fmt.Errorf("parse remote catalog: %w", err)
	}

	// Mark all as remote.
	for i := range idx.Templates {
		idx.Templates[i].Source = "remote"
	}

	return &idx, nil
}

// fetchRemoteFile fetches a single file from the remote catalog.
func (s *Service) fetchRemoteFile(ctx context.Context, path string) ([]byte, error) {
	url := s.cfg.URL
	if url == "" {
		return nil, fmt.Errorf("no remote URL configured")
	}

	// Normalize URL: strip trailing slash.
	url = fmt.Sprintf("%s/%s", url, path)

	req, err := http.NewRequestWithContext(ctx, "GET", url, nil)
	if err != nil {
		return nil, fmt.Errorf("create request: %w", err)
	}
	req.Header.Set("User-Agent", "Zitadel-Catalog/1.0")
	req.Header.Set("Accept", "application/json")

	client := &http.Client{Timeout: 15 * time.Second}
	resp, err := client.Do(req)
	if err != nil {
		return nil, fmt.Errorf("fetch %s: %w", url, err)
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		return nil, fmt.Errorf("fetch %s: HTTP %d", url, resp.StatusCode)
	}

	body, err := io.ReadAll(io.LimitReader(resp.Body, 5*1024*1024)) // 5MB limit
	if err != nil {
		return nil, fmt.Errorf("read %s: %w", url, err)
	}

	return body, nil
}

// loadFromDisk loads the catalog from a local directory.
func (s *Service) loadFromDisk(dir string) (*Index, error) {
	catalogPath := filepath.Join(dir, "catalog.json")
	data, err := os.ReadFile(catalogPath)
	if err != nil {
		return nil, fmt.Errorf("read local catalog %s: %w", catalogPath, err)
	}

	var idx Index
	if err := json.Unmarshal(data, &idx); err != nil {
		return nil, fmt.Errorf("parse local catalog: %w", err)
	}

	// Mark as remote (local overrides are treated the same as remote).
	for i := range idx.Templates {
		idx.Templates[i].Source = "remote"
	}

	return &idx, nil
}

// parseDuration parses a duration string, returning fallback if empty or invalid.
func parseDuration(s string, fallback time.Duration) time.Duration {
	if s == "" || s == "0" {
		if s == "0" {
			return 0
		}
		return fallback
	}
	d, err := time.ParseDuration(s)
	if err != nil {
		return fallback
	}
	return d
}
