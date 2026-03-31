package api

import (
	"encoding/json"
	"net/http"

	"github.com/zitadel/zitadel/internal/httputil"
	"github.com/zitadel/zitadel/internal/notify"
)

// RegisterNotificationRoutes mounts notification preview/test helpers.
func (a *API) RegisterNotificationRoutes(mux *http.ServeMux) {
	mux.HandleFunc("GET /v1/notifications/presets", a.listNotificationPresets)
	mux.HandleFunc("POST /v1/notifications/preview", a.previewNotification)
	mux.HandleFunc("POST /v1/notifications/test", a.testNotification)
}

func (a *API) listNotificationPresets(w http.ResponseWriter, r *http.Request) {
	presets := notify.Presets()
	resp := NotificationPresetsResponse{Presets: make([]NotificationPreset, 0, len(presets))}
	for _, preset := range presets {
		resp.Presets = append(resp.Presets, NotificationPreset{
			ID:          preset.ID,
			Label:       preset.Label,
			Medium:      preset.Medium,
			Driver:      preset.Driver,
			Description: preset.Description,
			Config:      preset.Config,
		})
	}
	httputil.WriteJSON(w, http.StatusOK, resp)
}

func (a *API) previewNotification(w http.ResponseWriter, r *http.Request) {
	if a.notifier == nil {
		httputil.WriteError(w, http.StatusServiceUnavailable, "notification service unavailable")
		return
	}
	var req NotificationPreviewRequest
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		httputil.WriteError(w, http.StatusBadRequest, "invalid JSON body")
		return
	}
	if req.Medium == "" || req.TemplateKey == "" {
		httputil.WriteError(w, http.StatusBadRequest, "medium and template_key are required")
		return
	}
	if req.OrgID == "" {
		req.OrgID = r.Header.Get("X-Org-Id")
	}
	rendered, err := a.notifier.Preview(r.Context(), notify.PreviewRequest{
		OrgID:       req.OrgID,
		Medium:      req.Medium,
		TemplateKey: req.TemplateKey,
		Locale:      req.Locale,
		Payload:     req.Payload,
	})
	if err != nil {
		httputil.WriteError(w, http.StatusBadRequest, err.Error())
		return
	}
	httputil.WriteJSON(w, http.StatusOK, NotificationRenderResponse(*rendered))
}

func (a *API) testNotification(w http.ResponseWriter, r *http.Request) {
	if a.notifier == nil {
		httputil.WriteError(w, http.StatusServiceUnavailable, "notification service unavailable")
		return
	}
	var req NotificationTestRequest
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		httputil.WriteError(w, http.StatusBadRequest, "invalid JSON body")
		return
	}
	if req.Medium == "" || req.TemplateKey == "" || req.Recipient == "" {
		httputil.WriteError(w, http.StatusBadRequest, "medium, template_key, and recipient are required")
		return
	}
	if req.OrgID == "" {
		req.OrgID = r.Header.Get("X-Org-Id")
	}
	rendered, err := a.notifier.SendTest(r.Context(), notify.TestRequest{
		OrgID:       req.OrgID,
		Medium:      req.Medium,
		ChannelID:   req.ChannelID,
		Recipient:   req.Recipient,
		TemplateKey: req.TemplateKey,
		Locale:      req.Locale,
		Payload:     req.Payload,
	})
	if err != nil {
		httputil.WriteError(w, http.StatusBadRequest, err.Error())
		return
	}
	httputil.WriteJSON(w, http.StatusOK, NotificationRenderResponse(*rendered))
}
