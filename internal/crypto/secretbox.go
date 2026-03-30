package crypto

import (
	"crypto/aes"
	"crypto/cipher"
	"crypto/rand"
	"encoding/hex"
	"fmt"
	"io"
)

// SecretBox provides envelope encryption using AES-256-GCM.
// It holds a ring of named symmetric keys and knows which one is "active"
// (used for new encryptions). Old keys remain available for decryption,
// enabling seamless key rotation.
//
// When no keys are configured, SecretBox operates in plaintext passthrough
// mode — data flows through unencrypted. This is the default for dev mode.
type SecretBox struct {
	activeKeyID string
	keys        map[string][]byte // keyID → 32-byte AES key
}

// SealedSecret is the output of Seal: ciphertext + nonce + the key ID used.
type SealedSecret struct {
	Ciphertext []byte
	Nonce      []byte
	KeyID      string
}

// NewSecretBox creates a SecretBox from a key ring.
// activeKeyID selects which key encrypts new writes.
// Pass an empty ring for plaintext mode.
func NewSecretBox(activeKeyID string, keys map[string]string) (*SecretBox, error) {
	sb := &SecretBox{
		activeKeyID: activeKeyID,
		keys:        make(map[string][]byte, len(keys)),
	}

	for id, hexKey := range keys {
		k, err := hex.DecodeString(hexKey)
		if err != nil {
			return nil, fmt.Errorf("secretbox: key %q: invalid hex: %w", id, err)
		}
		if len(k) != 32 {
			return nil, fmt.Errorf("secretbox: key %q: must be 32 bytes (got %d)", id, len(k))
		}
		sb.keys[id] = k
	}

	// Validate active key exists (unless plaintext mode)
	if activeKeyID != "" {
		if _, ok := sb.keys[activeKeyID]; !ok {
			return nil, fmt.Errorf("secretbox: active_key_id %q not found in key ring", activeKeyID)
		}
	}

	return sb, nil
}

// Plaintext returns true if no keys are configured (dev/passthrough mode).
func (sb *SecretBox) Plaintext() bool {
	return len(sb.keys) == 0
}

// ActiveKeyID returns the ID of the active encryption key (empty if plaintext).
func (sb *SecretBox) ActiveKeyID() string {
	return sb.activeKeyID
}

// Seal encrypts plaintext with the active key using AES-256-GCM.
// In plaintext mode, returns the data as-is with empty nonce and keyID.
func (sb *SecretBox) Seal(plaintext []byte) (*SealedSecret, error) {
	if sb.Plaintext() {
		return &SealedSecret{
			Ciphertext: plaintext,
			Nonce:      nil,
			KeyID:      "",
		}, nil
	}

	key, ok := sb.keys[sb.activeKeyID]
	if !ok {
		return nil, fmt.Errorf("secretbox: active key %q not found", sb.activeKeyID)
	}

	block, err := aes.NewCipher(key)
	if err != nil {
		return nil, fmt.Errorf("secretbox: aes.NewCipher: %w", err)
	}

	gcm, err := cipher.NewGCM(block)
	if err != nil {
		return nil, fmt.Errorf("secretbox: cipher.NewGCM: %w", err)
	}

	nonce := make([]byte, gcm.NonceSize())
	if _, err := io.ReadFull(rand.Reader, nonce); err != nil {
		return nil, fmt.Errorf("secretbox: generate nonce: %w", err)
	}

	ciphertext := gcm.Seal(nil, nonce, plaintext, nil)

	return &SealedSecret{
		Ciphertext: ciphertext,
		Nonce:      nonce,
		KeyID:      sb.activeKeyID,
	}, nil
}

// Open decrypts ciphertext using the key identified by keyID.
// In plaintext mode (keyID empty), returns ciphertext as-is.
func (sb *SecretBox) Open(ciphertext, nonce []byte, keyID string) ([]byte, error) {
	if keyID == "" {
		// Plaintext mode — no decryption needed.
		return ciphertext, nil
	}

	key, ok := sb.keys[keyID]
	if !ok {
		return nil, fmt.Errorf("secretbox: key %q not found in ring (available: %d keys)", keyID, len(sb.keys))
	}

	block, err := aes.NewCipher(key)
	if err != nil {
		return nil, fmt.Errorf("secretbox: aes.NewCipher: %w", err)
	}

	gcm, err := cipher.NewGCM(block)
	if err != nil {
		return nil, fmt.Errorf("secretbox: cipher.NewGCM: %w", err)
	}

	plaintext, err := gcm.Open(nil, nonce, ciphertext, nil)
	if err != nil {
		return nil, fmt.Errorf("secretbox: decrypt failed (tampered or wrong key): %w", err)
	}

	return plaintext, nil
}
