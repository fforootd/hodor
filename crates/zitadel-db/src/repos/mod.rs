pub mod actions {
    pub use crate::{
        ActionRecord, get_action, list_actions, upsert_catalog_action,
    };
}

pub mod analytics {
    pub use crate::{
        ConsoleBootstrapData, SavedQueryRecord, create_saved_query, delete_saved_query,
        list_saved_queries, load_console_bootstrap_data, load_entity_counts,
    };
}

pub mod auth {
    pub use crate::{
        IdentityMetadata, UserClaimsRecord, load_identity_metadata, load_session_user_profile,
        load_user_claims_record, user_has_capability,
    };
}

pub mod domains {
    pub use crate::{
        DomainDeleteOutcome, DomainRecord, add_instance_domain, delete_instance_domain,
        list_instance_domains,
    };
}

pub mod events {
    pub use crate::append_event;
}

pub mod fingerprints {
    pub use crate::{FingerprintRecord, list_fingerprints, upsert_fingerprint};
}

pub mod instances {
    pub use crate::{
        CreateManagedInstanceInput, InstanceMetadata, ManagedInstancePatch, ManagedInstanceRecord,
        create_managed_instance, deprovision_managed_instance, get_managed_instance,
        instance_visible, list_admin_instances, list_managed_instances, load_instance_metadata,
        update_managed_instance,
    };
}

pub mod jobs {
    pub use crate::{JobRecord, list_jobs_for_instance};
}

pub mod linked_identities {
    pub use crate::{
        LinkedIdentityRecord, create_linked_identity_record, find_linked_identity,
        touch_linked_identity,
    };
}

pub mod login_flows {
    pub use crate::{
        LoginFlowRecord, create_login_flow, get_login_flow_record, list_login_flow_records,
        resolve_login_flow, set_login_flow_state, update_login_flow,
    };
}

pub mod oidc {
    pub use crate::{
        OidcAuthRequestRecord, OidcClientRecord, create_oidc_auth_request_record,
        consume_oidc_auth_code_record, get_oidc_client_record,
    };
}

pub mod orgs {
    pub use crate::{
        OrgRecord, OrgSummary, create_org, first_org_id, get_org, list_org_records, update_org,
    };
}

pub mod pats {
    pub use crate::{PatRecord, create_pat, list_pats_for_instance, revoke_pat};
}

pub mod providers {
    pub use crate::delete_provider;
    pub use crate::provider::{
        ProviderCatalogRef, ProviderConnection, ProviderLinking, ProviderLinkingMode,
        ProviderMapping, ProviderMatchBy, ProviderPayload, ProviderRecord, ProviderTarget,
        ProviderUi, get_provider_for, insert_provider_for, list_providers_for, update_provider_for,
    };
}

pub mod routing {
    pub use crate::{RouteResolutionRecord, resolve_domain_route, resolve_instance_route};
}

pub mod schemas {
    pub use crate::{
        SchemaRegistryRecord, count_users_for_schema, create_schema_record, get_schema_record,
        list_schema_registry, promote_schema_record, update_schema_record,
    };
}

pub mod search {
    pub use crate::{SearchRecord, search_records};
}

pub mod settings {
    pub use crate::{SettingsRecord, delete_settings_record, get_settings_record, put_instance_settings};
}

pub mod sessions {
    pub use crate::update_session_metadata;
}

pub mod users {
    pub use crate::{
        UserRecord, create_user, find_active_user_by_identifier, get_user, list_users,
        replace_password_credential, update_password_hash, update_user,
    };
}
