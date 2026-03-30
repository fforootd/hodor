package api_test

import (
	"bytes"
	"encoding/base64"
	"encoding/json"
	"io"
	"mime/multipart"
	"net/http"
	"net/http/httptest"
	"testing"

	"github.com/zitadel/zitadel/internal/testutil"
)

const tinyPNGBase64 = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+aXx8AAAAASUVORK5CYII="

func TestLoginFlowJSONResponsesAreNoStore(t *testing.T) {
	srv := testutil.NewTestServer(t)
	token := srv.LoginAdmin()

	flowID := createLoginFlow(t, srv, token, map[string]any{
		"name":     "Cache Test",
		"strategy": "identifier_first",
		"state":    "draft",
	})

	req, err := http.NewRequest(http.MethodGet, srv.URL()+"/v1/login-flows/"+flowID, nil)
	if err != nil {
		t.Fatalf("new request: %v", err)
	}
	req.Header.Set("Authorization", "Bearer "+token)

	resp, err := http.DefaultClient.Do(req)
	if err != nil {
		t.Fatalf("GET login flow: %v", err)
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		body, _ := io.ReadAll(resp.Body)
		t.Fatalf("expected 200, got %d: %s", resp.StatusCode, string(body))
	}
	if got := resp.Header.Get("Cache-Control"); got != "no-store" {
		t.Fatalf("Cache-Control = %q, want no-store", got)
	}
}

func TestLoginFlowAssetUploadServeAndDelete(t *testing.T) {
	srv := testutil.NewTestServer(t)
	token := srv.LoginAdmin()
	flowID := createLoginFlow(t, srv, token, map[string]any{
		"name":     "Asset Upload",
		"strategy": "identifier_first",
		"state":    "draft",
	})

	payload := decodeTinyPNG(t)
	uploadResp := uploadLoginFlowAsset(t, srv, token, flowID, "logo_url", "logo.png", payload)

	if uploadResp["slot"] != "logo_url" {
		t.Fatalf("slot = %v, want logo_url", uploadResp["slot"])
	}
	assetID, _ := uploadResp["id"].(string)
	assetURL, _ := uploadResp["url"].(string)
	if assetID == "" || assetURL == "" {
		t.Fatalf("unexpected upload response: %#v", uploadResp)
	}

	code, flowBody := srv.GetWithBearer("/v1/login-flows/"+flowID, token)
	if code != http.StatusOK {
		t.Fatalf("expected 200 getting login flow, got %d: %#v", code, flowBody)
	}
	config, _ := flowBody["config"].(map[string]any)
	branding, _ := config["branding"].(map[string]any)
	if branding["logo_url"] != assetURL {
		t.Fatalf("branding.logo_url = %v, want %s", branding["logo_url"], assetURL)
	}

	req, err := http.NewRequest(http.MethodGet, srv.URL()+assetURL, nil)
	if err != nil {
		t.Fatalf("new asset request: %v", err)
	}
	resp, err := http.DefaultClient.Do(req)
	if err != nil {
		t.Fatalf("GET asset: %v", err)
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		body, _ := io.ReadAll(resp.Body)
		t.Fatalf("expected 200 serving asset, got %d: %s", resp.StatusCode, string(body))
	}
	if got := resp.Header.Get("Cache-Control"); got != "public, max-age=31536000, immutable" {
		t.Fatalf("Cache-Control = %q, want immutable asset cache", got)
	}
	if got := resp.Header.Get("Content-Type"); got != "image/png" {
		t.Fatalf("Content-Type = %q, want image/png", got)
	}
	etag := resp.Header.Get("ETag")
	if etag == "" {
		t.Fatal("expected ETag header")
	}
	served, err := io.ReadAll(resp.Body)
	if err != nil {
		t.Fatalf("read asset body: %v", err)
	}
	if !bytes.Equal(served, payload) {
		t.Fatal("served asset bytes do not match uploaded payload")
	}

	req304, err := http.NewRequest(http.MethodGet, srv.URL()+assetURL, nil)
	if err != nil {
		t.Fatalf("new conditional request: %v", err)
	}
	req304.Header.Set("If-None-Match", etag)
	resp304, err := http.DefaultClient.Do(req304)
	if err != nil {
		t.Fatalf("GET conditional asset: %v", err)
	}
	defer resp304.Body.Close()
	if resp304.StatusCode != http.StatusNotModified {
		t.Fatalf("expected 304, got %d", resp304.StatusCode)
	}

	reqDelete, err := http.NewRequest(http.MethodDelete, srv.URL()+"/v1/login-flows/"+flowID+"/assets/"+assetID, nil)
	if err != nil {
		t.Fatalf("new delete request: %v", err)
	}
	reqDelete.Header.Set("Authorization", "Bearer "+token)
	deleteResp, err := http.DefaultClient.Do(reqDelete)
	if err != nil {
		t.Fatalf("DELETE asset: %v", err)
	}
	defer deleteResp.Body.Close()
	if deleteResp.StatusCode != http.StatusOK {
		body, _ := io.ReadAll(deleteResp.Body)
		t.Fatalf("expected 200 deleting asset, got %d: %s", deleteResp.StatusCode, string(body))
	}

	code, flowBody = srv.GetWithBearer("/v1/login-flows/"+flowID, token)
	if code != http.StatusOK {
		t.Fatalf("expected 200 getting login flow after delete, got %d: %#v", code, flowBody)
	}
	config, _ = flowBody["config"].(map[string]any)
	branding, _ = config["branding"].(map[string]any)
	if branding != nil {
		if _, ok := branding["logo_url"]; ok {
			t.Fatalf("branding.logo_url still present after delete: %#v", branding)
		}
	}
}

func TestLoginFlowAssetImportPersistsSameOriginURL(t *testing.T) {
	srv := testutil.NewTestServer(t)
	token := srv.LoginAdmin()
	flowID := createLoginFlow(t, srv, token, map[string]any{
		"name":     "Asset Import",
		"strategy": "identifier_first",
		"state":    "draft",
	})

	remote := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.Header().Set("Content-Type", "image/png")
		_, _ = w.Write(decodeTinyPNG(t))
	}))
	defer remote.Close()

	code, body := srv.PostJSONWithBearer("/v1/login-flows/"+flowID+"/assets/import", map[string]any{
		"slot": "cover_image",
		"url":  remote.URL + "/cover.png",
	}, token)
	if code != http.StatusCreated {
		t.Fatalf("expected 201 importing asset, got %d: %#v", code, body)
	}
	importURL, _ := body["url"].(string)
	if importURL == "" || importURL[:14] != "/assets/login/" {
		t.Fatalf("import url = %q, want same-origin asset path", importURL)
	}

	code, flowBody := srv.GetWithBearer("/v1/login-flows/"+flowID, token)
	if code != http.StatusOK {
		t.Fatalf("expected 200 getting login flow, got %d: %#v", code, flowBody)
	}
	config, _ := flowBody["config"].(map[string]any)
	branding, _ := config["branding"].(map[string]any)
	if branding["cover_image"] != importURL {
		t.Fatalf("branding.cover_image = %v, want %s", branding["cover_image"], importURL)
	}
}

func createLoginFlow(t *testing.T, srv *testutil.TestServer, token string, body map[string]any) string {
	t.Helper()
	code, resp := srv.PostJSONWithBearer("/v1/login-flows", body, token)
	if code != http.StatusCreated {
		t.Fatalf("expected 201 creating login flow, got %d: %#v", code, resp)
	}
	flowID, _ := resp["id"].(string)
	if flowID == "" {
		t.Fatalf("missing flow id in response: %#v", resp)
	}
	return flowID
}

func uploadLoginFlowAsset(t *testing.T, srv *testutil.TestServer, token, flowID, slot, filename string, payload []byte) map[string]any {
	t.Helper()

	var body bytes.Buffer
	writer := multipart.NewWriter(&body)
	if err := writer.WriteField("slot", slot); err != nil {
		t.Fatalf("write slot field: %v", err)
	}
	part, err := writer.CreateFormFile("file", filename)
	if err != nil {
		t.Fatalf("create form file: %v", err)
	}
	if _, err := part.Write(payload); err != nil {
		t.Fatalf("write file payload: %v", err)
	}
	if err := writer.Close(); err != nil {
		t.Fatalf("close multipart writer: %v", err)
	}

	req, err := http.NewRequest(http.MethodPost, srv.URL()+"/v1/login-flows/"+flowID+"/assets", &body)
	if err != nil {
		t.Fatalf("new upload request: %v", err)
	}
	req.Header.Set("Authorization", "Bearer "+token)
	req.Header.Set("Content-Type", writer.FormDataContentType())

	resp, err := http.DefaultClient.Do(req)
	if err != nil {
		t.Fatalf("POST asset upload: %v", err)
	}
	defer resp.Body.Close()
	if resp.StatusCode != http.StatusCreated {
		raw, _ := io.ReadAll(resp.Body)
		t.Fatalf("expected 201 uploading asset, got %d: %s", resp.StatusCode, string(raw))
	}

	var decoded map[string]any
	if err := json.NewDecoder(resp.Body).Decode(&decoded); err != nil {
		t.Fatalf("decode upload response: %v", err)
	}
	return decoded
}

func decodeTinyPNG(t *testing.T) []byte {
	t.Helper()
	payload, err := base64.StdEncoding.DecodeString(tinyPNGBase64)
	if err != nil {
		t.Fatalf("decode PNG fixture: %v", err)
	}
	return payload
}
