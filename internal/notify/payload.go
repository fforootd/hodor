package notify

import (
	"encoding/json"
	"fmt"

	zcrypto "github.com/zitadel/zitadel/internal/crypto"
)

func (s *Service) sealPayload(payload map[string]any) (*zcrypto.SealedSecret, error) {
	raw, err := json.Marshal(payload)
	if err != nil {
		return nil, fmt.Errorf("notify: marshal payload: %w", err)
	}
	return s.box.Seal(raw)
}

func (s *Service) openPayload(ciphertext, nonce []byte, keyID string) (map[string]any, error) {
	raw, err := s.box.Open(ciphertext, nonce, keyID)
	if err != nil {
		return nil, fmt.Errorf("notify: decrypt payload: %w", err)
	}
	var payload map[string]any
	if len(raw) == 0 {
		return map[string]any{}, nil
	}
	if err := json.Unmarshal(raw, &payload); err != nil {
		return nil, fmt.Errorf("notify: decode payload: %w", err)
	}
	return payload, nil
}
