package crypto

import (
	"context"
	"database/sql"
	"fmt"
	"time"
)

// SecretMeta holds non-sensitive metadata about a stored secret.
type SecretMeta struct {
	ID              string
	InstanceID      string
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
	db  *sql.DB
	box *SecretBox
}

// NewSecretStore creates a SecretStore backed by the given DB and SecretBox.
func NewSecretStore(db *sql.DB, box *SecretBox) *SecretStore {
	return &SecretStore{db: db, box: box}
}

// PutOption configures optional fields for Put.
type PutOption func(*putOpts)

type putOpts struct {
	instanceID string
	algorithm  string
	publicKey  []byte
	expiresAt  *time.Time
}

// WithInstanceID sets the instance_id for multi-tenant isolation.
func WithInstanceID(id string) PutOption {
	return func(o *putOpts) { o.instanceID = id }
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
		instanceID: "inst_root",
		algorithm:  "RS256",
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

	_, err = s.db.ExecContext(ctx,
		`INSERT OR REPLACE INTO secrets (id, instance_id, secret_type, algorithm, encryption_key_id, ciphertext, nonce, public_key, expires_at)
		 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)`,
		id, o.instanceID, secretType, o.algorithm,
		sealed.KeyID, sealed.Ciphertext, sealed.Nonce,
		o.publicKey, expiresAt,
	)
	if err != nil {
		return fmt.Errorf("secretstore: put %s: %w", id, err)
	}
	return nil
}

// Get retrieves and decrypts a secret by ID.
func (s *SecretStore) Get(ctx context.Context, id string) ([]byte, error) {
	var ciphertext, nonce []byte
	var keyID string

	err := s.db.QueryRowContext(ctx,
		`SELECT ciphertext, nonce, encryption_key_id FROM secrets WHERE id = ?`, id,
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

	err := s.db.QueryRowContext(ctx,
		`SELECT id, ciphertext, nonce, encryption_key_id
		 FROM secrets WHERE secret_type = ?
		 ORDER BY created_at DESC LIMIT 1`, secretType,
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
	_, err := s.db.ExecContext(ctx, `DELETE FROM secrets WHERE id = ?`, id)
	return err
}

// List returns metadata (no decryption) for all secrets of a given type.
func (s *SecretStore) List(ctx context.Context, secretType string) ([]SecretMeta, error) {
	rows, err := s.db.QueryContext(ctx,
		`SELECT id, instance_id, secret_type, algorithm, encryption_key_id, expires_at, created_at
		 FROM secrets WHERE secret_type = ?
		 ORDER BY created_at DESC`, secretType,
	)
	if err != nil {
		return nil, fmt.Errorf("secretstore: list %s: %w", secretType, err)
	}
	defer rows.Close()

	var metas []SecretMeta
	for rows.Next() {
		var m SecretMeta
		var expiresAt, createdAt sql.NullString
		if err := rows.Scan(&m.ID, &m.InstanceID, &m.SecretType, &m.Algorithm,
			&m.EncryptionKeyID, &expiresAt, &createdAt); err != nil {
			return nil, err
		}
		if expiresAt.Valid {
			if t, err := time.Parse(time.RFC3339, expiresAt.String); err == nil {
				m.ExpiresAt = &t
			}
		}
		if createdAt.Valid {
			m.CreatedAt, _ = time.Parse(time.RFC3339, createdAt.String)
		}
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
	rows, err := s.db.QueryContext(ctx,
		`SELECT id, ciphertext, nonce, encryption_key_id
		 FROM secrets
		 WHERE encryption_key_id != ? AND encryption_key_id != ''`,
		activeID,
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

		_, err = s.db.ExecContext(ctx,
			`UPDATE secrets SET ciphertext = ?, nonce = ?, encryption_key_id = ? WHERE id = ?`,
			sealed.Ciphertext, sealed.Nonce, sealed.KeyID, r.id,
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
