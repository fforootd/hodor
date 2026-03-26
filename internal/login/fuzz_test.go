package login

import (
	"testing"
)

func FuzzParseIDTokenClaims(f *testing.F) {
	// Valid JWT-like inputs.
	f.Add("eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxMjMifQ.fakesig")
	f.Add("a.b.c")
	f.Add("")
	f.Add("single-part")
	f.Add("two.parts")

	f.Fuzz(func(t *testing.T, token string) {
		// Must never panic.
		_, _ = parseIDTokenClaims(token)
	})
}

func FuzzMapClaims(f *testing.F) {
	f.Add(`{"type":"object","properties":{"email":{"x-claim-mapping":"claims.email"}}}`)
	f.Add(`{}`)
	f.Add(`{"type":"object","properties":{}}`)
	f.Add(`not json`)

	f.Fuzz(func(t *testing.T, schema string) {
		claims := map[string]any{
			"email": "test@test.com",
			"name":  "Test",
			"sub":   "123",
		}
		// Must never panic.
		_, _ = MapClaims(schema, nil, claims)
	})
}
