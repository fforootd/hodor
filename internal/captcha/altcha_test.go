package captcha

import (
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"fmt"
	"testing"
)

func sha256Hash(input string) string {
	h := sha256.Sum256([]byte(input))
	return hex.EncodeToString(h[:])
}

func itoa(n int) string {
	return fmt.Sprintf("%d", n)
}

func TestAltchaCreateChallenge(t *testing.T) {
	v := NewAltchaVerifier("test-hmac-key-1234567890abcdef", "SHA-256", 1000)

	challenge, err := v.CreateChallenge()
	if err != nil {
		t.Fatalf("CreateChallenge() error: %v", err)
	}

	if challenge.Algorithm != "SHA-256" {
		t.Errorf("Algorithm = %q, want SHA-256", challenge.Algorithm)
	}
	if challenge.Challenge == "" {
		t.Error("Challenge is empty")
	}
	if challenge.Salt == "" {
		t.Error("Salt is empty")
	}
	if challenge.Signature == "" {
		t.Error("Signature is empty")
	}
	if challenge.MaxNumber != 1000 {
		t.Errorf("MaxNumber = %d, want 1000", challenge.MaxNumber)
	}
}

func TestAltchaCreateChallenge_UniquePerCall(t *testing.T) {
	v := NewAltchaVerifier("test-key", "SHA-256", 100000)

	c1, _ := v.CreateChallenge()
	c2, _ := v.CreateChallenge()

	if c1.Salt == c2.Salt {
		t.Error("Two challenges have the same salt — should be unique")
	}
}

func TestAltchaVerifySolution_Valid(t *testing.T) {
	v := NewAltchaVerifier("test-key-verify", "SHA-256", 100)

	challenge, err := v.CreateChallenge()
	if err != nil {
		t.Fatalf("CreateChallenge() error: %v", err)
	}

	// Brute-force solve the challenge (same as what the client Web Worker does).
	var solvedNumber int
	found := false
	for i := 0; i <= challenge.MaxNumber; i++ {
		input := challenge.Salt + itoa(i)
		h := sha256Hash(input)
		if h == challenge.Challenge {
			solvedNumber = i
			found = true
			break
		}
	}
	if !found {
		t.Fatal("Could not solve challenge by brute force")
	}

	// Build solution payload.
	sol := Solution{
		Algorithm: "SHA-256",
		Challenge: challenge.Challenge,
		Number:    solvedNumber,
		Salt:      challenge.Salt,
		Signature: challenge.Signature,
		Took:      500, // 500ms simulated
	}
	payload, _ := json.Marshal(sol)

	valid, took, err := v.Verify(string(payload))
	if err != nil {
		t.Fatalf("Verify() error: %v", err)
	}
	if !valid {
		t.Error("Verify() = false, want true")
	}
	if took != 500 {
		t.Errorf("took = %d, want 500", took)
	}
}

func TestAltchaVerifySolution_Invalid(t *testing.T) {
	v := NewAltchaVerifier("test-key-invalid", "SHA-256", 100)

	challenge, _ := v.CreateChallenge()

	// Build solution with wrong number.
	sol := Solution{
		Algorithm: "SHA-256",
		Challenge: challenge.Challenge,
		Number:    999999, // wrong
		Salt:      challenge.Salt,
		Signature: challenge.Signature,
		Took:      100,
	}
	payload, _ := json.Marshal(sol)

	valid, _, err := v.Verify(string(payload))
	if err != nil {
		t.Fatalf("Verify() error: %v", err)
	}
	if valid {
		t.Error("Verify() = true for wrong number, want false")
	}
}

func TestAltchaVerifySolution_TamperedSignature(t *testing.T) {
	v := NewAltchaVerifier("test-key-tamper", "SHA-256", 100)

	challenge, _ := v.CreateChallenge()

	sol := Solution{
		Algorithm: "SHA-256",
		Challenge: challenge.Challenge,
		Number:    0,
		Salt:      challenge.Salt,
		Signature: "tampered-signature",
		Took:      100,
	}
	payload, _ := json.Marshal(sol)

	valid, _, err := v.Verify(string(payload))
	if err != nil {
		t.Fatalf("Verify() error: %v", err)
	}
	if valid {
		t.Error("Verify() = true for tampered signature, want false")
	}
}

func TestVerifyAltcha_ScoreBasedOnTiming(t *testing.T) {
	v := NewAltchaVerifier("test-key-score", "SHA-256", 100)

	challenge, _ := v.CreateChallenge()

	// Solve it.
	var num int
	for i := 0; i <= challenge.MaxNumber; i++ {
		if sha256Hash(challenge.Salt+itoa(i)) == challenge.Challenge {
			num = i
			break
		}
	}

	// Test with suspiciously fast solve time (< 50ms).
	sol := Solution{
		Algorithm: "SHA-256",
		Challenge: challenge.Challenge,
		Number:    num,
		Salt:      challenge.Salt,
		Signature: challenge.Signature,
		Took:      10, // suspiciously fast
	}
	payload, _ := json.Marshal(sol)
	result := VerifyAltcha(v, string(payload))

	if !result.Valid {
		t.Error("Expected valid but got invalid")
	}
	if result.Recommendation != "challenge" {
		t.Errorf("Expected recommendation=challenge for fast solve, got %q", result.Recommendation)
	}
	if result.Score >= 1.0 {
		t.Errorf("Expected reduced score for fast solve, got %f", result.Score)
	}

	// Test with normal solve time.
	sol.Took = 500
	payload, _ = json.Marshal(sol)
	result = VerifyAltcha(v, string(payload))

	if result.Recommendation != "allow" {
		t.Errorf("Expected recommendation=allow for normal solve, got %q", result.Recommendation)
	}
	if result.Score != 1.0 {
		t.Errorf("Expected score=1.0 for normal solve, got %f", result.Score)
	}
}

func TestVerifyAltcha_InvalidPayload(t *testing.T) {
	v := NewAltchaVerifier("test-key-bad", "SHA-256", 100)
	result := VerifyAltcha(v, "not-valid-json")

	if result.Valid {
		t.Error("Expected invalid for bad payload")
	}
	if result.Recommendation != "block" {
		t.Errorf("Expected recommendation=block, got %q", result.Recommendation)
	}
}

func TestGenerateHMACKey(t *testing.T) {
	key1, err := GenerateHMACKey()
	if err != nil {
		t.Fatalf("GenerateHMACKey() error: %v", err)
	}
	if len(key1) != 64 { // 32 bytes = 64 hex chars
		t.Errorf("key length = %d, want 64", len(key1))
	}

	key2, _ := GenerateHMACKey()
	if key1 == key2 {
		t.Error("Two generated keys are the same")
	}
}

func TestIsSaltExpired(t *testing.T) {
	// Valid (future expiry).
	fresh := ExpiringSalt("abc", 1*60*1000*1000*1000) // 1 minute in ns
	if IsSaltExpired(fresh) {
		t.Error("Fresh salt should not be expired")
	}

	// Expired.
	expired := "abc.0" // epoch 0 = long past
	if !IsSaltExpired(expired) {
		t.Error("Epoch-0 salt should be expired")
	}

	// No timestamp.
	if IsSaltExpired("no-timestamp") {
		t.Error("Salt without timestamp should not be treated as expired")
	}
}
