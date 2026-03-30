package api

import (
	"context"
	"net/http"

	"github.com/zitadel/zitadel/internal/fga"
	"github.com/zitadel/zitadel/internal/httputil"
)

// memberFGA describes the FGA operations for a specific resource type's members.
type memberFGA struct {
	Add    func(ctx context.Context, userID, resourceID string) error
	Remove func(ctx context.Context, userID, resourceID string) error
}

// memberFGAFor returns the FGA member operations for a given resource type.
// Returns nil if FGA is not initialized.
func memberFGAFor(resourceType string) *memberFGA {
	svc := FGAService
	if svc == nil {
		return nil
	}
	switch resourceType {
	case "group":
		return &memberFGA{
			Add:    func(ctx context.Context, uid, rid string) error { return svc.AddGroupMember(ctx, uid, rid) },
			Remove: func(ctx context.Context, uid, rid string) error { return svc.RemoveGroupMember(ctx, uid, rid) },
		}
	case "project":
		return &memberFGA{
			Add:    func(ctx context.Context, uid, rid string) error { return svc.AddProjectMember(ctx, uid, rid) },
			Remove: func(ctx context.Context, uid, rid string) error { return svc.RemoveProjectMember(ctx, uid, rid) },
		}
	case "org":
		return &memberFGA{
			Add:    func(ctx context.Context, uid, rid string) error { return svc.AddOrgMember(ctx, uid, rid) },
			Remove: func(ctx context.Context, uid, rid string) error { return svc.RemoveOrgMember(ctx, uid, rid) },
		}
	}
	return nil
}

// orgAdminFGA returns the FGA add function for org admins.
func orgAdminFGA() func(ctx context.Context, userID, orgID string) error {
	svc := FGAService
	if svc == nil {
		return nil
	}
	return svc.AddOrgAdmin
}

// listMembers returns a handler that lists members for a given resource type.
// This replaces the 3 near-identical listOrgMembers, listGroupMembers, listProjectMembers.
func (a *API) listMembers(resourceType string) http.HandlerFunc {
	return func(w http.ResponseWriter, r *http.Request) {
		resourceID, ok := requireID(w, r, "id")
		if !ok {
			return
		}

		rows, err := a.db.SQL().QueryContext(r.Context(),
			`SELECT m.user_id, COALESCE(u.display_name, u.identifier, ''), m.role, m.added_at
			 FROM memberships m
			 LEFT JOIN users u ON u.id = m.user_id
			 WHERE m.resource_type = ? AND m.resource_id = ?
			 ORDER BY m.added_at ASC`, resourceType, resourceID)
		if err != nil {
			httputil.WriteError(w, http.StatusInternalServerError, "query failed")
			return
		}
		defer rows.Close()

		var members []MemberResponse
		for rows.Next() {
			var m MemberResponse
			if err := rows.Scan(&m.UserID, &m.DisplayName, &m.Role, &m.AddedAt); err != nil {
				continue
			}
			members = append(members, m)
		}
		if err := rows.Err(); err != nil {
			httputil.WriteError(w, http.StatusInternalServerError, "row iteration failed")
			return
		}
		if members == nil {
			members = []MemberResponse{}
		}
		httputil.WriteJSON(w, http.StatusOK, ListResponse{Items: members})
	}
}

// addMember returns a handler that adds a member to a given resource type.
// For orgs, it dispatches admin role to AddOrgAdmin.
func (a *API) addMember(resourceType string) http.HandlerFunc {
	return func(w http.ResponseWriter, r *http.Request) {
		resourceID, ok := requireID(w, r, "id")
		if !ok {
			return
		}

		req, ok := decodeBody[MemberRequest](w, r)
		if !ok {
			return
		}
		if req.UserID == "" {
			httputil.WriteError(w, http.StatusBadRequest, "user_id is required")
			return
		}
		if req.Role == "" {
			req.Role = "member"
		}

		now := timeNow()

		tx, ok := a.beginTx(w, r)
		if !ok {
			return
		}
		defer tx.Rollback()

		_, err := tx.ExecContext(r.Context(),
			`INSERT OR REPLACE INTO memberships (resource_type, resource_id, user_id, role, added_at) VALUES (?, ?, ?, ?, ?)`,
			resourceType, resourceID, req.UserID, req.Role, now)
		if err != nil {
			httputil.WriteError(w, http.StatusConflict, "failed to add member")
			return
		}

		// FGA: membership tuple.
		// For org admin/owner grants, FGA is security-critical and must succeed.
		// For basic org member grants, skip FGA — SQL memberships table is
		// the canonical source for org filtering, and the FGA call can block
		// for extended periods on transient errors.
		if resourceType == "org" && (req.Role == "admin" || req.Role == "owner") {
			if fn := orgAdminFGA(); fn != nil {
				if !fgaSync(w, "add org admin", func(ctx context.Context, _ *fga.Service) error {
					return fn(ctx, req.UserID, resourceID)
				}, r.Context()) {
					return
				}
			}
		} else if resourceType != "org" {
			// For non-org resources (group, project), FGA is authoritative
			if fgaOps := memberFGAFor(resourceType); fgaOps != nil {
				if !fgaSync(w, "add "+resourceType+" member", func(ctx context.Context, _ *fga.Service) error {
					return fgaOps.Add(ctx, req.UserID, resourceID)
				}, r.Context()) {
					return
				}
			}
		}
		// org + member role: no FGA write needed (SQL is canonical)

		if !commitTx(w, tx) {
			return
		}

		a.bus.Signal()

		httputil.WriteJSON(w, http.StatusCreated, MemberResponse{
			UserID: req.UserID, Role: req.Role, AddedAt: now,
		})
	}
}

// removeMember returns a handler that removes a member from a given resource type.
func (a *API) removeMember(resourceType string) http.HandlerFunc {
	return func(w http.ResponseWriter, r *http.Request) {
		resourceID, ok := requireID(w, r, "id")
		if !ok {
			return
		}
		userID := r.PathValue("userId")
		if userID == "" {
			httputil.WriteError(w, http.StatusBadRequest, "userId is required")
			return
		}

		tx, ok := a.beginTx(w, r)
		if !ok {
			return
		}
		defer tx.Rollback()

		result, err := tx.ExecContext(r.Context(),
			`DELETE FROM memberships WHERE resource_type = ? AND resource_id = ? AND user_id = ?`,
			resourceType, resourceID, userID)
		if err != nil {
			httputil.WriteError(w, http.StatusInternalServerError, "delete failed")
			return
		}
		rowsAffected, _ := result.RowsAffected()
		if rowsAffected == 0 {
			httputil.WriteError(w, http.StatusNotFound, "member not found")
			return
		}

		// FGA: remove membership tuple — must succeed before commit (security-critical).
		if fgaOps := memberFGAFor(resourceType); fgaOps != nil {
			if !fgaSync(w, "remove "+resourceType+" member", func(ctx context.Context, _ *fga.Service) error {
				return fgaOps.Remove(ctx, userID, resourceID)
			}, r.Context()) {
				return
			}
		}

		if !commitTx(w, tx) {
			return
		}

		a.bus.Signal()
		w.WriteHeader(http.StatusNoContent)
	}
}
