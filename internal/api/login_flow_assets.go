package api

import (
	"context"
	"crypto/sha256"
	"database/sql"
	"encoding/hex"
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"net/url"
	"path"
	"path/filepath"
	"strings"
	"time"

	"github.com/zitadel/zitadel/internal/httputil"
	"github.com/zitadel/zitadel/internal/id"
)

const maxLoginFlowAssetBytes = 5 << 20

var loginFlowAssetFields = map[string]struct{}{
	"logo_url":    {},
	"logo_dark":   {},
	"cover_image": {},
	"favicon":     {},
}

type loginFlowAssetResponse struct {
	ID          string `json:"id"`
	LoginFlowID string `json:"login_flow_id"`
	Slot        string `json:"slot"`
	URL         string `json:"url"`
	Filename    string `json:"filename"`
	ContentType string `json:"content_type"`
	SizeBytes   int64  `json:"size_bytes"`
	ETag        string `json:"etag"`
}

func (a *API) uploadLoginFlowAsset(w http.ResponseWriter, r *http.Request) {
	flowID := r.PathValue("id")
	if flowID == "" {
		httputil.WriteError(w, http.StatusBadRequest, "id required")
		return
	}
	if err := r.ParseMultipartForm(maxLoginFlowAssetBytes + (1 << 20)); err != nil {
		httputil.WriteError(w, http.StatusBadRequest, "invalid multipart form")
		return
	}

	slot := r.FormValue("slot")
	if _, ok := loginFlowAssetFields[slot]; !ok {
		httputil.WriteError(w, http.StatusBadRequest, "invalid asset slot")
		return
	}

	file, header, err := r.FormFile("file")
	if err != nil {
		httputil.WriteError(w, http.StatusBadRequest, "file is required")
		return
	}
	defer file.Close()

	payload, err := io.ReadAll(io.LimitReader(file, maxLoginFlowAssetBytes+1))
	if err != nil {
		httputil.WriteError(w, http.StatusBadRequest, "failed to read upload")
		return
	}
	if len(payload) == 0 {
		httputil.WriteError(w, http.StatusBadRequest, "file is empty")
		return
	}
	if len(payload) > maxLoginFlowAssetBytes {
		httputil.WriteError(w, http.StatusRequestEntityTooLarge, "file too large")
		return
	}

	contentType, err := detectLoginFlowAssetContentType(header.Filename, header.Header.Get("Content-Type"), payload)
	if err != nil {
		httputil.WriteError(w, http.StatusBadRequest, err.Error())
		return
	}

	resp, err := a.replaceLoginFlowAsset(r.Context(), flowID, slot, header.Filename, contentType, payload)
	if err != nil {
		httputil.WriteError(w, http.StatusInternalServerError, err.Error())
		return
	}
	httputil.WriteJSON(w, http.StatusCreated, resp)
}

func (a *API) importLoginFlowAsset(w http.ResponseWriter, r *http.Request) {
	flowID := r.PathValue("id")
	if flowID == "" {
		httputil.WriteError(w, http.StatusBadRequest, "id required")
		return
	}

	var req struct {
		Slot string `json:"slot"`
		URL  string `json:"url"`
	}
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		httputil.WriteError(w, http.StatusBadRequest, "invalid JSON body")
		return
	}
	if _, ok := loginFlowAssetFields[req.Slot]; !ok {
		httputil.WriteError(w, http.StatusBadRequest, "invalid asset slot")
		return
	}
	parsedURL, err := url.Parse(req.URL)
	if err != nil || parsedURL.Scheme == "" || parsedURL.Host == "" {
		httputil.WriteError(w, http.StatusBadRequest, "invalid asset URL")
		return
	}
	if parsedURL.Scheme != "http" && parsedURL.Scheme != "https" {
		httputil.WriteError(w, http.StatusBadRequest, "only http and https URLs are supported")
		return
	}

	client := &http.Client{Timeout: 10 * time.Second}
	remoteReq, err := http.NewRequestWithContext(r.Context(), http.MethodGet, req.URL, nil)
	if err != nil {
		httputil.WriteError(w, http.StatusBadRequest, "invalid asset URL")
		return
	}
	resp, err := client.Do(remoteReq)
	if err != nil {
		httputil.WriteError(w, http.StatusBadGateway, "failed to fetch remote asset")
		return
	}
	defer resp.Body.Close()

	if resp.StatusCode < 200 || resp.StatusCode >= 300 {
		httputil.WriteError(w, http.StatusBadGateway, "remote asset returned an error")
		return
	}

	payload, err := io.ReadAll(io.LimitReader(resp.Body, maxLoginFlowAssetBytes+1))
	if err != nil {
		httputil.WriteError(w, http.StatusBadGateway, "failed to read remote asset")
		return
	}
	if len(payload) == 0 {
		httputil.WriteError(w, http.StatusBadRequest, "remote asset is empty")
		return
	}
	if len(payload) > maxLoginFlowAssetBytes {
		httputil.WriteError(w, http.StatusRequestEntityTooLarge, "remote asset is too large")
		return
	}

	filename := path.Base(parsedURL.Path)
	if filename == "" || filename == "." || filename == "/" {
		filename = req.Slot
	}

	contentType, err := detectLoginFlowAssetContentType(filename, resp.Header.Get("Content-Type"), payload)
	if err != nil {
		httputil.WriteError(w, http.StatusBadRequest, err.Error())
		return
	}

	assetResp, err := a.replaceLoginFlowAsset(r.Context(), flowID, req.Slot, filename, contentType, payload)
	if err != nil {
		httputil.WriteError(w, http.StatusInternalServerError, err.Error())
		return
	}
	httputil.WriteJSON(w, http.StatusCreated, assetResp)
}

func (a *API) deleteLoginFlowAsset(w http.ResponseWriter, r *http.Request) {
	flowID := r.PathValue("id")
	assetID := r.PathValue("assetId")
	if flowID == "" || assetID == "" {
		httputil.WriteError(w, http.StatusBadRequest, "id and assetId required")
		return
	}

	tx, err := a.db.SQL().BeginTx(r.Context(), nil)
	if err != nil {
		httputil.WriteError(w, http.StatusInternalServerError, "database error")
		return
	}
	defer tx.Rollback()

	var slot string
	err = tx.QueryRowContext(
		r.Context(),
		fmt.Sprintf("SELECT slot FROM login_flow_assets WHERE id = %s AND login_flow_id = %s", a.db.Placeholder(1), a.db.Placeholder(2)),
		assetID,
		flowID,
	).Scan(&slot)
	if err != nil {
		if err == sql.ErrNoRows {
			httputil.WriteError(w, http.StatusNotFound, "asset not found")
			return
		}
		httputil.WriteError(w, http.StatusInternalServerError, "query failed")
		return
	}

	if _, err = tx.ExecContext(
		r.Context(),
		fmt.Sprintf("DELETE FROM login_flow_assets WHERE id = %s AND login_flow_id = %s", a.db.Placeholder(1), a.db.Placeholder(2)),
		assetID,
		flowID,
	); err != nil {
		httputil.WriteError(w, http.StatusInternalServerError, "delete failed")
		return
	}

	if err = a.clearLoginFlowBrandingField(r.Context(), tx, flowID, slot); err != nil {
		httputil.WriteError(w, http.StatusInternalServerError, "failed to update login flow config")
		return
	}
	if err = tx.Commit(); err != nil {
		httputil.WriteError(w, http.StatusInternalServerError, "commit failed")
		return
	}

	httputil.WriteJSON(w, http.StatusOK, map[string]any{"id": assetID, "slot": slot, "deleted": true})
}

func (a *API) serveLoginFlowAsset(w http.ResponseWriter, r *http.Request) {
	assetID := r.PathValue("id")
	if assetID == "" {
		http.NotFound(w, r)
		return
	}

	var contentType, etag string
	var payload []byte
	err := a.db.SQL().QueryRowContext(
		r.Context(),
		fmt.Sprintf("SELECT content_type, etag, data FROM login_flow_assets WHERE id = %s", a.db.Placeholder(1)),
		assetID,
	).Scan(&contentType, &etag, &payload)
	if err != nil {
		http.NotFound(w, r)
		return
	}

	if match := r.Header.Get("If-None-Match"); match != "" && match == etag {
		httputil.SetImmutableAssetCache(w)
		w.Header().Set("ETag", etag)
		w.WriteHeader(http.StatusNotModified)
		return
	}

	httputil.SetImmutableAssetCache(w)
	w.Header().Set("ETag", etag)
	w.Header().Set("Content-Type", contentType)
	w.Header().Set("Content-Length", fmt.Sprintf("%d", len(payload)))
	w.WriteHeader(http.StatusOK)
	_, _ = w.Write(payload)
}

func (a *API) replaceLoginFlowAsset(ctx context.Context, flowID, slot, filename, contentType string, payload []byte) (loginFlowAssetResponse, error) {
	tx, err := a.db.SQL().BeginTx(ctx, nil)
	if err != nil {
		return loginFlowAssetResponse{}, fmt.Errorf("database error")
	}
	defer tx.Rollback()

	orgID, config, err := a.loadLoginFlowConfigForAsset(ctx, tx, flowID)
	if err != nil {
		if err == sql.ErrNoRows {
			return loginFlowAssetResponse{}, fmt.Errorf("login flow not found")
		}
		return loginFlowAssetResponse{}, fmt.Errorf("query failed")
	}

	if _, err = tx.ExecContext(
		ctx,
		fmt.Sprintf("DELETE FROM login_flow_assets WHERE login_flow_id = %s AND slot = %s", a.db.Placeholder(1), a.db.Placeholder(2)),
		flowID,
		slot,
	); err != nil {
		return loginFlowAssetResponse{}, fmt.Errorf("failed to replace existing asset")
	}

	assetID := id.New()
	sum := sha256.Sum256(payload)
	etag := fmt.Sprintf(`"%s"`, hex.EncodeToString(sum[:]))
	now := time.Now().UTC().Format(time.RFC3339)

	insertQuery := `INSERT INTO login_flow_assets (id, org_id, login_flow_id, slot, filename, content_type, size_bytes, sha256, etag, data, metadata, created_at, updated_at)
		 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`
	if a.db.Dialect() == "postgres" {
		insertQuery = `INSERT INTO login_flow_assets (id, org_id, login_flow_id, slot, filename, content_type, size_bytes, sha256, etag, data, metadata, created_at, updated_at)
		 VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)`
	}
	if _, err = tx.ExecContext(
		ctx,
		insertQuery,
		assetID,
		orgID,
		flowID,
		slot,
		filename,
		contentType,
		len(payload),
		hex.EncodeToString(sum[:]),
		etag,
		payload,
		"{}",
		now,
		now,
	); err != nil {
		return loginFlowAssetResponse{}, fmt.Errorf("failed to save asset")
	}

	assetURL := "/assets/login/" + assetID
	if err = setLoginFlowBrandingField(config, slot, assetURL); err != nil {
		return loginFlowAssetResponse{}, fmt.Errorf("failed to update login flow config")
	}
	if err = a.updateLoginFlowConfig(ctx, tx, flowID, config); err != nil {
		return loginFlowAssetResponse{}, fmt.Errorf("failed to update login flow config")
	}
	if err = tx.Commit(); err != nil {
		return loginFlowAssetResponse{}, fmt.Errorf("commit failed")
	}

	return loginFlowAssetResponse{
		ID:          assetID,
		LoginFlowID: flowID,
		Slot:        slot,
		URL:         assetURL,
		Filename:    filename,
		ContentType: contentType,
		SizeBytes:   int64(len(payload)),
		ETag:        etag,
	}, nil
}

func (a *API) loadLoginFlowConfigForAsset(ctx context.Context, tx *sql.Tx, flowID string) (string, map[string]any, error) {
	var orgID string
	var configJSON string
	err := tx.QueryRowContext(
		ctx,
		fmt.Sprintf("SELECT COALESCE(org_id, '1'), COALESCE(config, '{}') FROM login_flows WHERE id = %s", a.db.Placeholder(1)),
		flowID,
	).Scan(&orgID, &configJSON)
	if err != nil {
		return "", nil, err
	}

	var config map[string]any
	if err = json.Unmarshal([]byte(configJSON), &config); err != nil || config == nil {
		config = map[string]any{}
	}
	return orgID, config, nil
}

func (a *API) updateLoginFlowConfig(ctx context.Context, tx *sql.Tx, flowID string, config map[string]any) error {
	configBytes, err := json.Marshal(config)
	if err != nil {
		return err
	}
	_, err = tx.ExecContext(
		ctx,
		fmt.Sprintf("UPDATE login_flows SET config = %s, updated_at = %s WHERE id = %s", a.db.Placeholder(1), a.db.Placeholder(2), a.db.Placeholder(3)),
		string(configBytes),
		time.Now().UTC().Format(time.RFC3339),
		flowID,
	)
	return err
}

func (a *API) clearLoginFlowBrandingField(ctx context.Context, tx *sql.Tx, flowID, slot string) error {
	_, config, err := a.loadLoginFlowConfigForAsset(ctx, tx, flowID)
	if err != nil {
		return err
	}
	if err := setLoginFlowBrandingField(config, slot, ""); err != nil {
		return err
	}
	return a.updateLoginFlowConfig(ctx, tx, flowID, config)
}

func setLoginFlowBrandingField(config map[string]any, field, value string) error {
	branding, ok := config["branding"].(map[string]any)
	if !ok || branding == nil {
		branding = map[string]any{}
	}
	if value == "" {
		delete(branding, field)
	} else {
		branding[field] = value
	}
	config["branding"] = branding
	return nil
}

func detectLoginFlowAssetContentType(filename, declared string, payload []byte) (string, error) {
	candidate := strings.ToLower(strings.TrimSpace(strings.Split(declared, ";")[0]))
	switch {
	case strings.HasPrefix(candidate, "image/"):
		return candidate, nil
	case strings.EqualFold(filepath.Ext(filename), ".svg"):
		return "image/svg+xml", nil
	}

	sniffed := strings.ToLower(strings.TrimSpace(strings.Split(http.DetectContentType(payload), ";")[0]))
	if strings.HasPrefix(sniffed, "image/") {
		return sniffed, nil
	}
	return "", fmt.Errorf("only image uploads are supported")
}
