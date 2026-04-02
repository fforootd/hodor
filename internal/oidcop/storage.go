package oidcop

import (
	"context"
	"time"

	"github.com/zitadel/oidc/v3/pkg/op"

	zcrypto "github.com/zitadel/zitadel/internal/crypto"
	"github.com/zitadel/zitadel/internal/database"
)

var (
	_ op.Storage                  = &Storage{}
	_ op.ClientCredentialsStorage = &Storage{}
)

// Storage implements op.Storage backed by the Zitadel database.
type Storage struct {
	db      *database.DB
	secrets *zcrypto.SecretStore
}

// NewStorage creates a new OIDC Storage.
func NewStorage(db *database.DB, secrets *zcrypto.SecretStore) *Storage {
	return &Storage{db: db, secrets: secrets}
}

func (s *Storage) Health(_ context.Context) error {
	return s.db.SQL().Ping()
}

func (s *Storage) scoped(ctx context.Context) *database.ScopedDB {
	return s.db.Scoped(ctx)
}

func parseStoredTimestamp(value string) (time.Time, bool) {
	for _, layout := range []string{time.RFC3339Nano, time.RFC3339, "2006-01-02 15:04:05"} {
		if t, err := time.Parse(layout, value); err == nil {
			return t, true
		}
	}
	return time.Time{}, false
}
