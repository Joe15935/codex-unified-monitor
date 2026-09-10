//! Presentation-only translations shared with the React UI. Evidence JSON stays canonical.
use std::{collections::BTreeMap, sync::LazyLock};

static CHINESE: LazyLock<BTreeMap<String, String>> = LazyLock::new(|| {
    serde_json::from_str(include_str!("../../../locales/zh-CN.json"))
        .expect("bundled translation catalog must be valid")
});

pub fn text<'a>(language: &str, source: &'a str) -> &'a str {
    if language == "zh-CN" {
        CHINESE.get(source).map(String::as_str).unwrap_or(source)
    } else {
        source
    }
}

/// Translate keys in the human-readable HTML appendix, never the signed evidence envelope.
pub fn display_json(language: &str, value: &serde_json::Value) -> serde_json::Value {
    use serde_json::Value;
    match value {
        Value::Object(fields) => Value::Object(
            fields
                .iter()
                .map(|(key, value)| {
                    (
                        text(language, key).to_string(),
                        if matches!(
                            key.as_str(),
                            "rates" | "model" | "sources" | "baseline_source" | "fingerprint"
                        ) {
                            value.clone()
                        } else {
                            display_json(language, value)
                        },
                    )
                })
                .collect(),
        ),
        Value::Array(values) => {
            Value::Array(values.iter().map(|v| display_json(language, v)).collect())
        }
        Value::String(value) => Value::String(text(language, value).to_string()),
        _ => value.clone(),
    }
}
