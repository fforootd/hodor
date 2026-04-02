package captcha

import (
	"context"
	"io"
	"net/http"
	"net/url"
	"testing"

	"github.com/zitadel/zitadel/internal/testutil/httptestutil"
)

func TestVerifyProviderToken_Success(t *testing.T) {
	original := providerVerifyURLs["turnstile"]
	defer func() {
		providerVerifyURLs["turnstile"] = original
	}()

	ts := httptestutil.NewServer(t, http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if r.Method != http.MethodPost {
			t.Fatalf("method = %s, want POST", r.Method)
		}
		body, err := io.ReadAll(r.Body)
		if err != nil {
			t.Fatalf("read body: %v", err)
		}
		values, err := url.ParseQuery(string(body))
		if err != nil {
			t.Fatalf("parse body: %v", err)
		}
		if values.Get("secret") != "secret-123" {
			t.Fatalf("secret = %q, want secret-123", values.Get("secret"))
		}
		if values.Get("response") != "token-abc" {
			t.Fatalf("response = %q, want token-abc", values.Get("response"))
		}
		if values.Get("remoteip") != "127.0.0.1" {
			t.Fatalf("remoteip = %q, want 127.0.0.1", values.Get("remoteip"))
		}
		w.Header().Set("Content-Type", "application/json")
		_, _ = w.Write([]byte(`{"success":true}`))
	}))
	defer ts.Close()

	providerVerifyURLs["turnstile"] = ts.URL

	result, err := VerifyProviderToken(context.Background(), ts.Client(), "turnstile", "secret-123", "token-abc", "127.0.0.1")
	if err != nil {
		t.Fatalf("VerifyProviderToken() error: %v", err)
	}
	if !result.Valid {
		t.Fatalf("Valid = false, want true")
	}
	if result.Provider != "turnstile" {
		t.Fatalf("Provider = %q, want turnstile", result.Provider)
	}
	if result.Recommendation != "allow" {
		t.Fatalf("Recommendation = %q, want allow", result.Recommendation)
	}
}

func TestVerifyProviderToken_InvalidToken(t *testing.T) {
	original := providerVerifyURLs["recaptcha"]
	defer func() {
		providerVerifyURLs["recaptcha"] = original
	}()

	ts := httptestutil.NewServer(t, http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.Header().Set("Content-Type", "application/json")
		_, _ = w.Write([]byte(`{"success":false,"error-codes":["invalid-input-response"]}`))
	}))
	defer ts.Close()

	providerVerifyURLs["recaptcha"] = ts.URL

	result, err := VerifyProviderToken(context.Background(), ts.Client(), "recaptcha", "secret-123", "token-abc", "")
	if err != nil {
		t.Fatalf("VerifyProviderToken() error: %v", err)
	}
	if result.Valid {
		t.Fatal("Valid = true, want false")
	}
	if result.Recommendation != "block" {
		t.Fatalf("Recommendation = %q, want block", result.Recommendation)
	}
}

func TestVerifyProviderToken_MissingSecretFailsClosed(t *testing.T) {
	if _, err := VerifyProviderToken(context.Background(), nil, "hcaptcha", "", "token-abc", ""); err == nil {
		t.Fatal("expected error for missing secret key")
	}
}
