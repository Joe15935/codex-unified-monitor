import { t } from "../i18n";
import { useCallback, useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { call, count, date, duration, exact, native, usd } from "../data";
import type { Data, Tokens } from "../types";

interface Controls {
  expected_tier: string;
  model: string;
  effort: string;
  context_band: string;
  workload_class: string;
  fast_off_attested: boolean;
  no_subagents_attested: boolean;
  exclusive_local_use_attested: boolean;
}
interface Interval {
  start: number;
  end: number;
  reset_at: number | null;
  weekly_before: number | null;
  weekly_after: number | null;
  delta_quota: number | null;
  tokens: Tokens;
  api_equivalent: number;
  unpriced_tokens: number;
  usd_per_percent: number | null;
  responses: number;
  workloads: string[];
  tool_calls: number;
  models: string[];
  efforts: string[];
  input_min: number;
  input_max: number;
  fast_evidence: string;
  subagent_evidence: string;
  eligible: boolean;
  reasons: string[];
}
interface Summary {
  samples: number;
  workloads: number;
  observation_seconds: number;
  quota_change: number;
  tokens: Tokens;
  responses: number;
  tool_calls: number;
  api_equivalent: number;
  usd_per_percent: number | null;
  dispersion: number | null;
  coefficient_of_variation: number | null;
  capacity_per_week: number | null;
  capacity_range: number[] | null;
  quality: string;
  reasons: string[];
}
interface Evidence {
  reported_plan: string;
  controls: Controls | null;
  started_at: number | null;
  ended_at: number | null;
  intervals: Interval[];
  summary: Summary;
  assessment: string;
  confidence: string;
  relative_index: number | null;
  relative_range: number[] | null;
  baseline_capacity: number | null;
  baseline_source: string | null;
  assessment_reasons: string[];
  boundary: string;
  methodology: string[];
  pricing: { verified_at: string; fingerprint: string; sources: string[] };
}
interface AuditView {
  evidence: Evidence;
  running: boolean;
  active_elsewhere: boolean;
  baseline_loaded: boolean;
  quota_status: string;
}
const tierName = (s?: string) =>
  s === "pro_20x" ? "Pro 20x" : s === "pro_5x" ? "Pro 5x" : "Not configured";
const pp = (n: number | null) =>
  n == null ? "—" : `${n > 0 ? "+" : ""}${n.toFixed(1)} ${t("pp")}`;

export default function TierAuditor({
  data,
  onToast,
}: {
  data: Data;
  onToast: (s: string) => void;
}) {
  const [view, setView] = useState<AuditView | null>(null);
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);
  const [page, setPage] = useState(0);
  const [eligibleOnly, setEligibleOnly] = useState(false);
  const [baseline, setBaseline] = useState("");
  const [source, setSource] = useState("");
  const [exports, setExports] = useState<string[]>([]);
  const [controls, setControls] = useState<Controls>({
    expected_tier: "pro_20x",
    model: data.report.models[0]?.model || "gpt-6-astra",
    effort: "high",
    context_band: "medium",
    workload_class: "coding",
    fast_off_attested: false,
    no_subagents_attested: false,
    exclusive_local_use_attested: false,
  });
  const reload = useCallback(async () => {
    try {
      setView(await call<AuditView>("audit_report"));
      setError("");
    } catch (e) {
      setError(String(e));
    }
  }, []);
  useEffect(() => {
    void reload();
    let timer: ReturnType<typeof setTimeout>;
    const unsub = native
      ? listen("monitor-updated", () => {
          clearTimeout(timer);
          timer = setTimeout(() => {
            if (!document.hidden) void reload();
          }, 800);
        })
      : null;
    const tick = setInterval(() => {
      if (!document.hidden) void reload();
    }, 30000);
    return () => {
      clearTimeout(timer);
      clearInterval(tick);
      void unsub?.then((f) => f());
    };
  }, [reload]);
  const action = async (work: () => Promise<unknown>, message: string) => {
    setBusy(true);
    setError("");
    try {
      await work();
      await reload();
      onToast(message);
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };
  if (!view)
    return (
      <section className="panel">
        <p>{t(error || "Aligning official changes with local tokens…")}</p>
        <button onClick={() => void reload()}>{t("Retry")}</button>
      </section>
    );
  const e = view.evidence,
    s = e.summary,
    tz = data.settings.timezone;
  const rows = [...e.intervals]
    .reverse()
    .filter((r) => !eligibleOnly || r.eligible);
  const currentPage = Math.min(
    page,
    Math.max(0, Math.ceil(rows.length / 15) - 1),
  );
  const observed = e.intervals.reduce(
    (a, r) => ({
      fresh: a.fresh + r.tokens.uncached_input_tokens,
      cached: a.cached + r.tokens.cached_input_tokens,
      output: a.output + r.tokens.output_tokens,
      value: a.value + r.api_equivalent,
      unpriced: a.unpriced + r.unpriced_tokens,
    }),
    { fresh: 0, cached: 0, output: 0, value: 0, unpriced: 0 },
  );
  const names = Array.from(
    new Set([
      ...data.report.models.map((m) => m.model),
      ...data.report.catalog.entries.map((p) => p.model),
    ]),
  )
    .filter((m) => m !== "unknown")
    .sort();
  return (
    <div className="auditor">
      {error && (
        <div className="notice error" role="alert">
          {t(error)}
        </div>
      )}
      <section className="panel audit-intro">
        <div className="section-heading">
          <div>
            <p className="audit-eyebrow">{t("PRO TIER AUDITOR")}</p>
            <h2>{t("Plan capacity audit")}</h2>
          </div>
          <span className="badge estimated">{t("OBSERVED")}</span>
        </div>
        <div className="audit-plan">
          <div>
            <span className="muted">{t("Reported plan")}</span>
            <strong>{t(e.reported_plan)}</strong>
            <small>
              {t("Official account service ·")} {t(view.quota_status)}
            </small>
          </div>
          <div>
            <span className="muted">{t("Expected tier")}</span>
            <strong>{t(tierName(e.controls?.expected_tier))}</strong>
            <small>
              {t(
                e.controls
                  ? "User configured; not an entitlement claim"
                  : "Choose when starting a controlled observation",
              )}
            </small>
          </div>
          <div>
            <span className="muted">{t("Observation")}</span>
            <strong>
              {t(
                view.running
                  ? "Recording"
                  : e.started_at
                    ? "Completed"
                    : "Passive history",
              )}
            </strong>
            <small>
              {t(
                e.started_at
                  ? date(e.started_at, tz)
                  : "Recent 7 days of available readings",
              )}
            </small>
          </div>
        </div>
        <p className="muted">{t(e.boundary)}</p>
      </section>

      <div className="audit-columns">
        <section className="panel">
          <h2>{t("Observed data")}</h2>
          <p className="muted">
            {t(
              "All displayed intervals, including excluded evidence. API equivalent uses public base rates.",
            )}{" "}
          </p>
          {[
            ["Fresh input", count(observed.fresh)],
            ["Cached input", count(observed.cached)],
            ["Output", count(observed.output)],
            ["API equivalent", usd(observed.value)],
            ["Unpriced tokens", exact(observed.unpriced)],
          ].map(([k, v]) => (
            <div className="key-row" key={k}>
              <span>{t(k)}</span>
              <b>{v}</b>
            </div>
          ))}
          <h3 className="audit-subheading">{t("Eligible evidence")}</h3>
          {[
            ["Weekly usage change", pp(s.quota_change)],
            ["Measured workloads", exact(s.workloads)],
            ["Quota-change windows", exact(s.samples)],
            ["Observation coverage", duration(s.observation_seconds)],
            ["API equivalent", usd(s.api_equivalent)],
          ].map(([k, v]) => (
            <div className="key-row" key={k}>
              <span>{t(k)}</span>
              <b>{v}</b>
            </div>
          ))}
        </section>
        <section className="panel audit-capacity">
          <h2>{t("Observed effective capacity")}</h2>
          <strong className="audit-capacity-number">
            {t(
              s.capacity_per_week == null
                ? "Collecting data"
                : `≈ ${usd(s.capacity_per_week)}`,
            )}
          </strong>
          <p className="muted">
            {t("API-equivalent / weekly meter · observed estimate")}{" "}
          </p>
          <div className="key-row">
            <span>{t("API equivalent / 1%")}</span>
            <b>
              {usd(s.usd_per_percent)}
              {s.dispersion != null && ` ± ${usd(s.dispersion)}`}
            </b>
          </div>
          <small>
            {t(
              "± shows weighted window dispersion, not a confidence interval.",
            )}{" "}
          </small>
          {s.capacity_range && (
            <p className="muted">
              {t("Sensitivity range")} {usd(s.capacity_range[0])}–
              {usd(s.capacity_range[1])} {t("/ week")}{" "}
            </p>
          )}
          <h3 className="audit-subheading">{t("Relative capacity index")}</h3>
          <div className="key-row">
            <span>{t("Pro 5x baseline")}</span>
            <b>{t(e.baseline_capacity == null ? "Not supplied" : "1.00×")}</b>
          </div>
          <div className="key-row">
            <span>{t("Observed")}</span>
            <b>
              {t(
                e.relative_index == null
                  ? "—"
                  : `${e.relative_index.toFixed(2)}×`,
              )}
            </b>
          </div>
          <div className="key-row">
            <span>{t("Pro 20x nominal reference")}</span>
            <b>~4.00×</b>
          </div>
          <small>
            {t(
              "The nominal ratio is a comparison assumption, not a fixed weekly-dollar promise.",
            )}{" "}
          </small>
        </section>
      </div>

      <section
        className={`panel audit-assessment ${e.assessment === "PRO-5X-LIKE" ? "audit-signal" : ""}`}
      >
        <div className="section-heading">
          <h2>{t("Assessment")}</h2>
          <span className="badge estimated">
            {t("Confidence:")} {t(e.confidence)}
          </span>
        </div>
        <strong>{t(e.assessment)}</strong>
        <p>
          {t(
            e.assessment === "PRO-5X-LIKE"
              ? "Observed capacity is substantially closer to the supplied Pro 5x baseline under the declared controls."
              : e.assessment === "PRO-20X-LIKE"
                ? "Observed capacity resembles the nominal Pro 20x multiple of the supplied baseline under the declared controls."
                : "The available evidence does not support a tier classification yet.",
          )}
        </p>
        <ul>
          {[...new Set([...e.assessment_reasons, ...s.reasons])].map((r) => (
            <li key={r}>{t(r)}</li>
          ))}
        </ul>
        <small>
          {t(
            "Confidence is a heuristic evidence-quality level. User-attested Fast/subagent controls cap it at MEDIUM. No result proves a backend entitlement error.",
          )}{" "}
        </small>
      </section>

      <section className="panel">
        <div className="section-heading">
          <div>
            <h2>{t("Official changes ↔ local tokens")}</h2>
            <p className="muted">
              {t(
                "Every observed change remains visible. Quota is remaining %, Δ is used percentage points. Times use",
              )}{" "}
              {t(tz)}.
            </p>
          </div>
          <label className="check">
            <input
              type="checkbox"
              checked={eligibleOnly}
              onChange={(x) => {
                setEligibleOnly(x.target.checked);
                setPage(0);
              }}
            />
            {t("Eligible only")}{" "}
          </label>
        </div>
        <div className="table-wrap">
          <table className="audit-table">
            <thead>
              <tr>
                <th>{t("Time window")}</th>
                <th>{t("Weekly")}</th>
                <th>{t("Δ quota")}</th>
                <th>{t("API eq.")}</th>
                <th>$ / 1%</th>
                <th>{t("Workloads")}</th>
                <th>{t("Evidence")}</th>
              </tr>
            </thead>
            <tbody>
              {rows
                .slice(currentPage * 15, currentPage * 15 + 15)
                .map((r, i) => (
                  <tr key={`${r.start}-${r.end}-${i}`}>
                    <td>
                      <span>{date(r.end, tz)}</span>
                      <small>
                        {t("from")} {date(r.start, tz)}
                      </small>
                    </td>
                    <td>
                      {t(r.weekly_before == null ? "—" : `${r.weekly_before}%`)}{" "}
                      → {t(r.weekly_after == null ? "—" : `${r.weekly_after}%`)}
                    </td>
                    <td>{pp(r.delta_quota)}</td>
                    <td>
                      {usd(r.api_equivalent)}
                      {r.unpriced_tokens > 0 && ` ${t("partial")}`}
                    </td>
                    <td>{usd(r.usd_per_percent)}</td>
                    <td>{r.workloads.length}</td>
                    <td>
                      <details>
                        <summary>
                          {t(r.eligible ? "Eligible" : "Excluded")}{" "}
                          {t("· details")}{" "}
                        </summary>
                        <p>
                          {t(
                            r.reasons.map((reason) => t(reason)).join("; ") ||
                              "Matches controlled observation",
                          )}
                        </p>
                        <p>
                          {r.models.join(" / ")} ·{" "}
                          {t(r.efforts.map((effort) => t(effort)).join(" / "))}
                        </p>
                        <p>
                          {t("Fresh")} {exact(r.tokens.uncached_input_tokens)}{" "}
                          {t("· cached")} {exact(r.tokens.cached_input_tokens)}{" "}
                          {t("· output")} {exact(r.tokens.output_tokens)} (
                          {exact(r.tokens.reasoning_output_tokens)}{" "}
                          {t("reasoning included)")}{" "}
                        </p>
                        <p>
                          {t("Fast:")} {t(r.fast_evidence)} {t("· subagents:")}{" "}
                          {t(r.subagent_evidence)}
                        </p>
                        <small>
                          {t("Input / response")} {exact(r.input_min)}–
                          {exact(r.input_max)} · {r.responses}{" "}
                          {t("responses ·")} {r.tool_calls}{" "}
                          {t("tool calls · reset")} {date(r.reset_at, tz)}
                        </small>
                      </details>
                    </td>
                  </tr>
                ))}
              {!rows.length && (
                <tr>
                  <td colSpan={7}>
                    {t(
                      eligibleOnly && e.intervals.length > 0
                        ? "No eligible intervals yet. Turn off the filter to inspect exclusions."
                        : "Waiting for two official readings with a measurable weekly change.",
                    )}
                  </td>
                </tr>
              )}
            </tbody>
          </table>
        </div>
        <div className="table-controls">
          <span className="muted">
            {t("{count} intervals · page {page} of {pages}", {
              count: rows.length,
              page: currentPage + 1,
              pages: Math.max(1, Math.ceil(rows.length / 15)),
            })}
          </span>
          <div>
            <button
              disabled={currentPage === 0}
              onClick={() => setPage(currentPage - 1)}
            >
              {t("Previous")}{" "}
            </button>{" "}
            <button
              disabled={(currentPage + 1) * 15 >= rows.length}
              onClick={() => setPage(currentPage + 1)}
            >
              {t("Next")}{" "}
            </button>
          </div>
        </div>
      </section>

      <section className="panel">
        <div className="section-heading">
          <h2>{t("Controlled observation")}</h2>
          {(view.running || view.active_elsewhere) && (
            <button
              disabled={busy}
              onClick={() =>
                void action(
                  () => call("stop_audit"),
                  "Observation stopped; evidence retained.",
                )
              }
            >
              {t("Stop observation")}{" "}
            </button>
          )}
        </div>
        {view.running ? (
          <>
            <p>
              {t("Recording")} {e.controls?.model} · {t(e.controls?.effort)}{" "}
              {t("reasoning ·")} {t(e.controls?.context_band)}{" "}
              {t("input context ·")} {t(e.controls?.workload_class)}.
            </p>
            <p className="muted">
              {t(
                "Keep Fast off and avoid subagents or other clients sharing this account quota. Stop this observation before changing those conditions. Existing evidence and prices remain frozen.",
              )}{" "}
            </p>
          </>
        ) : view.active_elsewhere ? (
          <p>
            {t(
              "Another account has an active observation. Stop it before starting this account.",
            )}{" "}
          </p>
        ) : (
          <>
            <p className="muted">
              {t(
                "This records your measurement protocol. It does not change Codex settings. Start before the workload; historical activity cannot be retroactively declared controlled.",
              )}{" "}
            </p>
            <div className="audit-form">
              <label>
                {t("Expected tier")}{" "}
                <select
                  value={controls.expected_tier}
                  onChange={(x) =>
                    setControls({ ...controls, expected_tier: x.target.value })
                  }
                >
                  <option value="pro_20x">
                    {t("Pro 20x — user configured")}
                  </option>
                  <option value="pro_5x">
                    {t("Pro 5x — user configured")}
                  </option>
                  <option value="unspecified">{t("Unspecified")}</option>
                </select>
              </label>
              <label>
                {t("Model")}{" "}
                <select
                  value={controls.model}
                  onChange={(x) =>
                    setControls({ ...controls, model: x.target.value })
                  }
                >
                  {names.map((m) => (
                    <option key={m}>{m}</option>
                  ))}
                </select>
              </label>
              <label>
                {t("Reasoning")}{" "}
                <select
                  value={controls.effort}
                  onChange={(x) =>
                    setControls({ ...controls, effort: x.target.value })
                  }
                >
                  {[
                    "none",
                    "minimal",
                    "low",
                    "medium",
                    "high",
                    "xhigh",
                    "max",
                    "ultra",
                  ].map((v) => (
                    <option key={v} value={v}>
                      {t(v)}
                    </option>
                  ))}
                </select>
              </label>
              <label>
                {t("Input tokens / response")}{" "}
                <select
                  value={controls.context_band}
                  onChange={(x) =>
                    setControls({ ...controls, context_band: x.target.value })
                  }
                >
                  <option value="short">{t("Short · ≤32k")}</option>
                  <option value="medium">{t("Medium · 32k–128k")}</option>
                  <option value="long">{t("Long · 128k–256k")}</option>
                  <option value="extended">{t("Extended · >256k")}</option>
                </select>
              </label>
              <label>
                {t("Workload class")}{" "}
                <select
                  value={controls.workload_class}
                  onChange={(x) =>
                    setControls({ ...controls, workload_class: x.target.value })
                  }
                >
                  {["coding", "research", "writing", "no_tools"].map((v) => (
                    <option key={v} value={v}>
                      {t(v)}
                    </option>
                  ))}
                </select>
              </label>
            </div>
            <div className="audit-declarations">
              <label className="check">
                <input
                  type="checkbox"
                  checked={controls.fast_off_attested}
                  onChange={(x) =>
                    setControls({
                      ...controls,
                      fast_off_attested: x.target.checked,
                    })
                  }
                />
                {t("I have turned Fast off for this observation.")}{" "}
              </label>
              <label className="check">
                <input
                  type="checkbox"
                  checked={controls.no_subagents_attested}
                  onChange={(x) =>
                    setControls({
                      ...controls,
                      no_subagents_attested: x.target.checked,
                    })
                  }
                />
                {t("I will avoid subagents and delegated tasks.")}{" "}
              </label>
              <label className="check">
                <input
                  type="checkbox"
                  checked={controls.exclusive_local_use_attested}
                  onChange={(x) =>
                    setControls({
                      ...controls,
                      exclusive_local_use_attested: x.target.checked,
                    })
                  }
                />
                {t(
                  "Only this Mac's signed-in subscription workloads will use the quota; no API-key usage, other devices, cloud tasks or shared agentic features.",
                )}{" "}
              </label>
            </div>
            <button
              className="primary"
              disabled={
                busy ||
                view.quota_status !== "LIVE" ||
                !controls.fast_off_attested ||
                !controls.no_subagents_attested ||
                !controls.exclusive_local_use_attested
              }
              onClick={() =>
                void action(
                  () => call("start_audit", { controls }),
                  "Controlled observation started. Prices and declarations recorded.",
                )
              }
            >
              {t("Start controlled observation")}{" "}
            </button>
          </>
        )}
      </section>

      <div className="audit-columns">
        <section className="panel">
          <h2>{t("Independent Pro 5x baseline")}</h2>
          <p className="muted">
            {t(
              "Import a completed, qualifying Auditor JSON from a separate Pro 5x account. No baseline dollar amount is assumed. Imported measurements are user supplied, not authenticated by OpenAI.",
            )}{" "}
          </p>
          {e.baseline_source && (
            <p>
              {t("Source:")}{" "}
              <span className="audit-source">{e.baseline_source}</span>
            </p>
          )}
          <label>
            {t("Baseline provenance")}{" "}
            <input
              type="text"
              value={source}
              onChange={(x) => setSource(x.target.value)}
              placeholder={t("https://github.com/… or private-reference")}
            />
          </label>
          <label>
            {t("Evidence JSON")}{" "}
            <input
              type="file"
              accept=".json,application/json"
              onChange={async (x) => {
                const file = x.target.files?.[0];
                if (!file) return;
                if (file.size > 2 * 1024 * 1024) {
                  setError("Baseline must be at most 2 MiB");
                  return;
                }
                setBaseline(await file.text());
              }}
            />
          </label>
          <div className="audit-buttons">
            <button
              disabled={busy || !baseline || !source}
              onClick={() =>
                void action(
                  () =>
                    call("import_audit_baseline", { text: baseline, source }),
                  "Baseline validated and imported.",
                )
              }
            >
              {t("Validate and import")}{" "}
            </button>
            {view.baseline_loaded && (
              <button
                disabled={busy}
                onClick={() =>
                  void action(
                    () => call("clear_audit_baseline"),
                    "Baseline removed; measurements preserved.",
                  )
                }
              >
                {t("Remove baseline")}{" "}
              </button>
            )}
          </div>
        </section>
        <section className="panel">
          <h2>{t("Evidence Report")}</h2>
          <p>
            {t(
              "Export a report for your own review or OpenAI Support, including tokens, official changes, model, reasoning, timestamps, exclusions and calculation method.",
            )}{" "}
          </p>
          <p className="muted">
            {t(
              "JSON, HTML and CSV are generated together. Account identifiers, email, original session IDs, paths, prompts and tool payloads are omitted. Files stay on this Mac.",
            )}{" "}
          </p>
          <button
            className="primary"
            disabled={busy}
            onClick={() =>
              void action(
                async () => setExports(await call<string[]>("export_audit")),
                "Redacted evidence report exported: JSON, HTML and CSV.",
              )
            }
          >
            {t("Export Evidence Report")}{" "}
          </button>
          {exports.length > 0 && (
            <div className="audit-exported" role="status">
              <p>
                {t("Saved")} {exports.length} {t("report files.")}
              </p>
              <button
                onClick={() =>
                  void action(
                    () => call("open_exports"),
                    "Export folder opened.",
                  )
                }
              >
                {t("Show export folder")}{" "}
              </button>
            </div>
          )}
        </section>
      </div>
      <section className="panel">
        <details>
          <summary>{t("Method, thresholds and evidence boundaries")}</summary>
          <ol>
            {e.methodology.map((m) => (
              <li key={m}>{t(m)}</li>
            ))}
          </ol>
          <p className="muted">
            {t("Prices verified")} {e.pricing.verified_at}
            {t(". Observation price fingerprint:")}{" "}
            {e.pricing.fingerprint.slice(0, 16)}…
          </p>
          <p className="muted">
            {t(
              "Official reference: learn.chatgpt.com/docs/pricing · learn.chatgpt.com/docs/agent-configuration/speed",
            )}{" "}
          </p>
        </details>
      </section>
    </div>
  );
}
