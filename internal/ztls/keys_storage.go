package ztls

import (
	"context"
	"database/sql"
	"fmt"
	"io/fs"
	"strings"
	"time"

	"github.com/caddyserver/certmagic"

	zcrypto "github.com/zitadel/zitadel/internal/crypto"
	"github.com/zitadel/zitadel/internal/httputil"
)

// secretsStorage implements certmagic.Storage using the `secrets` table
// with envelope encryption via SecretBox.
//
// CertMagic stores certs, keys, and metadata as key-value blobs.
// We map these to the secrets table:
//   - id:             CertMagic key path (e.g., "certificates/acme-v02.../example.com/example.com.crt")
//   - secret_type:    "certmagic"
//   - ciphertext:     encrypted cert/key/metadata bytes
//   - nonce:          AES-GCM nonce
//   - encryption_key_id: which key was used
//   - expires_at:     used for cert expiry tracking
type secretsStorage struct {
	db  *sql.DB
	box *zcrypto.SecretBox
}

// Verify interface compliance.
var _ certmagic.Storage = (*secretsStorage)(nil)

func (s *secretsStorage) Lock(ctx context.Context, key string) error {
	// For single-instance SQLite, no distributed lock is needed.
	// CertMagic only uses locks to prevent concurrent ACME operations.
	// SQLite's WAL mode handles serialization at the DB level.
	return nil
}

func (s *secretsStorage) Unlock(ctx context.Context, key string) error {
	return nil
}

func (s *secretsStorage) Store(ctx context.Context, key string, value []byte) error {
	// Encrypt the value before storing.
	sealed, err := s.box.Seal(value)
	if err != nil {
		return fmt.Errorf("certmagic store encrypt: %w", err)
	}

	now := time.Now().UTC().Format(time.RFC3339)
	instanceID := httputil.InstanceIDFromContext(ctx)

	// Upsert: try update first, then insert.
	result, err := s.db.ExecContext(ctx,
		`UPDATE secrets SET ciphertext = ?, nonce = ?, encryption_key_id = ?, expires_at = ?
		 WHERE instance_id = ? AND id = ? AND secret_type = 'certmagic'`,
		sealed.Ciphertext, sealed.Nonce, sealed.KeyID, now, instanceID, key)
	if err != nil {
		return fmt.Errorf("certmagic store update: %w", err)
	}
	rows, _ := result.RowsAffected()
	if rows > 0 {
		return nil
	}

	_, err = s.db.ExecContext(ctx,
		`INSERT INTO secrets (instance_id, id, secret_type, algorithm, encryption_key_id, ciphertext, nonce, expires_at, created_at)
		 VALUES (?, ?, 'certmagic', 'none', ?, ?, ?, ?, ?)`,
		instanceID, key, sealed.KeyID, sealed.Ciphertext, sealed.Nonce, now, now)
	if err != nil {
		// If it was a race and the row now exists, try update again.
		if strings.Contains(err.Error(), "UNIQUE") {
			_, err = s.db.ExecContext(ctx,
				`UPDATE secrets SET ciphertext = ?, nonce = ?, encryption_key_id = ?, expires_at = ?
				 WHERE instance_id = ? AND id = ? AND secret_type = 'certmagic'`,
				sealed.Ciphertext, sealed.Nonce, sealed.KeyID, now, instanceID, key)
		}
		if err != nil {
			return fmt.Errorf("certmagic store insert: %w", err)
		}
	}
	return nil
}

func (s *secretsStorage) Load(ctx context.Context, key string) ([]byte, error) {
	var ciphertext, nonce []byte
	var keyID string
	instanceID := httputil.InstanceIDFromContext(ctx)
	err := s.db.QueryRowContext(ctx,
		`SELECT ciphertext, nonce, encryption_key_id FROM secrets WHERE instance_id = ? AND id = ? AND secret_type = 'certmagic'`, instanceID, key,
	).Scan(&ciphertext, &nonce, &keyID)
	if err == sql.ErrNoRows {
		return nil, fs.ErrNotExist
	}
	if err != nil {
		return nil, fmt.Errorf("certmagic load: %w", err)
	}

	plaintext, err := s.box.Open(ciphertext, nonce, keyID)
	if err != nil {
		return nil, fmt.Errorf("certmagic load decrypt: %w", err)
	}
	return plaintext, nil
}

func (s *secretsStorage) Delete(ctx context.Context, key string) error {
	instanceID := httputil.InstanceIDFromContext(ctx)
	_, err := s.db.ExecContext(ctx,
		`DELETE FROM secrets WHERE instance_id = ? AND id = ? AND secret_type = 'certmagic'`, instanceID, key)
	if err != nil {
		return fmt.Errorf("certmagic delete: %w", err)
	}
	return nil
}

func (s *secretsStorage) Exists(ctx context.Context, key string) bool {
	instanceID := httputil.InstanceIDFromContext(ctx)
	var count int
	err := s.db.QueryRowContext(ctx,
		`SELECT COUNT(*) FROM secrets WHERE instance_id = ? AND id = ? AND secret_type = 'certmagic'`, instanceID, key,
	).Scan(&count)
	return err == nil && count > 0
}

func (s *secretsStorage) List(ctx context.Context, prefix string, recursive bool) ([]string, error) {
	instanceID := httputil.InstanceIDFromContext(ctx)
	query := `SELECT id FROM secrets WHERE instance_id = ? AND secret_type = 'certmagic' AND id LIKE ? ORDER BY id`

	rows, err := s.db.QueryContext(ctx, query, instanceID, prefix+"%")
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

func (s *secretsStorage) Stat(ctx context.Context, key string) (certmagic.KeyInfo, error) {
	instanceID := httputil.InstanceIDFromContext(ctx)
	var ciphertext []byte
	var modifiedStr string
	err := s.db.QueryRowContext(ctx,
		`SELECT ciphertext, COALESCE(expires_at, created_at) FROM secrets WHERE instance_id = ? AND id = ? AND secret_type = 'certmagic'`, instanceID, key,
	).Scan(&ciphertext, &modifiedStr)
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
		Size:       int64(len(ciphertext)),
		IsTerminal: !strings.HasSuffix(key, "/"),
	}, nil
}
