package api

import (
	"net/http"

	"github.com/zitadel/zitadel/internal/httputil"
)

// openAPISpecFromRegistry serves the OpenAPI 3.1 spec generated from the registry.
func (a *API) openAPISpecFromRegistry(w http.ResponseWriter, r *http.Request) {
	spec := a.spec.Spec()
	httputil.WriteJSON(w, http.StatusOK, spec)
}

// Spec returns the OpenAPI registry for CLI export.
func (a *API) Spec() *OpenAPIRegistry {
	return a.spec
}

// RegisterOpenAPIOnly populates the spec registry without mounting HTTP handlers.
// Used by the CLI export command where no server is needed.
func (a *API) RegisterOpenAPIOnly() {
	a.registerOpenAPIOperations()
}
