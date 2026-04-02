package crypto

import (
	"context"
	"fmt"
	"time"

	"github.com/zitadel/zitadel/internal/database"
)

// SecretMeta holds non-sensitive metadata about a stored secret.
type SecretMeta struct {
	ID              string
	SecretType      string
	Algorithm       string
	EncryptionKeyID string
	ExpiresAt       *time.Time
	CreatedAt       time.Time
}

// SecretStore provides encrypted CRUD for the secrets table.
// All data passes through a SecretBox for envelope encryption before
// being written to the database, and is decrypted on read.
type SecretStore struct {
	db  *database.DB
	box *SecretBox
}

// NewSecretStore creates a SecretStore backed by the given DB and SecretBox.
func NewSecretStore(db *database.DB, box *SecretBox) *SecretStore {
	return &SecretStore{db: db, box: box}
}

// PutOption configures optional fields for Put.
type PutOption func(*putOpts)

type putOpts struct {
	algorithm string
	publicKey []byte
	expiresAt *time.Time
}

// WithAlgorithm sets the algorithm field (e.g., "RS256", "AES256").
func WithAlgorithm(alg string) PutOption {
	return func(o *putOpts) { o.algorithm = alg }
}

// WithPublicKey stores the public portion alongside the encrypted private key.
func WithPublicKey(pub []byte) PutOption {
	return func(o *putOpts) { o.publicKey = pub }
}

// WithExpiresAt sets an expiration time for the secret.
func WithExpiresAt(t time.Time) PutOption {
	return func(o *putOpts) { o.expiresAt = &t }
}

// Put encrypts and stores a secret. If a secret with the same ID already
// exists, it is replaced (upsert).
func (s *SecretStore) Put(ctx context.Context, id, secretType string, plaintext []byte, opts ...PutOption) error {
	o := &putOpts{
		algorithm: "RS256",
	}
	for _, fn := range opts {
		fn(o)
	}

	sealed, err := s.box.Seal(plaintext)
	if err != nil {
		return fmt.Errorf("secretstore: seal: %w", err)
	}

	var expiresAt *string
	if o.expiresAt != nil {
		t := o.expiresAt.Format(time.RFC3339)
		expiresAt = &t
	}

	scoped := s.db.Scoped(ctx)
	result, err := scoped.ExecContext(ctx, scoped.Rebind(
		`INSERT INTO secrets (instance_id, id, secret_type, algorithm, encryption_key_id, ciphertext, nonce, public_key, expires_at)
		 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
		 ON CONFLICT(id) DO UPDATE SET
			 instance_id = excluded.instance_id,
			 secret_type = excluded.secret_type,
			 algorithm = excluded.algorithm,
			 encryption_key_id = excluded.encryption_key_id,
			 ciphertext = excluded.ciphertext,
			 nonce = excluded.nonce,
			 public_key = excluded.public_key,
			 expires_at = excluded.expires_at
		 WHERE secrets.instance_id = excluded.instance_id`),
		scoped.InstanceID(), id, secretType, o.algorithm,
		sealed.KeyID, sealed.Ciphertext, sealed.Nonce,
		o.publicKey, expiresAt,
	)
	if err != nil {
		return fmt.Errorf("secretstore: put %s: %w", id, err)
	}
	if rows, _ := result.RowsAffected(); rows == 0 {
		return fmt.Errorf("secretstore: put %s: secret id already exists in another instance", id)
	}
	return nil
}

// Get retrieves and decrypts a secret by ID.
func (s *SecretStore) Get(ctx context.Context, id string) ([]byte, error) {
	var ciphertext, nonce []byte
	var keyID string

	scoped := s.db.Scoped(ctx)
	err := scoped.QueryRowContext(ctx,
		scoped.Rebind(`SELECT ciphertext, nonce, encryption_key_id FROM secrets WHERE instance_id = ? AND id = ?`),
		scoped.InstanceID(), id,
	).Scan(&ciphertext, &nonce, &keyID)
	if err != nil {
		return nil, fmt.Errorf("secretstore: get %s: %w", id, err)
	}

	return s.box.Open(ciphertext, nonce, keyID)
}

// GetByType retrieves the latest secret of a given type (by created_at DESC).
// Returns the secret ID and decrypted plaintext.
func (s *SecretStore) GetByType(ctx context.Context, secretType string) (string, []byte, error) {
	var id string
	var ciphertext, nonce []byte
	var keyID string

	scoped := s.db.Scoped(ctx)
	err := scoped.QueryRowContext(ctx,
		scoped.Rebind(`SELECT id, ciphertext, nonce, encryption_key_id
		 FROM secrets WHERE instance_id = ? AND secret_type = ?
		 ORDER BY created_at DESC LIMIT 1`),
		scoped.InstanceID(), secretType,
	).Scan(&id, &ciphertext, &nonce, &keyID)
	if err != nil {
		return "", nil, fmt.Errorf("secretstore: getByType %s: %w", secretType, err)
	}

	plaintext, err := s.box.Open(ciphertext, nonce, keyID)
	if err != nil {
		return "", nil, fmt.Errorf("secretstore: decrypt %s/%s: %w", secretType, id, err)
	}

	return id, plaintext, nil
}

// Delete removes a secret by ID.
func (s *SecretStore) Delete(ctx context.Context, id string) error {
	scoped := s.db.Scoped(ctx)
	_, err := scoped.ExecContext(ctx, scoped.Rebind(`DELETE FROM secrets WHERE instance_id = ? AND id = ?`), scoped.InstanceID(), id)
	return err
}

// List returns metadata (no decryption) for all secrets of a given type.
func (s *SecretStore) List(ctx context.Context, secretType string) ([]SecretMeta, error) {
	scoped := s.db.Scoped(ctx)
	rows, err := scoped.QueryContext(ctx,
		scoped.Rebind(`SELECT id, secret_type, algorithm, encryption_key_id, expires_at, created_at
		 FROM secrets WHERE instance_id = ? AND secret_type = ?
		 ORDER BY created_at DESC`),
		scoped.InstanceID(), secretType,
	)
	if err != nil {
		return nil, fmt.Errorf("secretstore: list %s: %w", secretType, err)
	}
	defer rows.Close()

	var metas []SecretMeta
	for rows.Next() {
		var m SecretMeta
		var expiresAt, createdAt string
		if err := rows.Scan(&m.ID, &m.SecretType, &m.Algorithm,
			&m.EncryptionKeyID, &expiresAt, &createdAt); err != nil {
			return nil, err
		}
		if expiresAt != "" {
			if t, ok := parseSecretTimestamp(expiresAt); ok {
				m.ExpiresAt = &t
			}
		}
		m.CreatedAt, _ = parseSecretTimestamp(createdAt)
		metas = append(metas, m)
	}
	return metas, rows.Err()
}

// ReEncryptAll re-encrypts all secrets currently using old key IDs
// to the active key. Returns the count of rotated rows.
// Safe to call multiple times (idempotent).
func (s *SecretStore) ReEncryptAll(ctx context.Context) (int, error) {
	if s.box.Plaintext() {
		return 0, nil // nothing to do
	}

	activeID := s.box.ActiveKeyID()
	scoped := s.db.Scoped(ctx)
	rows, err := scoped.QueryContext(ctx,
		scoped.Rebind(`SELECT id, ciphertext, nonce, encryption_key_id
		 FROM secrets
		 WHERE instance_id = ? AND encryption_key_id != ? AND encryption_key_id != ''`),
		scoped.InstanceID(), activeID,
	)
	if err != nil {
		return 0, fmt.Errorf("secretstore: re-encrypt query: %w", err)
	}
	defer rows.Close()

	type row struct {
		id         string
		ciphertext []byte
		nonce      []byte
		keyID      string
	}
	var toRotate []row

	for rows.Next() {
		var r row
		if err := rows.Scan(&r.id, &r.ciphertext, &r.nonce, &r.keyID); err != nil {
			return 0, err
		}
		toRotate = append(toRotate, r)
	}
	if err := rows.Err(); err != nil {
		return 0, err
	}

	rotated := 0
	for _, r := range toRotate {
		// Decrypt with old key
		plaintext, err := s.box.Open(r.ciphertext, r.nonce, r.keyID)
		if err != nil {
			return rotated, fmt.Errorf("secretstore: re-encrypt decrypt %s: %w", r.id, err)
		}

		// Re-encrypt with active key
		sealed, err := s.box.Seal(plaintext)
		if err != nil {
			return rotated, fmt.Errorf("secretstore: re-encrypt seal %s: %w", r.id, err)
		}

		_, err = scoped.ExecContext(ctx,
			scoped.Rebind(`UPDATE secrets SET ciphertext = ?, nonce = ?, encryption_key_id = ? WHERE instance_id = ? AND id = ?`),
			sealed.Ciphertext, sealed.Nonce, sealed.KeyID, scoped.InstanceID(), r.id,
		)
		if err != nil {
			return rotated, fmt.Errorf("secretstore: re-encrypt update %s: %w", r.id, err)
		}
		rotated++
	}

	return rotated, nil
}

// Box returns a reference to the underlying SecretBox for callers that
// need direct encrypt/decrypt without the store (e.g., CertMagic storage).
func (s *SecretStore) Box() *SecretBox {
	return s.box
}

func parseSecretTimestamp(value string) (time.Time, bool) {
	for _, layout := range []string{time.RFC3339Nano, time.RFC3339, "2006-01-02 15:04:05"} {
		if t, err := time.Parse(layout, value); err == nil {
			return t, true
		}
	}
	return time.Time{}, false
}
