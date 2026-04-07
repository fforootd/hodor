use std::sync::Arc;

use tokio::{sync::Barrier, task::JoinSet};
use zitadel_app::{
    effect::{Effect, EffectType},
    repo::EffectRepository,
};
use zitadel_db::{
    DEFAULT_INSTANCE_ID, JobReconcileSpec, complete_job_run, due_job_names, migrate,
    reconcile_jobs, test_support::spanner_db_from_env, try_acquire_job_lease,
};

fn sample_effect(source_key: &str) -> Effect {
    Effect::new(
        format!("evt-{source_key}"),
        source_key.to_string(),
        EffectType::Webhook,
        serde_json::json!({ "url": "https://example.com/webhook" }),
        serde_json::json!({ "kind": source_key }),
    )
}

#[tokio::test]
async fn effect_repository_handles_claim_retry_complete_and_cleanup_on_spanner_when_configured()
-> anyhow::Result<()> {
    let Some(db) = spanner_db_from_env("db-effects").await? else {
        return Ok(());
    };
    migrate::migrate(&db).await?;

    let repo = zitadel_db::repos::adapters::DbEffectRepository::new(db.clone());
    repo.enqueue_batch(
        DEFAULT_INSTANCE_ID,
        &[sample_effect("spanner-a"), sample_effect("spanner-b")],
    )
    .await?;

    let claimed = repo
        .claim_due(DEFAULT_INSTANCE_ID, "worker-a", 30, 2)
        .await?;
    assert_eq!(claimed.len(), 2);

    let retry_at = "1970-01-01T00:00:00Z";
    repo.record_failure(DEFAULT_INSTANCE_ID, &claimed[0].id, "boom", retry_at)
        .await?;
    let retried = repo
        .claim_due(DEFAULT_INSTANCE_ID, "worker-b", 30, 1)
        .await?;
    assert_eq!(retried.len(), 1);
    assert_eq!(retried[0].id, claimed[0].id);

    repo.mark_completed(DEFAULT_INSTANCE_ID, &retried[0].id)
        .await?;
    repo.mark_dead(DEFAULT_INSTANCE_ID, &claimed[1].id, "exhausted")
        .await?;

    let removed = repo
        .cleanup(DEFAULT_INSTANCE_ID, "9999-12-31T00:00:00Z", 10)
        .await?;
    assert_eq!(removed, 2);

    Ok(())
}

#[tokio::test]
async fn job_leases_are_exclusive_on_spanner_when_configured() -> anyhow::Result<()> {
    let Some(db) = spanner_db_from_env("db-jobs").await? else {
        return Ok(());
    };
    migrate::migrate(&db).await?;

    reconcile_jobs(
        &db,
        DEFAULT_INSTANCE_ID,
        &[JobReconcileSpec {
            name: "effects_gc".into(),
            display_name: "Effects GC".into(),
            description: "Delete durable effect rows".into(),
            cron: "* * * * *".into(),
            cadence_secs: 0,
            strategy: "retention".into(),
            targets: vec!["effects".into()],
            retention: "7d".into(),
        }],
    )
    .await?;

    let due = due_job_names(&db, DEFAULT_INSTANCE_ID, &["effects_gc"]).await?;
    assert_eq!(due, vec!["effects_gc".to_string()]);

    let barrier = Arc::new(Barrier::new(2));
    let mut tasks = JoinSet::new();
    for worker in ["worker-a", "worker-b"] {
        let db = db.clone();
        let barrier = barrier.clone();
        let worker = worker.to_string();
        tasks.spawn(async move {
            barrier.wait().await;
            let acquired =
                try_acquire_job_lease(&db, DEFAULT_INSTANCE_ID, "effects_gc", &worker, 30).await?;
            Ok::<(String, bool), anyhow::Error>((worker, acquired))
        });
    }

    let mut winners = Vec::new();
    while let Some(result) = tasks.join_next().await {
        let (worker, acquired) = result??;
        if acquired {
            winners.push(worker);
        }
    }

    assert_eq!(winners.len(), 1, "only one worker should win the job lease");
    complete_job_run(
        &db,
        DEFAULT_INSTANCE_ID,
        "effects_gc",
        &winners[0],
        0,
        "ok",
        "",
        0,
    )
    .await?;

    Ok(())
}
