package api

import (
	"encoding/json"
	"fmt"
	"net/http"

	"github.com/zitadel/zitadel/internal/httputil"

	"github.com/zitadel/zitadel/internal/fga"
)

// FGAService holds the FGA service reference for the API layer.
// Set by the server after FGA initialization.
var FGAService *fga.Service

// RegisterFGARoutes mounts the customer-facing FGA API endpoints.
// These allow customers to use FGA for their own applications.
func (a *API) RegisterFGARoutes(mux *http.ServeMux) {
	// System FGA operations (internal Zitadel authorization).
	mux.HandleFunc("POST /v1/fga/check", a.fgaCheck)
	mux.HandleFunc("POST /v1/fga/tuples", a.fgaWriteTuples)
	mux.HandleFunc("DELETE /v1/fga/tuples", a.fgaDeleteTuples)
	mux.HandleFunc("GET /v1/fga/tuples", a.fgaReadTuples)
	mux.HandleFunc("POST /v1/fga/list-objects", a.fgaListObjects)
	mux.HandleFunc("GET /v1/fga/model", a.fgaGetModel)
	mux.HandleFunc("GET /v1/fga/model/graph", a.fgaModelGraph)
	mux.HandleFunc("POST /v1/fga/expand", a.fgaExpand)
	mux.HandleFunc("POST /v1/fga/test", a.fgaBatchTest)
}

// fgaCheck performs an authorization check.
// POST /v1/fga/check
// Body: { "user": "user:alice", "relation": "can_edit", "object": "document:123" }
func (a *API) fgaCheck(w http.ResponseWriter, r *http.Request) {
	svc := FGAService
	if svc == nil {
		httputil.WriteError(w, http.StatusServiceUnavailable, "FGA not initialized")
		return
	}

	var req struct {
		User     string `json:"user"`
		Relation string `json:"relation"`
		Object   string `json:"object"`
	}
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		httputil.WriteError(w, http.StatusBadRequest, "invalid JSON body")
		return
	}
	if req.User == "" || req.Relation == "" || req.Object == "" {
		httputil.WriteError(w, http.StatusBadRequest, "user, relation, and object are required")
		return
	}

	allowed, err := svc.Check(r.Context(), req.User, req.Relation, req.Object)
	if err != nil {
		httputil.WriteError(w, http.StatusInternalServerError, fmt.Sprintf("FGA check failed: %v", err))
		return
	}

	httputil.WriteJSON(w, http.StatusOK, map[string]any{
		"allowed":  allowed,
		"user":     req.User,
		"relation": req.Relation,
		"object":   req.Object,
	})
}

// fgaWriteTuples writes relationship tuples.
// POST /v1/fga/tuples
// Body: { "tuples": [{"user":"user:alice","relation":"editor","object":"doc:1"}] }
func (a *API) fgaWriteTuples(w http.ResponseWriter, r *http.Request) {
	svc := FGAService
	if svc == nil {
		httputil.WriteError(w, http.StatusServiceUnavailable, "FGA not initialized")
		return
	}

	var req struct {
		Tuples []struct {
			User     string `json:"user"`
			Relation string `json:"relation"`
			Object   string `json:"object"`
		} `json:"tuples"`
	}
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		httputil.WriteError(w, http.StatusBadRequest, "invalid JSON body")
		return
	}
	if len(req.Tuples) == 0 {
		httputil.WriteError(w, http.StatusBadRequest, "at least one tuple is required")
		return
	}

	tuples := make([][3]string, len(req.Tuples))
	for i, t := range req.Tuples {
		if t.User == "" || t.Relation == "" || t.Object == "" {
			httputil.WriteError(w, http.StatusBadRequest, fmt.Sprintf("tuple %d: user, relation, and object are required", i))
			return
		}
		tuples[i] = [3]string{t.User, t.Relation, t.Object}
	}

	if err := svc.WriteTuples(r.Context(), tuples...); err != nil {
		httputil.WriteError(w, http.StatusInternalServerError, fmt.Sprintf("write tuples failed: %v", err))
		return
	}

	httputil.WriteJSON(w, http.StatusOK, map[string]any{
		"status":  "ok",
		"written": len(tuples),
	})
}

// fgaDeleteTuples removes relationship tuples.
// DELETE /v1/fga/tuples
// Body: { "tuples": [{"user":"user:alice","relation":"editor","object":"doc:1"}] }
func (a *API) fgaDeleteTuples(w http.ResponseWriter, r *http.Request) {
	svc := FGAService
	if svc == nil {
		httputil.WriteError(w, http.StatusServiceUnavailable, "FGA not initialized")
		return
	}

	var req struct {
		Tuples []struct {
			User     string `json:"user"`
			Relation string `json:"relation"`
			Object   string `json:"object"`
		} `json:"tuples"`
	}
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		httputil.WriteError(w, http.StatusBadRequest, "invalid JSON body")
		return
	}

	tuples := make([][3]string, len(req.Tuples))
	for i, t := range req.Tuples {
		tuples[i] = [3]string{t.User, t.Relation, t.Object}
	}

	if err := svc.DeleteTuples(r.Context(), tuples...); err != nil {
		httputil.WriteError(w, http.StatusInternalServerError, fmt.Sprintf("delete tuples failed: %v", err))
		return
	}

	httputil.WriteJSON(w, http.StatusOK, map[string]any{
		"status":  "ok",
		"deleted": len(tuples),
	})
}

// fgaReadTuples reads relationship tuples for a given filter.
// GET /v1/fga/tuples?user=user:alice&relation=editor&object=doc:1
func (a *API) fgaReadTuples(w http.ResponseWriter, r *http.Request) {
	svc := FGAService
	if svc == nil {
		httputil.WriteError(w, http.StatusServiceUnavailable, "FGA not initialized")
		return
	}

	user := r.URL.Query().Get("user")
	relation := r.URL.Query().Get("relation")
	object := r.URL.Query().Get("object")

	// If no filters specified, read all tuples via direct SQL.
	if object == "" && user == "" && relation == "" {
		allTuples, err := svc.ReadAllTuples(r.Context())
		if err != nil {
			httputil.WriteError(w, http.StatusInternalServerError, fmt.Sprintf("read tuples failed: %v", err))
			return
		}
		httputil.WriteJSON(w, http.StatusOK, map[string]any{
			"tuples": allTuples,
		})
		return
	}

	resp, err := svc.ReadTuples(r.Context(), user, relation, object)
	if err != nil {
		httputil.WriteError(w, http.StatusInternalServerError, fmt.Sprintf("read tuples failed: %v", err))
		return
	}

	httputil.WriteJSON(w, http.StatusOK, map[string]any{
		"tuples": resp,
	})
}

// fgaListObjects lists objects a user has access to.
// POST /v1/fga/list-objects
// Body: { "user": "user:alice", "relation": "can_read", "type": "document" }
func (a *API) fgaListObjects(w http.ResponseWriter, r *http.Request) {
	svc := FGAService
	if svc == nil {
		httputil.WriteError(w, http.StatusServiceUnavailable, "FGA not initialized")
		return
	}

	var req struct {
		User     string `json:"user"`
		Relation string `json:"relation"`
		Type     string `json:"type"`
	}
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		httputil.WriteError(w, http.StatusBadRequest, "invalid JSON body")
		return
	}
	if req.User == "" || req.Relation == "" || req.Type == "" {
		httputil.WriteError(w, http.StatusBadRequest, "user, relation, and type are required")
		return
	}

	objects, err := svc.ListObjects(r.Context(), req.User, req.Relation, req.Type)
	if err != nil {
		httputil.WriteError(w, http.StatusInternalServerError, fmt.Sprintf("list objects failed: %v", err))
		return
	}

	httputil.WriteJSON(w, http.StatusOK, map[string]any{
		"objects": objects,
	})
}

// fgaGetModel returns the current FGA authorization model.
// GET /v1/fga/model
func (a *API) fgaGetModel(w http.ResponseWriter, _ *http.Request) {
	// Return a description of the model types and relations.
	model := fga.ZitadelModel()
	types := make([]map[string]any, len(model))
	for i, td := range model {
		relations := make([]string, 0, len(td.GetRelations()))
		for name := range td.GetRelations() {
			relations = append(relations, name)
		}
		types[i] = map[string]any{
			"type":      td.GetType(),
			"relations": relations,
		}
	}

	httputil.WriteJSON(w, http.StatusOK, map[string]any{
		"schema_version": "1.1",
		"types":          types,
	})
}

// fgaExpand expands the relationship tree for a given object and relation.
// POST /v1/fga/expand
// Body: { "relation": "can_read", "object": "entity:123" }
func (a *API) fgaExpand(w http.ResponseWriter, r *http.Request) {
	svc := FGAService
	if svc == nil {
		httputil.WriteError(w, http.StatusServiceUnavailable, "FGA not initialized")
		return
	}

	var req struct {
		Relation string `json:"relation"`
		Object   string `json:"object"`
	}
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		httputil.WriteError(w, http.StatusBadRequest, "invalid JSON body")
		return
	}
	if req.Relation == "" || req.Object == "" {
		httputil.WriteError(w, http.StatusBadRequest, "relation and object are required")
		return
	}

	tree, err := svc.Expand(r.Context(), req.Relation, req.Object)
	if err != nil {
		httputil.WriteError(w, http.StatusInternalServerError, fmt.Sprintf("expand failed: %v", err))
		return
	}

	httputil.WriteJSON(w, http.StatusOK, map[string]any{
		"tree": tree,
	})
}

// fgaModelGraph returns the authorization model as a graph structure
// for interactive visualization in the console.
// GET /v1/fga/model/graph
func (a *API) fgaModelGraph(w http.ResponseWriter, _ *http.Request) {
	model := fga.ZitadelModel()

	type Edge struct {
		From     string `json:"from"`
		To       string `json:"to"`
		Relation string `json:"relation"`
		Kind     string `json:"kind"` // "direct", "computed", "inherited"
	}

	type Node struct {
		ID          string   `json:"id"`
		Relations   []string `json:"relations"`
		Permissions []string `json:"permissions"`
	}

	var nodes []Node
	var edges []Edge

	for _, td := range model {
		typeName := td.GetType()
		rels := make([]string, 0)
		perms := make([]string, 0)
		for name := range td.GetRelations() {
			// Heuristic: permissions start with "can_"
			if len(name) > 4 && name[:4] == "can_" {
				perms = append(perms, name)
			} else {
				rels = append(rels, name)
			}
		}
		nodes = append(nodes, Node{
			ID:          typeName,
			Relations:   rels,
			Permissions: perms,
		})

		// Parse metadata to find cross-type relationships.
		if td.GetMetadata() != nil {
			for relName, relMeta := range td.GetMetadata().GetRelations() {
				for _, ref := range relMeta.GetDirectlyRelatedUserTypes() {
					refType := ref.GetType()
					if refType != "" && refType != typeName {
						edges = append(edges, Edge{
							From:     refType,
							To:       typeName,
							Relation: relName,
							Kind:     "direct",
						})
					}
				}
			}
		}
	}

	httputil.WriteJSON(w, http.StatusOK, map[string]any{
		"nodes": nodes,
		"edges": edges,
	})
}

// fgaBatchTest runs multiple authorization checks in one request.
// POST /v1/fga/test
// Body: { "assertions": [{"user":"user:admin","relation":"can_read","object":"org:1","expected":true}] }
func (a *API) fgaBatchTest(w http.ResponseWriter, r *http.Request) {
	svc := FGAService
	if svc == nil {
		httputil.WriteError(w, http.StatusServiceUnavailable, "FGA not initialized")
		return
	}

	var req struct {
		Assertions []struct {
			User     string `json:"user"`
			Relation string `json:"relation"`
			Object   string `json:"object"`
			Expected bool   `json:"expected"`
		} `json:"assertions"`
	}
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		httputil.WriteError(w, http.StatusBadRequest, "invalid JSON body")
		return
	}

	type Result struct {
		User     string `json:"user"`
		Relation string `json:"relation"`
		Object   string `json:"object"`
		Expected bool   `json:"expected"`
		Actual   bool   `json:"actual"`
		Pass     bool   `json:"pass"`
		Error    string `json:"error,omitempty"`
	}

	results := make([]Result, len(req.Assertions))
	passed := 0
	for i, a := range req.Assertions {
		result := Result{
			User:     a.User,
			Relation: a.Relation,
			Object:   a.Object,
			Expected: a.Expected,
		}
		allowed, err := svc.Check(r.Context(), a.User, a.Relation, a.Object)
		if err != nil {
			result.Error = err.Error()
		} else {
			result.Actual = allowed
			result.Pass = allowed == a.Expected
			if result.Pass {
				passed++
			}
		}
		results[i] = result
	}

	httputil.WriteJSON(w, http.StatusOK, map[string]any{
		"results": results,
		"total":   len(results),
		"passed":  passed,
		"failed":  len(results) - passed,
	})
}
