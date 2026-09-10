use codexmeter_core::{
    account, analytics, auditor, export, locale, pricing::Catalog, settings::Settings,
    storage::Store,
};
use serde_json::json;

#[test]
fn old_preferences_gain_a_language_without_losing_existing_values() {
    let mut settings: Settings = serde_json::from_value(json!({
        "timezone": "Asia/Shanghai", "quota_poll_seconds": 300,
        "theme": "dark", "aliases": {"my-model": "gpt-5.5"}, "account_enabled": false
    }))
    .unwrap();
    assert_eq!(settings.language, "zh-CN");
    let before = serde_json::to_value(&settings).unwrap();
    let directory = tempfile::tempdir().unwrap();
    let store = Store::open(&directory.path().join("db")).unwrap();
    settings.language = "en".into();
    settings.save(&store).unwrap();
    let mut after = serde_json::to_value(Settings::load(&store).unwrap()).unwrap();
    assert_eq!(after["language"], "en");
    after["language"] = before["language"].clone();
    assert_eq!(before, after);
    settings.language = "unsupported".into();
    assert!(settings.save(&store).is_err());
    assert_eq!(Settings::load(&store).unwrap().language, "en");
}

#[test]
fn bilingual_usage_html_preserves_numbers_escaping_and_machine_exports() {
    let directory = tempfile::tempdir().unwrap();
    let store = Store::open(&directory.path().join("db")).unwrap();
    let report = analytics::report(
        &store,
        &Catalog::bundled(),
        &Settings::default(),
        analytics::Range::default(),
        1_800_000_000,
    )
    .unwrap();
    let mut quota = account::normalize(&json!({"rateLimits":{"primary":{"usedPercent":25,"windowDurationMins":10080,"resetsAt":1_900_000_000}}}),123).unwrap();
    quota.meta.source = "<img src=x onerror=alert(1)>".into();
    let en = export::render_localized(&report, &[quota.clone()], "html", "quota", "en").unwrap();
    let zh = export::render_localized(&report, &[quota.clone()], "html", "quota", "zh-CN").unwrap();
    assert!(en.contains("lang=\"en\"") && en.contains("weekly used percent"));
    assert!(zh.contains("lang=\"zh-CN\"") && zh.contains("周额度已用百分比"));
    for html in [&en, &zh] {
        assert!(html.contains("<td>25.0</td>") && html.contains("<td>75.0</td>"));
        assert!(html.contains("&lt;img") && !html.contains("<img"));
        assert!(!html.contains("<script") && !html.contains("<link"));
    }
    for format in ["csv", "json"] {
        assert_eq!(
            export::render_localized(&report, &[quota.clone()], format, "quota", "en").unwrap(),
            export::render_localized(&report, &[quota.clone()], format, "quota", "zh-CN").unwrap()
        );
    }
}

#[test]
fn auditor_translation_does_not_change_signed_evidence_or_protocol_values() {
    let directory = tempfile::tempdir().unwrap();
    let store = Store::open(&directory.path().join("db")).unwrap();
    let quota = account::normalize(&json!({"rateLimits":{}}), 1_800_000_000).unwrap();
    let evidence = auditor::report(
        &store,
        &Catalog::bundled(),
        &Settings::default(),
        &quota,
        1_800_000_000,
    )
    .unwrap()
    .evidence;
    let before = auditor::envelope(&evidence).unwrap();
    let english = auditor::render_localized(&evidence, "html", "en").unwrap();
    let chinese = auditor::render_localized(&evidence, "html", "zh-CN").unwrap();
    assert!(
        english.contains("INCONCLUSIVE")
            && english.contains("No independent Pro 5x baseline imported")
    );
    assert!(
        chinese.contains("证据不足（INCONCLUSIVE）")
            && chinese.contains("尚未导入独立的 Pro 5x 基准")
    );
    assert!(chinese.contains("不能证明后台权益配置错误"));
    for format in ["json", "csv"] {
        assert_eq!(
            auditor::render_localized(&evidence, format, "en").unwrap(),
            auditor::render_localized(&evidence, format, "zh-CN").unwrap()
        );
    }
    assert_eq!(before, auditor::envelope(&evidence).unwrap());
    let appendix = locale::display_json(
        "zh-CN",
        &json!({"model":"Output", "rates":{"Input":[1,2,3]}, "expected_tier":"pro_20x"}),
    );
    assert_eq!(appendix["模型"], "Output");
    assert_eq!(
        appendix["每百万 Token 的价格：输入、缓存、输出"]["Input"],
        json!([1, 2, 3])
    );
}
