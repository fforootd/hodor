const SPANNER_BASELINE: &str = include_str!("../../../migrations/spanner/00001_baseline.sql");
const SPANNER_INITIAL: &str = include_str!("../../../migrations/spanner/00001_initial.sql");
const SPANNER_EFFECTS: &str = include_str!("../../../migrations/spanner/00007_effects.sql");

fn table_block<'a>(sql: &'a str, table_name: &str) -> &'a str {
    let marker = format!("CREATE TABLE IF NOT EXISTS {table_name}");
    let start = sql
        .find(&marker)
        .unwrap_or_else(|| panic!("missing Spanner table definition for {table_name}"));
    let rest = &sql[start..];
    let end = rest
        .find(";\n")
        .unwrap_or_else(|| panic!("missing statement terminator for {table_name}"));
    &rest[..end]
}

#[test]
fn hot_auth_and_observability_tables_are_not_interleaved_in_spanner_migrations() {
    let all_spanner_sql = [SPANNER_BASELINE, SPANNER_INITIAL, SPANNER_EFFECTS].join("\n");
    for table_name in ["events", "sessions", "tokens", "effects"] {
        let block = table_block(SPANNER_BASELINE, table_name);
        assert!(
            !block.contains("INTERLEAVE IN PARENT"),
            "{table_name} should remain a top-level table in the Spanner baseline",
        );
    }
    assert!(
        !all_spanner_sql.contains("INTERLEAVE IN PARENT"),
        "the current Spanner migration set should not silently introduce table interleaving",
    );
}

#[test]
fn events_and_sessions_keep_instance_prefixed_primary_keys() {
    for table_name in ["events", "sessions"] {
        let block = table_block(SPANNER_BASELINE, table_name);
        assert!(
            block.contains("PRIMARY KEY (instance_id, id)"),
            "{table_name} should keep PRIMARY KEY(instance_id, id) in the Spanner baseline",
        );
    }
}

#[test]
fn observability_auth_and_effect_lookup_indexes_remain_present() {
    for index_name in [
        "idx_events_instance_created",
        "idx_events_instance_type_created",
        "idx_events_org",
        "idx_sessions_instance_user",
        "idx_sessions_instance_token_unique",
        "idx_tokens_instance_session",
        "idx_effects_due",
        "idx_effects_event",
    ] {
        assert!(
            SPANNER_BASELINE.contains(index_name),
            "Spanner baseline should keep the {index_name} secondary index",
        );
    }

    assert!(
        SPANNER_BASELINE.contains("CREATE UNIQUE INDEX IF NOT EXISTS")
            && SPANNER_BASELINE.contains("ON tokens(instance_id, token_hash)"),
        "Spanner baseline should keep a unique secondary index over tokens(instance_id, token_hash)",
    );
}
