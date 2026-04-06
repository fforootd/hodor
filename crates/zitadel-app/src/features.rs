use std::collections::BTreeMap;

use serde_json::Value;

pub type FeatureMap = BTreeMap<String, bool>;

pub fn validate_feature_overrides(overrides: &Value, allowed: &[&str]) -> anyhow::Result<()> {
    let Some(obj) = overrides.as_object() else {
        anyhow::bail!("feature overrides must be a JSON object");
    };

    for (key, value) in obj {
        if !allowed.iter().any(|allowed_key| allowed_key == key) {
            anyhow::bail!("unknown feature override: {key}");
        }
        if !value.is_boolean() {
            anyhow::bail!("feature override values must be boolean");
        }
    }

    Ok(())
}

pub fn merge_feature_overrides(
    defaults: impl IntoIterator<Item = (impl Into<String>, bool)>,
    overrides: &Value,
    allowed: &[&str],
) -> anyhow::Result<FeatureMap> {
    validate_feature_overrides(overrides, allowed)?;

    let mut merged = defaults
        .into_iter()
        .map(|(key, value)| (key.into(), value))
        .collect::<FeatureMap>();

    if let Some(obj) = overrides.as_object() {
        for (key, value) in obj {
            if let Some(enabled) = value.as_bool() {
                merged.insert(key.clone(), enabled);
            }
        }
    }

    Ok(merged)
}

pub fn feature_enabled(
    defaults: &FeatureMap,
    overrides: &Value,
    allowed: &[&str],
    key: &str,
) -> anyhow::Result<bool> {
    validate_feature_overrides(overrides, allowed)?;
    if let Some(enabled) = overrides.get(key).and_then(Value::as_bool) {
        return Ok(enabled);
    }
    Ok(*defaults.get(key).unwrap_or(&false))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn missing_key_inherits_default() {
        let defaults = FeatureMap::from([
            ("instance_management".into(), true),
            ("beta_console".into(), false),
        ]);

        assert!(
            feature_enabled(
                &defaults,
                &json!({}),
                &["instance_management", "beta_console"],
                "instance_management"
            )
            .unwrap()
        );
    }

    #[test]
    fn override_can_force_enable_or_disable() {
        let defaults = FeatureMap::from([("instance_management".into(), false)]);

        assert!(
            feature_enabled(
                &defaults,
                &json!({"instance_management": true}),
                &["instance_management"],
                "instance_management"
            )
            .unwrap()
        );
        assert!(
            !feature_enabled(
                &defaults,
                &json!({"instance_management": false}),
                &["instance_management"],
                "instance_management"
            )
            .unwrap()
        );
    }

    #[test]
    fn unknown_keys_are_rejected() {
        let error = validate_feature_overrides(&json!({"unknown_flag": true}), &["known_flag"])
            .err()
            .unwrap();
        assert!(error.to_string().contains("unknown feature override"));
    }
}
