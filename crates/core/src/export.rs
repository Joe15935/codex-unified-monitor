use crate::{account::Quota, analytics::Report};
use anyhow::Result;
use serde_json::{json, Value};
pub fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}
pub fn csv_cell(s: &str) -> String {
    let t = if s.starts_with(['=', '+', '-', '@', '\t', '\r']) {
        format!("'{s}")
    } else {
        s.into()
    };
    format!("\"{}\"", t.replace('"', "\"\""))
}
fn cells(data: &Value, quota: bool) -> Result<(Vec<String>, Vec<Vec<String>>)> {
    let array = data
        .as_array()
        .ok_or_else(|| anyhow::anyhow!("Choose a dataset for CSV export"))?;
    let head = if quota {
        "timestamp,source,status,5h_used_percent,5h_remaining_percent,5h_reset_at,weekly_used_percent,weekly_remaining_percent,weekly_reset_at,reset_credits"
    } else {
        "record,raw_input,cached_input,uncached_input,output,reasoning,total,cache_hit,public_api_equivalent_usd,public_api_unpriced_tokens,codex_work_equivalent_usd,metadata"
    };
    let mut rows = vec![];
    for (i, row) in array.iter().enumerate() {
        let string = |v: &Value| {
            if v.is_null() {
                String::new()
            } else if let Some(s) = v.as_str() {
                s.into()
            } else {
                v.to_string()
            }
        };
        if quota {
            rows.push(vec![
                string(&row["meta"]["updated_at"]),
                string(&row["meta"]["source"]),
                string(&row["meta"]["status"]),
                string(&row["five_hour"]["used_percent"]),
                string(&row["five_hour"]["remaining_percent"]),
                string(&row["five_hour"]["resets_at"]),
                string(&row["weekly"]["used_percent"]),
                string(&row["weekly"]["remaining_percent"]),
                string(&row["weekly"]["resets_at"]),
                string(&row["reset_credits"]["available_count"]),
            ]);
            continue;
        }
        let a = &row["aggregate"];
        let t = &a["tokens"];
        let n = |v: &Value| v.as_u64().unwrap_or(0);
        let raw = n(&t["raw_input_tokens"]);
        let cached = n(&t["cached_input_tokens"]);
        let id = row["label"]
            .as_str()
            .or(row["model"].as_str())
            .or(row["id"].as_str())
            .map(str::to_owned)
            .unwrap_or(i.to_string());
        let value = |mode: &str| {
            let m = &a[mode];
            if m.is_null() || (n(&m["priced_tokens"]) == 0 && n(&m["unpriced_tokens"]) > 0) {
                "UNPRICED".into()
            } else {
                format!("{:.6}", m["known_value_usd"].as_f64().unwrap_or(0.0))
            }
        };
        rows.push(vec![
            id,
            raw.to_string(),
            cached.to_string(),
            string(&t["uncached_input_tokens"]),
            string(&t["output_tokens"]),
            string(&t["reasoning_output_tokens"]),
            string(&t["total_tokens"]),
            if raw > 0 {
                format!("{:.6}", cached as f64 / raw as f64)
            } else {
                String::new()
            },
            value("public_api"),
            string(&a["public_api"]["unpriced_tokens"]),
            value("codex_work"),
            serde_json::to_string(row)?,
        ]);
    }
    Ok((head.split(',').map(str::to_owned).collect(), rows))
}
fn html_table(data: &Value, quota: bool, language: &str) -> Result<String> {
    let (head, rows) = cells(data, quota)?;
    // Full metadata remains available in JSON/CSV; the printable report focuses
    // on comparable counters and values instead of a wall of JSON.
    let keep = head.len() - if quota { 0 } else { 1 };
    let mut out = String::from("<div class=scroll><table><thead><tr>");
    for h in &head[..keep] {
        out.push_str(&format!(
            "<th>{}</th>",
            escape(crate::locale::text(language, &h.replace('_', " ")))
        ));
    }
    out.push_str("</tr></thead><tbody>");
    for row in rows {
        out.push_str("<tr>");
        for (index, c) in row[..keep].iter().enumerate() {
            let display = if (quota && index == 2) || c == "UNPRICED" {
                crate::locale::text(language, c)
            } else {
                c
            };
            out.push_str(&format!("<td>{}</td>", escape(display)));
        }
        out.push_str("</tr>");
    }
    out.push_str("</tbody></table></div>");
    Ok(out)
}
pub fn render(report: &Report, quota: &[Quota], format: &str, dataset: &str) -> Result<String> {
    render_localized(report, quota, format, dataset, "en")
}
pub fn render_localized(
    report: &Report,
    quota: &[Quota],
    format: &str,
    dataset: &str,
    language: &str,
) -> Result<String> {
    let tr = |source: &str| escape(crate::locale::text(language, source));
    let data = match dataset {
        "daily" => serde_json::to_value(&report.trend)?,
        "models" => serde_json::to_value(&report.models)?,
        "sessions" => serde_json::to_value(&report.sessions)?,
        "quota" => serde_json::to_value(quota)?,
        "all" => json!({"usage":report,"quota_history":quota}),
        _ => anyhow::bail!("Unknown export dataset"),
    };
    if format == "json" {
        return Ok(serde_json::to_string_pretty(&data)?);
    }
    if format == "html" {
        let mut body = String::new();
        if dataset == "all" {
            for (title, value, is_quota) in [
                (
                    "Usage over time",
                    serde_json::to_value(&report.trend)?,
                    false,
                ),
                ("Models", serde_json::to_value(&report.models)?, false),
                ("Sessions", serde_json::to_value(&report.sessions)?, false),
                ("Quota history", serde_json::to_value(quota)?, true),
            ] {
                body.push_str(&format!(
                    "<h2>{}</h2>{}",
                    tr(title),
                    html_table(&value, is_quota, language)?
                ));
            }
        } else {
            body = html_table(&data, dataset == "quota", language)?;
        }
        let lang = if language == "zh-CN" { "zh-CN" } else { "en" };
        return Ok(format!(r#"<!doctype html><html lang="{lang}"><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><meta http-equiv="Content-Security-Policy" content="default-src 'none'; style-src 'unsafe-inline'"><title>{title}</title><style>body{{font:14px system-ui;max-width:1400px;margin:40px auto;padding:24px;color:#183630;background:#f6f7f2}}.scroll{{overflow:auto;background:white;border:1px solid #d8e1db;border-radius:12px}}table{{border-collapse:collapse;width:100%}}th,td{{padding:12px;text-align:right;border-bottom:1px solid #e4e9e3;font-variant-numeric:tabular-nums}}th{{background:#e7efe2;font-size:12px;text-transform:capitalize}}th:first-child,td:first-child{{text-align:left}}h1{{font-weight:600}}p{{line-height:1.7;max-width:1000px}}.summary{{font-size:20px}}@media print{{body{{margin:0;padding:0;background:white;font-size:9px}}th,td{{padding:4px}}.scroll{{overflow:visible}}}}</style><h1>Codex Unified Monitor</h1><p>{metadata} · {timezone} · {period} · {dataset}</p><p class=summary>{total_label}: {total} · {cached_label}: {cached} · {output_label}: {output}</p><p>{boundary}</p>{body}</html>"#,
            title=tr("Codex Unified Monitor report"), metadata=tr("Local metadata"), timezone=escape(&report.timezone), period=tr(&report.range.period), dataset=tr(dataset),
            total_label=tr("Total tokens"), total=report.total.tokens.total_tokens, cached_label=tr("Cached input"), cached=report.total.tokens.cached_input_tokens, output_label=tr("Output"), output=report.total.tokens.output_tokens,
            boundary=tr("API equivalent values are theoretical base-rate estimates in USD, not subscription bills. Unknown models remain unpriced. Long-context, cache-write, service-tier and request-level adjustments are not reconstructed. Quota timestamps are Unix seconds; blank quota fields are unavailable."),
        ));
    }
    anyhow::ensure!(format == "csv", "Choose CSV, JSON or HTML");
    let (head, rows) = cells(&data, dataset == "quota")?;
    let mut out = head.join(",") + "\n";
    for row in rows {
        out.push_str(
            &row.iter()
                .map(|c| csv_cell(c))
                .collect::<Vec<_>>()
                .join(","),
        );
        out.push('\n');
    }
    Ok(out)
}
