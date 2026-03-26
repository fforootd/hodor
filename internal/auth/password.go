// Package auth provides authentication primitives for ZITADEL.
// Password hashing uses zitadel/passwap with argon2id as the default algorithm.
package auth

import (
	"context"
	"crypto/rand"
	"database/sql"
	"encoding/hex"
	"fmt"

	"github.com/zitadel/passwap"
	"github.com/zitadel/passwap/argon2"

	"github.com/zitadel/zitadel/internal/database"
	"github.com/zitadel/zitadel/internal/id"
)

// Passwords provides password hashing, verification, and credential storage.
type Passwords struct {
	swapper *passwap.Swapper
	db      *database.DB
}

// NewPasswords creates a new Passwords instance with argon2id as the default hasher.
func NewPasswords(db *database.DB) *Passwords {
	swapper := passwap.NewSwapper(
		argon2.NewArgon2id(argon2.RecommendedIDParams, nil),
		// Add legacy verifiers here when migrating from other algorithms.
	)
	return &Passwords{
		swapper: swapper,
		db:      db,
	}
}

// Hash hashes a plaintext password using the configured algorithm (argon2id).
func (p *Passwords) Hash(plain string) (string, error) {
	encoded, err := p.swapper.Hash(plain)
	if err != nil {
		return "", fmt.Errorf("hash password: %w", err)
	}
	return encoded, nil
}

// Verify checks a plaintext password against an encoded hash.
// Returns (true, updatedHash) if the password matches. If updatedHash is
// non-empty, the caller should persist it (algorithm upgrade occurred).
func (p *Passwords) Verify(encoded, plain string) (ok bool, updated string, err error) {
	updated, err = p.swapper.Verify(encoded, plain)
	if err != nil {
		return false, "", nil //nolint:nilerr // Intentional: passwap returns error for wrong password; we map to ok=false.
	}
	return true, updated, nil
}

// SetPassword stores a password credential for the given identity.
// If a password credential already exists, it is replaced.
func (p *Passwords) SetPassword(ctx context.Context, identityID int64, plain string) error {
	encoded, err := p.Hash(plain)
	if err != nil {
		return err
	}

	tx, err := p.db.SQL().BeginTx(ctx, nil)
	if err != nil {
		return fmt.Errorf("begin tx: %w", err)
	}
	defer tx.Rollback()

	// Delete existing password credential if any.
	_, err = tx.ExecContext(ctx,
		`DELETE FROM identity_credentials WHERE identity_id = ? AND credential_type = 'password'`,
		identityID,
	)
	if err != nil {
		return fmt.Errorf("delete old password: %w", err)
	}

	credID, err := id.New()
	if err != nil {
		return fmt.Errorf("generate credential id: %w", err)
	}

	// Store the encoded hash as credential_data JSON.
	credJSON := fmt.Sprintf(`{"hash":"%s"}`, encoded)
	_, err = tx.ExecContext(ctx,
		`INSERT INTO identity_credentials (id, identity_id, credential_type, credential_data)
		 VALUES (?, ?, 'password', ?)`,
		credID, identityID, credJSON,
	)
	if err != nil {
		return fmt.Errorf("insert password credential: %w", err)
	}

	return tx.Commit()
}

// CheckPassword verifies a password for the given identity.
// Returns true if the password is correct. Transparently re-hashes if the
// algorithm has been upgraded.
func (p *Passwords) CheckPassword(ctx context.Context, identityID int64, plain string) (bool, error) {
	// Load password credential.
	var credJSON string
	var credID int64
	err := p.db.SQL().QueryRowContext(ctx,
		`SELECT id, credential_data FROM identity_credentials
		 WHERE identity_id = ? AND credential_type = 'password'`,
		identityID,
	).Scan(&credID, &credJSON)
	if err == sql.ErrNoRows {
		return false, nil // No password credential.
	}
	if err != nil {
		return false, fmt.Errorf("load password: %w", err)
	}

	// Extract hash from JSON. Simple extraction — credential_data is {"hash":"..."}
	encoded := extractHash(credJSON)
	if encoded == "" {
		return false, fmt.Errorf("invalid password credential data")
	}

	ok, updated, err := p.Verify(encoded, plain)
	if err != nil {
		return false, err
	}
	if !ok {
		return false, nil
	}

	// If passwap returned an updated hash (algorithm upgrade), persist it.
	if updated != "" {
		updatedJSON := fmt.Sprintf(`{"hash":"%s"}`, updated)
		_, _ = p.db.SQL().ExecContext(ctx,
			`UPDATE identity_credentials SET credential_data = ? WHERE id = ?`,
			updatedJSON, credID,
		)
	}

	return true, nil
}

// extractHash extracts the hash value from credential_data JSON.
// Expected format: {"hash":"$argon2id$..."}
func extractHash(credJSON string) string {
	// Simple extraction without importing encoding/json for this hot path.
	const prefix = `{"hash":"`
	const suffix = `"}`
	if len(credJSON) < len(prefix)+len(suffix) {
		return ""
	}
	if credJSON[:len(prefix)] != prefix {
		return ""
	}
	if credJSON[len(credJSON)-len(suffix):] != suffix {
		return ""
	}
	return credJSON[len(prefix) : len(credJSON)-len(suffix)]
}

// GenerateRandomPassword generates a cryptographically random password
// of the given length (hex-encoded, so actual entropy is length*4 bits).
func GenerateRandomPassword(length int) (string, error) {
	b := make([]byte, length/2+1)
	if _, err := rand.Read(b); err != nil {
		return "", fmt.Errorf("generate random password: %w", err)
	}
	return hex.EncodeToString(b)[:length], nil
}
