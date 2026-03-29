// Package captcha provides server-side captcha challenge generation and
// verification. It supports Altcha (self-hosted PoW) as the primary provider,
// with a pluggable interface for third-party providers (hCaptcha, reCAPTCHA, Turnstile).
package captcha

import (
	"crypto/hmac"
	"crypto/rand"
	"crypto/sha256"
	"encoding/base64"
	"encoding/hex"
	"encoding/json"
	"fmt"
	"math/big"
	"strconv"
	"strings"
	"time"
)

// Challenge is sent to the client for PoW solving.
type Challenge struct {
	Algorithm string `json:"algorithm"` // "SHA-256"
	Challenge string `json:"challenge"` // hex-encoded HMAC(salt || number)
	Salt      string `json:"salt"`      // random salt
	MaxNumber int    `json:"maxnumber"` // upper bound for brute-force search
	Signature string `json:"signature"` // HMAC of challenge for server-side verification
}

// Solution is received from the client after solving the PoW.
type Solution struct {
	Algorithm string `json:"algorithm"`
	Challenge string `json:"challenge"`
	Number    int    `json:"number"`
	Salt      string `json:"salt"`
	Signature string `json:"signature"`
	Took      int    `json:"took"` // milliseconds the client took to solve
}

// AltchaVerifier handles Altcha PoW challenge creation and verification.
type AltchaVerifier struct {
	hmacKey   string
	algorithm string
	maxNumber int
}

// NewAltchaVerifier creates a new Altcha verifier.
// hmacKey is used to sign challenges. algorithm defaults to "SHA-256".
// maxNumber controls difficulty (default: 100000).
func NewAltchaVerifier(hmacKey, algorithm string, maxNumber int) *AltchaVerifier {
	if algorithm == "" {
		algorithm = "SHA-256"
	}
	if maxNumber == 0 {
		maxNumber = 100000
	}
	return &AltchaVerifier{
		hmacKey:   hmacKey,
		algorithm: algorithm,
		maxNumber: maxNumber,
	}
}

// CreateChallenge generates a new PoW challenge for the client.
func (v *AltchaVerifier) CreateChallenge() (*Challenge, error) {
	// Generate random salt.
	saltBytes := make([]byte, 12)
	if _, err := rand.Read(saltBytes); err != nil {
		return nil, fmt.Errorf("generate salt: %w", err)
	}
	salt := hex.EncodeToString(saltBytes)

	// Pick a random target number within [0, maxNumber).
	maxBig := big.NewInt(int64(v.maxNumber))
	secretNum, err := rand.Int(rand.Reader, maxBig)
	if err != nil {
		return nil, fmt.Errorf("generate number: %w", err)
	}
	number := int(secretNum.Int64())

	// Compute challenge = SHA-256(salt + number).
	challengeInput := salt + strconv.Itoa(number)
	h := sha256.Sum256([]byte(challengeInput))
	challenge := hex.EncodeToString(h[:])

	// Sign the challenge so we can verify it later without storing state.
	signature := v.sign(challenge)

	return &Challenge{
		Algorithm: v.algorithm,
		Challenge: challenge,
		Salt:      salt,
		MaxNumber: v.maxNumber,
		Signature: signature,
	}, nil
}

// Verify validates a client's PoW solution.
func (v *AltchaVerifier) Verify(payload string) (bool, int, error) {
	// Payload can be base64-encoded JSON or direct JSON.
	var sol Solution
	decoded, err := base64.StdEncoding.DecodeString(payload)
	if err != nil {
		// Try direct JSON.
		decoded = []byte(payload)
	}
	if err := json.Unmarshal(decoded, &sol); err != nil {
		return false, 0, fmt.Errorf("unmarshal solution: %w", err)
	}

	// Verify signature.
	expectedSig := v.sign(sol.Challenge)
	if !hmac.Equal([]byte(expectedSig), []byte(sol.Signature)) {
		return false, sol.Took, nil
	}

	// Verify the PoW: SHA-256(salt + number) must equal challenge.
	challengeInput := sol.Salt + strconv.Itoa(sol.Number)
	h := sha256.Sum256([]byte(challengeInput))
	computed := hex.EncodeToString(h[:])
	if computed != sol.Challenge {
		return false, sol.Took, nil
	}

	return true, sol.Took, nil
}

// sign creates an HMAC-SHA256 signature for the given data.
func (v *AltchaVerifier) sign(data string) string {
	mac := hmac.New(sha256.New, []byte(v.hmacKey))
	mac.Write([]byte(data))
	return hex.EncodeToString(mac.Sum(nil))
}

// VerifyResult contains the outcome of a captcha verification.
type VerifyResult struct {
	Valid          bool    `json:"valid"`
	Provider       string  `json:"provider"`
	PoWCompleted   bool    `json:"pow_completed"`
	PoWDurationMs  float64 `json:"pow_duration_ms"`
	Score          float64 `json:"score"`
	Recommendation string  `json:"recommendation"` // "allow" | "challenge" | "block"
}

// VerifyAltcha is a convenience function that verifies an Altcha payload and returns a structured result.
func VerifyAltcha(verifier *AltchaVerifier, payload string) *VerifyResult {
	valid, tookMs, err := verifier.Verify(payload)
	if err != nil {
		return &VerifyResult{
			Valid:          false,
			Provider:       "altcha",
			Recommendation: "block",
		}
	}

	result := &VerifyResult{
		Valid:         valid,
		Provider:      "altcha",
		PoWCompleted:  valid,
		PoWDurationMs: float64(tookMs),
	}

	if valid {
		result.Score = 1.0
		result.Recommendation = "allow"
		// Suspicious if PoW was solved too quickly (< 50ms = likely pre-computed or GPU).
		if tookMs > 0 && tookMs < 50 {
			result.Score = 0.3
			result.Recommendation = "challenge"
		}
	} else {
		result.Score = 0.0
		result.Recommendation = "block"
	}

	return result
}

// GenerateHMACKey creates a random HMAC key for use with AltchaVerifier.
func GenerateHMACKey() (string, error) {
	key := make([]byte, 32)
	if _, err := rand.Read(key); err != nil {
		return "", err
	}
	return hex.EncodeToString(key), nil
}

// ExpiringSalt adds a timestamp to salt for replay protection.
func ExpiringSalt(salt string, validFor time.Duration) string {
	expires := time.Now().Add(validFor).Unix()
	return fmt.Sprintf("%s.%d", salt, expires)
}

// IsSaltExpired checks if a timestamped salt has expired.
func IsSaltExpired(salt string) bool {
	parts := strings.SplitN(salt, ".", 2)
	if len(parts) != 2 {
		return false // no expiry embedded
	}
	ts, err := strconv.ParseInt(parts[1], 10, 64)
	if err != nil {
		return true // can't parse = treat as expired
	}
	return time.Now().Unix() > ts
}
