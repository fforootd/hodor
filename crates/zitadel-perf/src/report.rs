use std::{
    cmp::Ordering,
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

use anyhow::Context;
use serde::{Deserialize, Serialize};

use crate::PerfBackend;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StorageRolesSnapshot {
    pub stateful: String,
    pub read: String,
    pub kv: String,
    pub sink: String,
    pub process_cache: String,
    pub analytics: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DatasetProfile {
    pub name: String,
    pub users: u32,
    pub active_sessions: u32,
    pub revoked_sessions: u32,
    pub expired_sessions: u32,
    pub fga_tuples: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ScenarioReport {
    pub scenario: String,
    pub dataset_profile: String,
    pub warmup_rounds: u32,
    pub measured_rounds: u32,
    pub total_operations: u64,
    pub p50_ms: f64,
    pub p95_ms: f64,
    pub max_ms: f64,
    pub ops_per_sec: f64,
    pub error_count: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DbPerfReport {
    pub generated_at_epoch_secs: u64,
    pub backend: PerfBackend,
    pub profile: String,
    pub storage_roles: StorageRolesSnapshot,
    pub dataset: DatasetProfile,
    pub scenarios: Vec<ScenarioReport>,
}

pub fn write_report(path: &Path, report: &DbPerfReport) -> anyhow::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("create report parent {}", parent.display()))?;
    }
    let json = serde_json::to_string_pretty(report).context("serialize perf report")?;
    fs::write(path, json).with_context(|| format!("write perf report {}", path.display()))?;
    Ok(())
}

pub fn load_report(path: &Path) -> anyhow::Result<DbPerfReport> {
    let raw =
        fs::read_to_string(path).with_context(|| format!("read perf report {}", path.display()))?;
    serde_json::from_str(&raw).with_context(|| format!("parse perf report {}", path.display()))
}

pub fn load_reports(paths: &[PathBuf]) -> anyhow::Result<Vec<DbPerfReport>> {
    paths.iter().map(|path| load_report(path)).collect()
}

pub fn render_markdown_summary(current: &[DbPerfReport], previous: &[DbPerfReport]) -> String {
    let mut output = String::new();
    output.push_str("# DB Performance Summary\n\n");

    if current.is_empty() {
        output.push_str("No current benchmark reports were provided.\n");
        return output;
    }

    let mut current_reports = current.to_vec();
    current_reports.sort_by_key(|report| report.backend.as_str().to_string());

    let previous_by_key = previous
        .iter()
        .flat_map(|report| {
            report.scenarios.iter().map(move |scenario| {
                (
                    (
                        report.backend.as_str().to_string(),
                        scenario.scenario.clone(),
                    ),
                    scenario,
                )
            })
        })
        .collect::<BTreeMap<_, _>>();

    for report in &current_reports {
        output.push_str(&format!("## {}\n\n", report.backend.display_name()));
        output.push_str(&format!(
            "- Profile: `{}`\n- Storage roles: `stateful={}`, `read={}`, `kv={}`, `sink={}`, `process_cache={}`, `analytics={}`\n- Dataset: `{}` users, `{}` active sessions, `{}` revoked sessions, `{}` expired sessions, `{}` FGA tuples\n\n",
            report.profile,
            report.storage_roles.stateful,
            report.storage_roles.read,
            report.storage_roles.kv,
            report.storage_roles.sink,
            report.storage_roles.process_cache,
            report.storage_roles.analytics,
            report.dataset.users,
            report.dataset.active_sessions,
            report.dataset.revoked_sessions,
            report.dataset.expired_sessions,
            report.dataset.fga_tuples,
        ));
        output.push_str(
            "| Scenario | P50 ms | P95 ms | Max ms | Ops/s | Errors | Δ P95 | Δ Ops/s |\n",
        );
        output.push_str("| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |\n");

        let mut scenarios = report.scenarios.clone();
        scenarios.sort_by(|left, right| left.scenario.cmp(&right.scenario));
        for scenario in scenarios {
            let previous = previous_by_key
                .get(&(
                    report.backend.as_str().to_string(),
                    scenario.scenario.clone(),
                ))
                .copied();
            output.push_str(&format!(
                "| `{}` | {:.2} | {:.2} | {:.2} | {:.2} | {} | {} | {} |\n",
                scenario.scenario,
                scenario.p50_ms,
                scenario.p95_ms,
                scenario.max_ms,
                scenario.ops_per_sec,
                scenario.error_count,
                format_delta_percent(previous.map(|value| value.p95_ms), scenario.p95_ms),
                format_delta_percent(
                    previous.map(|value| value.ops_per_sec),
                    scenario.ops_per_sec
                ),
            ));
        }
        append_postgres_fga_focus(&mut output, report, &previous_by_key);
        output.push('\n');
    }

    output
}

fn append_postgres_fga_focus(
    output: &mut String,
    report: &DbPerfReport,
    previous_by_key: &BTreeMap<(String, String), &ScenarioReport>,
) {
    if report.backend != PerfBackend::Postgres {
        return;
    }

    let mut fga_scenarios = report
        .scenarios
        .iter()
        .filter(|scenario| is_fga_scenario(&scenario.scenario))
        .collect::<Vec<_>>();
    if fga_scenarios.is_empty() {
        return;
    }

    fga_scenarios.sort_by(|left, right| {
        right
            .p95_ms
            .partial_cmp(&left.p95_ms)
            .unwrap_or(Ordering::Equal)
    });

    output.push_str("### Postgres FGA Focus\n\n");
    for scenario in fga_scenarios.iter().take(5) {
        let previous = previous_by_key
            .get(&(
                report.backend.as_str().to_string(),
                scenario.scenario.clone(),
            ))
            .copied();
        let p95_delta = delta_percent(previous.map(|value| value.p95_ms), scenario.p95_ms);
        let ops_delta = delta_percent(
            previous.map(|value| value.ops_per_sec),
            scenario.ops_per_sec,
        );
        let marker = if is_large_regression(p95_delta, ops_delta) {
            " [large regression]"
        } else {
            ""
        };
        output.push_str(&format!(
            "- `{}`: P95 {:.2} ms, {:.2} ops/s{}{}\n",
            scenario.scenario,
            scenario.p95_ms,
            scenario.ops_per_sec,
            format_delta_summary(p95_delta, ops_delta),
            marker,
        ));
    }
    output.push('\n');
}

fn is_fga_scenario(scenario: &str) -> bool {
    scenario.starts_with("fga_") || scenario.starts_with("http_fga_")
}

fn delta_percent(previous: Option<f64>, current: f64) -> Option<f64> {
    let previous = previous?;
    if previous.abs() < f64::EPSILON {
        return None;
    }
    Some(((current - previous) / previous) * 100.0)
}

fn is_large_regression(p95_delta: Option<f64>, ops_delta: Option<f64>) -> bool {
    p95_delta.is_some_and(|delta| delta >= 50.0) || ops_delta.is_some_and(|delta| delta <= -25.0)
}

fn format_delta_summary(p95_delta: Option<f64>, ops_delta: Option<f64>) -> String {
    match (p95_delta, ops_delta) {
        (None, None) => String::new(),
        (p95, ops) => format!(
            " (Δ P95 {}, Δ Ops/s {})",
            p95.map_or_else(|| "n/a".into(), |delta| format!("{delta:+.1}%")),
            ops.map_or_else(|| "n/a".into(), |delta| format!("{delta:+.1}%"))
        ),
    }
}

fn format_delta_percent(previous: Option<f64>, current: f64) -> String {
    let Some(delta) = delta_percent(previous, current) else {
        return "n/a".into();
    };
    format!("{delta:+.1}%")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_report(
        backend: PerfBackend,
        scenario: &str,
        p95_ms: f64,
        ops_per_sec: f64,
    ) -> DbPerfReport {
        DbPerfReport {
            generated_at_epoch_secs: 1,
            backend,
            profile: "ci".into(),
            storage_roles: StorageRolesSnapshot {
                stateful: backend.as_str().into(),
                read: "same".into(),
                kv: "memory".into(),
                sink: "channel".into(),
                process_cache: "memory".into(),
                analytics: "same_stateful".into(),
            },
            dataset: DatasetProfile {
                name: "ci".into(),
                users: 10,
                active_sessions: 20,
                revoked_sessions: 10,
                expired_sessions: 10,
                fga_tuples: 50,
            },
            scenarios: vec![ScenarioReport {
                scenario: scenario.into(),
                dataset_profile: "ci".into(),
                warmup_rounds: 1,
                measured_rounds: 2,
                total_operations: 2,
                p50_ms: p95_ms / 2.0,
                p95_ms,
                max_ms: p95_ms,
                ops_per_sec,
                error_count: 0,
            }],
        }
    }

    #[test]
    fn markdown_summary_includes_deltas() {
        let current = vec![sample_report(
            PerfBackend::Sqlite,
            "session_lookup_hit",
            4.0,
            100.0,
        )];
        let previous = vec![sample_report(
            PerfBackend::Sqlite,
            "session_lookup_hit",
            2.0,
            80.0,
        )];

        let markdown = render_markdown_summary(&current, &previous);
        assert!(markdown.contains("session_lookup_hit"));
        assert!(markdown.contains("+100.0%"));
        assert!(markdown.contains("+25.0%"));
    }
}
