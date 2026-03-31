package oidcop

import (
	"context"

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
