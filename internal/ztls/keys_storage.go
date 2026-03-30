package ztls

import (
	"context"
	"database/sql"
	"fmt"
	"io/fs"
	"strings"
	"time"

	"github.com/caddyserver/certmagic"
)

// keysStorage implements certmagic.Storage using the existing `keys` table.
// CertMagic stores certs, keys, and metadata as key-value blobs.
// We map these to the keys table:
//   - id:         CertMagic key path (e.g., "certificates/acme-v02.../example.com/example.com.crt")
//   - type:       "certmagic"
//   - key_data:   raw cert/key/metadata bytes
//   - expires_at: used for cert expiry tracking
type keysStorage struct {
	db *sql.DB
}

// Verify interface compliance.
var _ certmagic.Storage = (*keysStorage)(nil)

func (s *keysStorage) Lock(ctx context.Context, key string) error {
	// For single-instance SQLite, no distributed lock is needed.
	// CertMagic only uses locks to prevent concurrent ACME operations.
	// SQLite's WAL mode handles serialization at the DB level.
	return nil
}

func (s *keysStorage) Unlock(ctx context.Context, key string) error {
	return nil
}

func (s *keysStorage) Store(ctx context.Context, key string, value []byte) error {
	now := time.Now().UTC().Format(time.RFC3339)

	// Upsert: try update first, then insert.
	result, err := s.db.ExecContext(ctx,
		`UPDATE keys SET key_data = ?, expires_at = ? WHERE id = ? AND type = 'certmagic'`,
		value, now, key)
	if err != nil {
		return fmt.Errorf("certmagic store update: %w", err)
	}
	rows, _ := result.RowsAffected()
	if rows > 0 {
		return nil
	}

	_, err = s.db.ExecContext(ctx,
		`INSERT INTO keys (id, type, algorithm, key_data, expires_at, created_at)
		 VALUES (?, 'certmagic', 'none', ?, ?, ?)`,
		key, value, now, now)
	if err != nil {
		// If it was a race and the row now exists, try update again.
		if strings.Contains(err.Error(), "UNIQUE") {
			_, err = s.db.ExecContext(ctx,
				`UPDATE keys SET key_data = ?, expires_at = ? WHERE id = ? AND type = 'certmagic'`,
				value, now, key)
		}
		if err != nil {
			return fmt.Errorf("certmagic store insert: %w", err)
		}
	}
	return nil
}

func (s *keysStorage) Load(ctx context.Context, key string) ([]byte, error) {
	var data []byte
	err := s.db.QueryRowContext(ctx,
		`SELECT key_data FROM keys WHERE id = ? AND type = 'certmagic'`, key,
	).Scan(&data)
	if err == sql.ErrNoRows {
		return nil, fs.ErrNotExist
	}
	if err != nil {
		return nil, fmt.Errorf("certmagic load: %w", err)
	}
	return data, nil
}

func (s *keysStorage) Delete(ctx context.Context, key string) error {
	_, err := s.db.ExecContext(ctx,
		`DELETE FROM keys WHERE id = ? AND type = 'certmagic'`, key)
	if err != nil {
		return fmt.Errorf("certmagic delete: %w", err)
	}
	return nil
}

func (s *keysStorage) Exists(ctx context.Context, key string) bool {
	var count int
	err := s.db.QueryRowContext(ctx,
		`SELECT COUNT(*) FROM keys WHERE id = ? AND type = 'certmagic'`, key,
	).Scan(&count)
	return err == nil && count > 0
}

func (s *keysStorage) List(ctx context.Context, prefix string, recursive bool) ([]string, error) {
	var query string
	if recursive {
		query = `SELECT id FROM keys WHERE type = 'certmagic' AND id LIKE ? ORDER BY id`
	} else {
		// Non-recursive: only return direct children (no further slashes after prefix).
		query = `SELECT id FROM keys WHERE type = 'certmagic' AND id LIKE ? ORDER BY id`
	}

	rows, err := s.db.QueryContext(ctx, query, prefix+"%")
	if err != nil {
		return nil, fmt.Errorf("certmagic list: %w", err)
	}
	defer rows.Close()

	var keys []string
	for rows.Next() {
		var k string
		if err := rows.Scan(&k); err != nil {
			continue
		}
		if !recursive {
			// Filter out nested paths: only include items that don't have
			// another slash after the prefix.
			rest := strings.TrimPrefix(k, prefix)
			if strings.Contains(rest, "/") {
				// Check if it's a directory-level entry.
				parts := strings.SplitN(rest, "/", 2)
				dirKey := prefix + parts[0] + "/"
				// Add directory if not already present.
				if len(keys) == 0 || keys[len(keys)-1] != dirKey {
					keys = append(keys, dirKey)
				}
				continue
			}
		}
		keys = append(keys, k)
	}
	if err := rows.Err(); err != nil {
		return nil, fmt.Errorf("certmagic list rows: %w", err)
	}

	return keys, nil
}

func (s *keysStorage) Stat(ctx context.Context, key string) (certmagic.KeyInfo, error) {
	var data []byte
	var modifiedStr string
	err := s.db.QueryRowContext(ctx,
		`SELECT key_data, COALESCE(expires_at, created_at) FROM keys WHERE id = ? AND type = 'certmagic'`, key,
	).Scan(&data, &modifiedStr)
	if err == sql.ErrNoRows {
		return certmagic.KeyInfo{}, fs.ErrNotExist
	}
	if err != nil {
		return certmagic.KeyInfo{}, fmt.Errorf("certmagic stat: %w", err)
	}

	modified, _ := time.Parse(time.RFC3339, modifiedStr)

	return certmagic.KeyInfo{
		Key:        key,
		Modified:   modified,
		Size:       int64(len(data)),
		IsTerminal: !strings.HasSuffix(key, "/"),
	}, nil
}
