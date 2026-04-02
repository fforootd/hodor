package api

import (
	"context"
	"database/sql"
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
	if err := decodeJSONBody(r, &req); err != nil {
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

	slot, err := a.loginFlowStore.DeleteAsset(r.Context(), flowID, assetID)
	if err != nil {
		if err == sql.ErrNoRows {
			httputil.WriteError(w, http.StatusNotFound, "asset not found")
			return
		}
		httputil.WriteError(w, http.StatusInternalServerError, "delete failed")
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

	data, err := a.loginFlowStore.GetAsset(r.Context(), assetID)
	if err != nil {
		http.NotFound(w, r)
		return
	}

	if match := r.Header.Get("If-None-Match"); match != "" && match == data.ETag {
		httputil.SetImmutableAssetCache(w)
		w.Header().Set("ETag", data.ETag)
		w.WriteHeader(http.StatusNotModified)
		return
	}

	httputil.SetImmutableAssetCache(w)
	w.Header().Set("ETag", data.ETag)
	w.Header().Set("Content-Type", data.ContentType)
	w.Header().Set("Content-Length", fmt.Sprintf("%d", len(data.Payload)))
	w.WriteHeader(http.StatusOK)
	_, _ = w.Write(data.Payload)
}

func (a *API) replaceLoginFlowAsset(ctx context.Context, flowID, slot, filename, contentType string, payload []byte) (loginFlowAssetResponse, error) {
	asset, err := a.loginFlowStore.ReplaceAsset(ctx, flowID, slot, filename, contentType, payload)
	if err != nil {
		return loginFlowAssetResponse{}, err
	}
	return loginFlowAssetResponse{
		ID:          asset.ID,
		LoginFlowID: asset.LoginFlowID,
		Slot:        asset.Slot,
		URL:         asset.URL,
		Filename:    asset.Filename,
		ContentType: asset.ContentType,
		SizeBytes:   asset.SizeBytes,
		ETag:        asset.ETag,
	}, nil
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

func decodeJSONBody(r *http.Request, dst any) error {
	return json.NewDecoder(r.Body).Decode(dst)
}
