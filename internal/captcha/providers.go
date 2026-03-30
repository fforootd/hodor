package captcha

import (
	"context"
	"encoding/json"
	"fmt"
	"net/http"
	"net/url"
	"strings"
	"time"
)

var providerVerifyURLs = map[string]string{
	"hcaptcha":  "https://hcaptcha.com/siteverify",
	"recaptcha": "https://www.google.com/recaptcha/api/siteverify",
	"turnstile": "https://challenges.cloudflare.com/turnstile/v0/siteverify",
}

type remoteVerifyResponse struct {
	Success    bool     `json:"success"`
	Score      float64  `json:"score"`
	ErrorCodes []string `json:"error-codes"`
}

// VerifyProviderToken verifies a third-party captcha response token with the
// configured upstream verification endpoint.
func VerifyProviderToken(ctx context.Context, client *http.Client, provider, secretKey, token, remoteIP string) (*VerifyResult, error) {
	endpoint, ok := providerVerifyURLs[provider]
	if !ok {
		return nil, fmt.Errorf("unsupported captcha provider %q", provider)
	}
	if strings.TrimSpace(secretKey) == "" {
		return nil, fmt.Errorf("captcha secret_key is required")
	}
	if strings.TrimSpace(token) == "" {
		return nil, fmt.Errorf("captcha token is required")
	}

	if client == nil {
		client = &http.Client{Timeout: 10 * time.Second}
	}

	form := url.Values{}
	form.Set("secret", secretKey)
	form.Set("response", token)
	if strings.TrimSpace(remoteIP) != "" {
		form.Set("remoteip", remoteIP)
	}

	req, err := http.NewRequestWithContext(ctx, http.MethodPost, endpoint, strings.NewReader(form.Encode()))
	if err != nil {
		return nil, fmt.Errorf("build verify request: %w", err)
	}
	req.Header.Set("Content-Type", "application/x-www-form-urlencoded")

	resp, err := client.Do(req)
	if err != nil {
		return nil, fmt.Errorf("verify captcha: %w", err)
	}
	defer resp.Body.Close()

	if resp.StatusCode < 200 || resp.StatusCode >= 300 {
		return nil, fmt.Errorf("verify captcha returned %s", resp.Status)
	}

	var result remoteVerifyResponse
	if err := json.NewDecoder(resp.Body).Decode(&result); err != nil {
		return nil, fmt.Errorf("decode captcha verify response: %w", err)
	}

	verify := &VerifyResult{
		Valid:          result.Success,
		Provider:       provider,
		Recommendation: "block",
		Score:          0,
	}
	if !result.Success {
		return verify, nil
	}

	verify.Score = result.Score
	if verify.Score <= 0 {
		verify.Score = 1
	}
	verify.Recommendation = "allow"
	if result.Score > 0 && result.Score < 0.5 {
		verify.Recommendation = "challenge"
	}
	return verify, nil
}
