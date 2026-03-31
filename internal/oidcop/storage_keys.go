package oidcop

import (
	"context"
	"crypto/rand"
	"crypto/rsa"
	"crypto/x509"
	"fmt"

	jose "github.com/go-jose/go-jose/v4"
	"github.com/google/uuid"
	"github.com/zitadel/oidc/v3/pkg/op"

	zcrypto "github.com/zitadel/zitadel/internal/crypto"
)

func (s *Storage) SigningKey(ctx context.Context) (op.SigningKey, error) {
	key, err := s.getOrCreateSigningKey(ctx)
	if err != nil {
		return nil, err
	}
	return key, nil
}

func (s *Storage) SignatureAlgorithms(_ context.Context) ([]jose.SignatureAlgorithm, error) {
	return []jose.SignatureAlgorithm{jose.RS256}, nil
}

func (s *Storage) KeySet(ctx context.Context) ([]op.Key, error) {
	sk, err := s.getOrCreateSigningKey(ctx)
	if err != nil {
		return nil, err
	}
	return []op.Key{&publicKey{sk}}, nil
}

type signingKeyData struct {
	id  string
	key *rsa.PrivateKey
}

func (sk *signingKeyData) SignatureAlgorithm() jose.SignatureAlgorithm { return jose.RS256 }
func (sk *signingKeyData) Key() any                                    { return sk.key }
func (sk *signingKeyData) ID() string                                  { return sk.id }

type publicKey struct {
	*signingKeyData
}

func (pk *publicKey) Algorithm() jose.SignatureAlgorithm { return jose.RS256 }
func (pk *publicKey) Use() string                        { return "sig" }
func (pk *publicKey) Key() any                           { return &pk.key.PublicKey }

func (s *Storage) getOrCreateSigningKey(ctx context.Context) (*signingKeyData, error) {
	if s.secrets == nil {
		return nil, fmt.Errorf("secret store is required")
	}

	id, keyBytes, err := s.secrets.GetByType(ctx, "oidc_signing")
	if err == nil {
		pk, err := x509.ParsePKCS1PrivateKey(keyBytes)
		if err != nil {
			return nil, fmt.Errorf("parse signing key: %w", err)
		}
		return &signingKeyData{id: id, key: pk}, nil
	}

	key, err := rsa.GenerateKey(rand.Reader, 2048)
	if err != nil {
		return nil, fmt.Errorf("generate signing key: %w", err)
	}
	id = uuid.NewString()
	keyDER := x509.MarshalPKCS1PrivateKey(key)

	if err := s.secrets.Put(ctx, id, "oidc_signing", keyDER,
		zcrypto.WithAlgorithm("RS256"),
		zcrypto.WithPublicKey(x509.MarshalPKCS1PublicKey(&key.PublicKey)),
	); err != nil {
		return nil, fmt.Errorf("store signing key: %w", err)
	}

	return &signingKeyData{id: id, key: key}, nil
}
