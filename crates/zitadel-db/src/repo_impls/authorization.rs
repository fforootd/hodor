use crate::{
    Db, create_role_assignment, get_instance_trust_link, get_role_assignment,
    list_role_assignments, list_role_definitions, revoke_role_assignment,
};
use zitadel_app::repo::{
    AuthorizationRepository, BoxFuture, InstanceTrustLinkRecord, RoleAssignmentFilter,
    RoleAssignmentRecord,
};
use zitadel_authz::RoleDefinition;

#[derive(Clone)]
pub struct DbAuthorizationRepository {
    db: Db,
}

impl DbAuthorizationRepository {
    pub fn new(db: Db) -> Self {
        Self { db }
    }
}

impl AuthorizationRepository for DbAuthorizationRepository {
    fn list_role_definitions(&self) -> BoxFuture<'_, anyhow::Result<Vec<RoleDefinition>>> {
        let db = self.db.clone();
        Box::pin(async move { list_role_definitions(&db).await })
    }

    fn create_role_assignment(
        &self,
        assignment: &RoleAssignmentRecord,
    ) -> BoxFuture<'_, anyhow::Result<RoleAssignmentRecord>> {
        let db = self.db.clone();
        let assignment = assignment.clone();
        Box::pin(async move { create_role_assignment(&db, &assignment).await })
    }

    fn get_role_assignment(
        &self,
        assignment_id: &str,
    ) -> BoxFuture<'_, anyhow::Result<Option<RoleAssignmentRecord>>> {
        let db = self.db.clone();
        let assignment_id = assignment_id.to_string();
        Box::pin(async move { get_role_assignment(&db, &assignment_id).await })
    }

    fn list_role_assignments(
        &self,
        filter: &RoleAssignmentFilter,
    ) -> BoxFuture<'_, anyhow::Result<Vec<RoleAssignmentRecord>>> {
        let db = self.db.clone();
        let filter = filter.clone();
        Box::pin(async move { list_role_assignments(&db, &filter).await })
    }

    fn revoke_role_assignment(
        &self,
        assignment_id: &str,
        revoked_at: &str,
    ) -> BoxFuture<'_, anyhow::Result<bool>> {
        let db = self.db.clone();
        let assignment_id = assignment_id.to_string();
        let revoked_at = revoked_at.to_string();
        Box::pin(async move { revoke_role_assignment(&db, &assignment_id, &revoked_at).await })
    }

    fn get_instance_trust_link(
        &self,
        child_instance_id: &str,
        issuer: &str,
        audience: &str,
    ) -> BoxFuture<'_, anyhow::Result<Option<InstanceTrustLinkRecord>>> {
        let db = self.db.clone();
        let child_instance_id = child_instance_id.to_string();
        let issuer = issuer.to_string();
        let audience = audience.to_string();
        Box::pin(async move {
            get_instance_trust_link(&db, &child_instance_id, &issuer, &audience).await
        })
    }
}
