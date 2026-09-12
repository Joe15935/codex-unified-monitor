import {
  t,
  useLanguage,
  applyLanguage,
  getLanguage,
  type Language,
} from "./i18n";
import LanguageSwitch from "./components/LanguageSwitch";
import { useState, useEffect, useCallback, useRef, useMemo } from "react";
import { listen } from "@tauri-apps/api/event";
import type {
  Data,
  Range,
  Aggregate,
  Quota,
  Window,
  Settings,
  SessionRow,
  Detail,
  Price,
  Burn,
} from "./types";
import {
  call,
  native,
  count,
  exact,
  usd,
  money,
  value,
  coverage,
  ratio,
  percent,
  duration,
  date,
  priceNote,
} from "./data";
import { TokenGlyph, Trend } from "./components/Charts";
import TierAuditor from "./components/TierAuditor";
import { quotaInsight } from "./quotaInsight";
import { useRefresh } from "./useRefresh";

const periods = [
  ["today", "Today"],
  ["5h", "5h"],
  ["week", "Week"],
  ["month", "Month"],
  ["30d", "30d"],
  ["90d", "90d"],
  ["year", "Year"],
  ["all", "All"],
  ["custom", "Custom"],
];
function Badge({ status }: { status: string }) {
  return <span className={`badge ${status.toLowerCase()}`}>{t(status)}</span>;
}
function Stat({
  label,
  children,
  note,
}: {
  label: string;
  children: React.ReactNode;
  note?: string;
}) {
  return (
    <div className="stat">
      <span className="muted">{t(label)}</span>
      <strong>{children}</strong>
      {note && <small>{t(note)}</small>}
    </div>
  );
}
function useClock() {
  const [now, setNow] = useState(Date.now() / 1000);
  useEffect(() => {
    const tick = () => {
      if (!document.hidden) setNow(Date.now() / 1000);
    };
    const timer = setInterval(tick, 1000);
    document.addEventListener("visibilitychange", tick);
    window.addEventListener("focus", tick);
    return () => {
      clearInterval(timer);
      document.removeEventListener("visibilitychange", tick);
      window.removeEventListener("focus", tick);
    };
  }, []);
  return now;
}
function QuotaCard({
  title,
  window: w,
  quota,
  tz,
  settings,
}: {
  title: string;
  window: Window | null;
  quota: Quota;
  tz: string;
  settings: Settings;
}) {
  const now = useClock();
  const insight = quotaInsight(w, quota.meta, now, settings);
  const live = quota.meta.status === "LIVE";
  return (
    <section className={`kpi ${!live ? "muted-kpi" : ""}`}>
      <div className="card-top">
        <span>{t(title)}</span>
        <Badge status={w ? quota.meta.status : "UNAVAILABLE"} />
      </div>
      {w ? (
        <>
          <div className="quota-number">
            {t(w.remaining_percent.toFixed(0))}
            <span>{t("% remaining")}</span>
          </div>
          <div
            className="meter"
            role="meter"
            aria-label={t("{title} used", { title: title })}
            aria-valuenow={w.used_percent}
            aria-valuemin={0}
            aria-valuemax={100}
          >
            <i style={{ width: `${w.used_percent}%` }} />
          </div>
          <div className="card-foot">
            <span>
              {t(w.used_percent.toFixed(0))}
              {t("% used")}
            </span>
            <span title={date(w.resets_at, tz)}>
              {t(
                w.resets_at
                  ? t("Reset {time}", { time: duration(w.resets_at - now) })
                  : "Reset unavailable",
              )}
            </span>
          </div>
          <small className="reset-date">{date(w.resets_at, tz)}</small>
          {insight && (
            <div className="quota-insight">
              {insight.low && (
                <strong className="quota-low" role="status">
                  {t("Low quota · {remaining}% remaining", {
                    remaining: insight.remaining.toFixed(0),
                  })}
                </strong>
              )}
              {insight.elapsed != null && (
                <small
                  title={t(
                    "Even pace reference at the last reading; not an exhaustion forecast.",
                  )}
                >
                  {t("{used}% used · {elapsed}% of cycle elapsed", {
                    used: w.used_percent.toFixed(0),
                    elapsed: insight.elapsed.toFixed(0),
                  })}
                  <span>
                    {insight.pace === "ahead"
                      ? t("Above even pace")
                      : insight.pace === "below"
                        ? t("Below even pace")
                        : t("Near even pace")}
                  </span>
                </small>
              )}
            </div>
          )}
        </>
      ) : (
        <>
          <div className="quota-number">—</div>
          <p className="muted">
            {t("This window was not returned by the account service.")}{" "}
          </p>
        </>
      )}
    </section>
  );
}
function TokenStrip({ a }: { a: Aggregate }) {
  return (
    <div className="token-strip">
      <Stat label={t("Total tokens")} note={t("Raw input + output")}>
        {count(a.tokens.total_tokens)}
      </Stat>
      <Stat label={t("Fresh input")}>
        {count(a.tokens.uncached_input_tokens)}
      </Stat>
      <Stat label={t("Cached input")}>
        {count(a.tokens.cached_input_tokens)}
      </Stat>
      <Stat
        label={t("Output")}
        note={t("{count} reasoning, included", {
          count: count(a.tokens.reasoning_output_tokens),
        })}
      >
        {count(a.tokens.output_tokens)}
      </Stat>
      <Stat label={t("Cache hit")} note={t("Cached / raw input")}>
        {percent(ratio(a))}
      </Stat>
    </div>
  );
}
function BurnPanel({
  b,
  title,
  tz,
  weekly = false,
}: {
  b: Burn;
  title: string;
  tz: string;
  weekly?: boolean;
}) {
  return (
    <section className="panel burn">
      <div className="card-top">
        <h3>{t(title)}</h3>
        <Badge status="ESTIMATED" />
      </div>
      {b.percent_per_hour == null ? (
        <>
          <strong>{t(b.status)}</strong>
          <p className="muted">
            {t(
              b.status.toLowerCase() === "unavailable"
                ? "The account provider has not supplied this quota window."
                : t(
                    "{count} samples · Requires 4 readings over at least 30 minutes and a measurable quota change.",
                    { count: b.samples },
                  ),
            )}
          </p>
        </>
      ) : (
        <>
          <strong>
            {t(
              weekly
                ? t("{value}% / day", {
                    value: b.percent_per_day?.toFixed(1) ?? "—",
                  })
                : t("{value}% / hour", {
                    value: b.percent_per_hour.toFixed(1),
                  }),
            )}
          </strong>
          <p>
            {t(
              b.resets_before_exhaustion
                ? "Window resets before projected exhaustion."
                : t("Projected exhaustion {time}", {
                    time: date(b.exhausted_at, tz),
                  }),
            )}
          </p>
          <div className="observed">
            <span>{t("Observed efficiency / 1% quota")}</span>
            <b>
              {t(
                b.tokens_per_percent == null
                  ? "—"
                  : count(b.tokens_per_percent),
              )}{" "}
              {t("tokens ·")} {usd(b.equivalent_usd_per_percent)}
            </b>
            <small>
              {t(
                b.output_per_percent == null
                  ? "—"
                  : count(b.output_per_percent),
              )}{" "}
              {t("output tokens ·")} {b.samples} {t("readings over")}{" "}
              {duration(b.observed_seconds)}
            </small>
          </div>
        </>
      )}
    </section>
  );
}
function SessionTable({
  rows,
  mode,
  tz,
  onOpen,
  limit = 15,
}: {
  rows: SessionRow[];
  mode: string;
  tz: string;
  onOpen: (id: string) => void;
  limit?: number;
}) {
  const [sort, setSort] = useState("value");
  const [search, setSearch] = useState("");
  const [page, setPage] = useState(0);
  const key = (s: SessionRow) =>
    sort === "tokens"
      ? s.aggregate.tokens.total_tokens
      : sort === "cache"
        ? -(ratio(s.aggregate) ?? 1)
        : sort === "longest"
          ? s.last_active - s.started_at
          : sort === "output"
            ? s.aggregate.tokens.output_tokens
            : money(s.aggregate, mode).known_value_usd;
  const filtered = useMemo(
    () =>
      rows
        .filter((s) =>
          `${s.id} ${s.project} ${s.models.join(" ")}`
            .toLowerCase()
            .includes(search.toLowerCase()),
        )
        .sort((a, b) => key(b) - key(a)),
    [rows, search, sort, mode],
  );
  const current = Math.min(
    page,
    Math.max(0, Math.ceil(filtered.length / limit) - 1),
  );
  const sortButton = (name: string, label: string) => (
    <button
      className="table-sort"
      onClick={() => {
        setSort(name);
        setPage(0);
      }}
    >
      {t(label)}
      {t(sort === name ? " ↓" : "")}
    </button>
  );
  return (
    <>
      <div className="table-controls">
        <input
          aria-label={t("Search sessions")}
          placeholder={t("Find a project, session or model…")}
          value={search}
          onChange={(e) => {
            setSearch(e.target.value);
            setPage(0);
          }}
        />
        <select
          aria-label={t("Session ranking")}
          value={sort}
          onChange={(e) => {
            setSort(e.target.value);
            setPage(0);
          }}
        >
          <option value="value">{t("Top API equivalent")}</option>
          <option value="tokens">{t("Most tokens")}</option>
          <option value="cache">{t("Lowest cache hit")}</option>
          <option value="longest">{t("Longest sessions")}</option>
          <option value="output">{t("Most output")}</option>
        </select>
      </div>
      <div className="table-wrap">
        <table className="session-table">
          <thead>
            <tr>
              <th>{t("Session / project")}</th>
              <th>{t("Models / reasoning")}</th>
              <th>{sortButton("tokens", "Tokens")}</th>
              <th>{t("Fresh / cached")}</th>
              <th>{sortButton("output", "Output")}</th>
              <th>{sortButton("cache", "Cache hit")}</th>
              <th>{sortButton("value", "Equivalent $")}</th>
              <th>{t("Turns / tools")}</th>
              <th>{sortButton("longest", "Duration")}</th>
              <th>{t("Started / last active")}</th>
            </tr>
          </thead>
          <tbody>
            {filtered.slice(current * limit, (current + 1) * limit).map((s) => (
              <tr key={s.id}>
                <td>
                  <button
                    className="session-link"
                    onClick={() => onOpen(s.id)}
                    title={s.id}
                  >
                    {s.project.split("/").filter(Boolean).slice(-1)[0] ||
                      t("Codex session")}
                  </button>
                  <small>
                    {s.id.slice(0, 8)}…{s.id.slice(-6)}
                  </small>
                </td>
                <td>
                  {s.models.join(", ") || "—"}
                  <small>
                    {t(
                      s.efforts.map((effort) => t(effort)).join(", ") ||
                        "Unknown",
                    )}
                  </small>
                </td>
                <td>{count(s.aggregate.tokens.total_tokens)}</td>
                <td>
                  {count(s.aggregate.tokens.uncached_input_tokens)}
                  <small>
                    {count(s.aggregate.tokens.cached_input_tokens)}{" "}
                    {t("cached")}{" "}
                  </small>
                </td>
                <td>{count(s.aggregate.tokens.output_tokens)}</td>
                <td>{percent(ratio(s.aggregate))}</td>
                <td title={t(priceNote)}>
                  {usd(value(money(s.aggregate, mode)))}
                  {money(s.aggregate, mode).unpriced_tokens > 0 && (
                    <small className="amber">{t("Partly unpriced")}</small>
                  )}
                </td>
                <td>
                  {s.turns.length} / {s.aggregate.tool_calls}
                </td>
                <td>{duration(s.last_active - s.started_at)}</td>
                <td>
                  {date(s.started_at, tz)}
                  <small>{date(s.last_active, tz)}</small>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
        {!filtered.length && (
          <div className="empty-chart">{t("No sessions match this view.")}</div>
        )}
      </div>
      <div className="pagination">
        <span>
          {filtered.length} {t("sessions · metadata only")}
        </span>
        <div>
          <button onClick={() => setPage(current - 1)} disabled={current === 0}>
            {t("Previous")}{" "}
          </button>
          <span>
            {current + 1} / {Math.max(1, Math.ceil(filtered.length / limit))}
          </span>
          <button
            onClick={() => setPage(current + 1)}
            disabled={(current + 1) * limit >= filtered.length}
          >
            {t("Next")}{" "}
          </button>
        </div>
      </div>
    </>
  );
}
function DetailDialog({
  detail,
  onClose,
  mode,
  tz,
}: {
  detail: Detail | null;
  onClose: () => void;
  mode: string;
  tz: string;
}) {
  const ref = useRef<HTMLDialogElement>(null);
  useEffect(() => {
    ref.current?.showModal();
    return () => ref.current?.close();
  }, []);
  return (
    <dialog ref={ref} className="detail-dialog" onCancel={onClose}>
      <div className="section-heading">
        <div>
          <span className="eyebrow">{t("SESSION DETAILS")}</span>
          <h2>{detail?.id || t("Loading session…")}</h2>
        </div>
        <button onClick={onClose} aria-label={t("Close session details")}>
          {t("Close")}{" "}
        </button>
      </div>
      {detail && (
        <>
          <TokenStrip a={detail.aggregate} />
          <div className="inline-summary">
            <b>
              {t("API equivalent")} {usd(value(money(detail.aggregate, mode)))}
            </b>
            <span>
              {percent(coverage(money(detail.aggregate, mode)))}{" "}
              {t("priced")}{" "}
            </span>
          </div>
          <Trend points={detail.timeline} mode={mode} />
          <div className="two-col">
            <section>
              <h3>{t("Model & reasoning changes")}</h3>
              {detail.model_changes.map(([timestamp, m, e], i) => (
                <p key={i}>
                  <small>{date(timestamp, tz)}</small>
                  <br />
                  {m} · {t(e || "Unknown effort")}
                </p>
              ))}
            </section>
            <section>
              <h3>{t("Tool calls")}</h3>
              {detail.tools.map(([tool, n]) => (
                <div className="key-row" key={tool}>
                  <span>{tool}</span>
                  <b>{n}</b>
                </div>
              ))}
            </section>
          </div>
          <p className="muted">
            {t(
              "Only counters and metadata are stored. Prompts, reasoning text and tool outputs are excluded.",
            )}{" "}
          </p>
        </>
      )}
    </dialog>
  );
}
function Preferences({
  data,
  onSave,
  onToast,
}: {
  data: Data;
  onSave: (s: Settings) => Promise<void>;
  onToast: (s: string) => void;
}) {
  const [draft, setDraft] = useState<Settings>(structuredClone(data.settings));
  useEffect(() => {
    setDraft((previous) => ({ ...previous, language: data.settings.language }));
  }, [data.settings.language]);
  const [busy, setBusy] = useState(false);
  const [auto, setAuto] = useState(data.autostart);
  const [alias, setAlias] = useState("");
  const [target, setTarget] = useState("gpt-5.6-sol");
  const [model, setModel] = useState("");
  const [rates, setRates] = useState(["", "", ""]);
  const set = <K extends keyof Settings>(key: K, v: Settings[K]) =>
    setDraft({ ...draft, [key]: v });
  return (
    <div className="settings-grid">
      <section className="panel">
        <h2>{t("Preferences")}</h2>
        <div className="language-setting">
          <span>{t("Language")}</span>
          <LanguageSwitch />
        </div>
        <label>
          {t("Menu bar metric")}{" "}
          <select
            value={draft.tray_metric}
            onChange={(e) => set("tray_metric", e.target.value)}
          >
            <option value="weekly">{t("Weekly remaining")}</option>
            <option value="five_hour">{t("5-hour remaining")}</option>
            <option value="today_value">{t("Today API equivalent")}</option>
            <option value="today_tokens">{t("Today tokens")}</option>
          </select>
        </label>
        <label>
          {t("Time zone")}{" "}
          <input
            value={draft.timezone}
            onChange={(e) => set("timezone", e.target.value)}
          />
          <small>
            {t("Calendar days and Monday-start weeks use this time zone.")}{" "}
          </small>
        </label>
        <label>
          {t("Quota refresh")}{" "}
          <select
            value={draft.quota_poll_seconds}
            onChange={(e) => set("quota_poll_seconds", Number(e.target.value))}
          >
            {[60, 90, 120, 300, 600].map((n) => (
              <option key={n} value={n}>
                {n} {t("seconds")}{" "}
              </option>
            ))}
          </select>
        </label>
        <label className="check">
          <input
            type="checkbox"
            checked={draft.adaptive_refresh}
            onChange={(e) => set("adaptive_refresh", e.target.checked)}
          />
          {t("Slow down refresh when idle")}
        </label>
        <small>
          {t(
            "Up to 5 minutes while idle. Controlled audit observations keep the configured interval.",
          )}
        </small>
        <label>
          {t("Low quota hint")}
          <select
            value={draft.low_quota_threshold}
            onChange={(e) => set("low_quota_threshold", Number(e.target.value))}
          >
            {[0, 5, 10, 20].map((n) => (
              <option key={n} value={n}>
                {n === 0
                  ? t("Off")
                  : t("At {percent}% remaining", { percent: n })}
              </option>
            ))}
          </select>
          <small>
            {t(
              "Shown in the overview for fresh readings only. No system notifications.",
            )}
          </small>
        </label>
        <label>
          {t("Appearance")}{" "}
          <select
            value={draft.theme}
            onChange={(e) => set("theme", e.target.value)}
          >
            <option value="system">{t("System")}</option>
            <option value="light">{t("Light")}</option>
            <option value="dark">{t("Dark")}</option>
          </select>
        </label>
        <label className="check">
          <input
            type="checkbox"
            checked={auto}
            onChange={async (e) => {
              try {
                setAuto(
                  await call<boolean>("set_autostart", {
                    enabled: e.target.checked,
                  }),
                );
                onToast("Login item updated.");
              } catch (err) {
                onToast(String(err));
              }
            }}
          />
          {t("Launch at Login")}{" "}
        </label>
        <label className="check">
          <input
            type="checkbox"
            checked={draft.account_enabled}
            onChange={(e) => set("account_enabled", e.target.checked)}
          />
          {t("Read live account quota")}{" "}
        </label>
        <small>
          {t(
            "Turning this off disconnects this monitor. It does not sign you out of Codex.",
          )}{" "}
        </small>
      </section>
      <section className="panel">
        <h2>{t("Equivalent value")}</h2>
        <label>
          {t("Pricing basis")}{" "}
          <select
            value={draft.pricing_mode}
            onChange={(e) => set("pricing_mode", e.target.value)}
          >
            <option value="public_api">{t("Public API Equivalent")}</option>
            <option value="codex_work">
              {t("Codex / Work Rate-Card Equivalent")}{" "}
            </option>
          </select>
        </label>
        <label>
          {t("Monthly subscription cost (USD)")}{" "}
          <input
            type="number"
            min="0.01"
            step="0.01"
            placeholder={t("Optional; enter your own price")}
            value={draft.monthly_subscription_cost ?? ""}
            onChange={(e) =>
              set(
                "monthly_subscription_cost",
                e.target.value ? Number(e.target.value) : null,
              )
            }
          />
        </label>
        <p className="muted">
          {t(
            "Enables the Equivalent Value Multiple and break-even comparison. No subscription price is assumed.",
          )}{" "}
        </p>
        <h3>{t("Cache health thresholds")}</h3>
        <div className="three-col">
          {["Excellent ≥", "Good ≥", "Average ≥"].map((x, i) => (
            <label key={x}>
              {t(x)}
              <input
                type="number"
                min="0"
                max="100"
                value={Math.round(draft.cache_thresholds[i] * 100)}
                onChange={(e) => {
                  const t = [...draft.cache_thresholds];
                  t[i] = Number(e.target.value) / 100;
                  set("cache_thresholds", t);
                }}
              />
              %
            </label>
          ))}
        </div>
        <small>
          {t(
            "Below the Average threshold is Poor. Ratios are weighted by raw input tokens.",
          )}{" "}
        </small>
        <h3>{t("Model aliases")}</h3>
        <div className="alias-row">
          <input
            aria-label={t("Unknown model alias")}
            placeholder={t("Unknown model name")}
            value={alias}
            onChange={(e) => setAlias(e.target.value)}
          />
          <select
            aria-label={t("Alias target")}
            value={target}
            onChange={(e) => setTarget(e.target.value)}
          >
            {Array.from(
              new Set(data.report.catalog.entries.map((p) => p.model)),
            ).map((m) => (
              <option key={m}>{m}</option>
            ))}
          </select>
          <button
            onClick={() => {
              if (alias.trim()) {
                set("aliases", {
                  ...draft.aliases,
                  [alias.trim().toLowerCase()]: target,
                });
                setAlias("");
              }
            }}
          >
            {t("Add")}{" "}
          </button>
        </div>
        {Object.entries(draft.aliases).map(([a, b]) => (
          <div className="key-row" key={a}>
            <span>
              {a} → {b}
            </span>
            <button
              onClick={() => {
                const next = { ...draft.aliases };
                delete next[a];
                set("aliases", next);
              }}
            >
              {t("Remove")}{" "}
            </button>
          </div>
        ))}
      </section>
      <section className="panel full">
        <h2>{t("Custom model rates")}</h2>
        <p className="muted">
          {t(
            "USD per 1 million tokens, for the selected pricing basis. These are explicitly user supplied estimates.",
          )}{" "}
        </p>
        <div className="rate-form">
          <label>
            {t("Model")}{" "}
            <input
              value={model}
              onChange={(e) => setModel(e.target.value)}
              placeholder={t("Exact model name")}
            />
          </label>
          {["Fresh input", "Cached input", "Output"].map((x, i) => (
            <label key={x}>
              {t(x)}
              <input
                type="number"
                min="0"
                step="0.001"
                value={rates[i]}
                onChange={(e) =>
                  setRates(rates.map((v, j) => (i === j ? e.target.value : v)))
                }
              />
            </label>
          ))}
          <button
            onClick={() => {
              if (
                !model.trim() ||
                rates.some((r) => r === "" || !Number.isFinite(Number(r)))
              ) {
                onToast("Enter a model and all three rates.");
                return;
              }
              const p: Price = {
                model: model.trim(),
                input: Number(rates[0]),
                cached_input: Number(rates[1]),
                output: Number(rates[2]),
                cache_write: null,
                source: "User configured",
                effective_date: null,
                verified_at: new Date().toISOString().slice(0, 10),
                pricing_mode: draft.pricing_mode,
                processing: "standard",
                context: "base",
                notes: "User configured base rates",
              };
              set("custom_rates", [
                ...draft.custom_rates.filter(
                  (r) =>
                    !(r.model === p.model && r.pricing_mode === p.pricing_mode),
                ),
                p,
              ]);
              setModel("");
              setRates(["", "", ""]);
            }}
          >
            {t("Add rate")}{" "}
          </button>
        </div>
        {draft.custom_rates.map((p, i) => (
          <div className="key-row" key={`${p.model}:${p.pricing_mode}`}>
            <span>
              {p.model} · {t(p.pricing_mode)} · {p.input} / {p.cached_input} /{" "}
              {p.output}
            </span>
            <button
              onClick={() =>
                set(
                  "custom_rates",
                  draft.custom_rates.filter((_, j) => i !== j),
                )
              }
            >
              {t("Remove")}{" "}
            </button>
          </div>
        ))}
      </section>
      <div className="settings-actions">
        <button
          className="primary"
          disabled={busy}
          onClick={async () => {
            setBusy(true);
            try {
              await onSave(draft);
            } finally {
              setBusy(false);
            }
          }}
        >
          {t(busy ? "Saving…" : "Save preferences")}
        </button>
      </div>
    </div>
  );
}
function Compact({
  data,
  reload,
  onToast,
}: {
  data: Data;
  reload: () => void;
  onToast: (s: string) => void;
}) {
  const now = useClock();
  const q = data.quota;
  const a = data.report.periods.today;
  const mode = data.settings.pricing_mode;
  const row = (title: string, w: Window | null) => (
    <div className="compact-row">
      <span>{t(title)}</span>
      <b>
        {t(
          w
            ? t("{value}% remaining", { value: w.remaining_percent.toFixed(0) })
            : "Unavailable",
        )}
      </b>
      <small>
        {t(
          w?.resets_at
            ? t("reset {time}", { time: duration(w.resets_at - now) })
            : "Window not returned",
        )}
      </small>
    </div>
  );
  return (
    <div className="compact">
      <header>
        <TokenGlyph />
        <div>
          <h2>Codex</h2>
          <small>{t("Unified Monitor")}</small>
        </div>
        <Badge status={q.meta.status} />
        <LanguageSwitch />
      </header>
      {row("5-hour", q.five_hour)}
      {row("Weekly", q.weekly)}
      <div className="compact-pair">
        <Stat label={t("Today equivalent")}>{usd(value(money(a, mode)))}</Stat>
        <Stat label={t("Cache hit")}>{percent(ratio(a))}</Stat>
      </div>
      <div className="key-row">
        <span>{t("Reset credits")}</span>
        <b>{q.reset_credits?.available_count ?? t("Unavailable")}</b>
      </div>
      <small className="muted">
        {t(
          q.error ||
            `${q.meta.updated_at ? t("Last quota update {time} ago", { time: duration(now - q.meta.updated_at) }) : "Connecting to account…"}`,
        )}
      </small>
      <button
        className="primary wide"
        onClick={() => call("open_dashboard").catch((e) => onToast(String(e)))}
      >
        {t("Open Dashboard")}{" "}
      </button>
      <footer>
        <button
          onClick={async () => {
            await call("refresh");
            reload();
          }}
        >
          {t("Refresh")}{" "}
        </button>
        <button onClick={() => call("open_dashboard", { tab: "settings" })}>
          {t("Settings")}{" "}
        </button>
        <button onClick={() => call("quit")}>{t("Quit")}</button>
      </footer>
    </div>
  );
}
function QuotaStatus({
  quota,
  scanStatus,
}: {
  quota: Quota;
  scanStatus: string;
}) {
  const now = useClock();
  return (
    <div className="status-line">
      <span
        className={`status-dot ${quota.meta.status === "LIVE" ? "live" : ""}`}
      />
      <span>
        {t(
          quota.meta.status === "LIVE"
            ? "Live quota"
            : t("Quota {status}", { status: t(quota.meta.status) }),
        )}
        {t(
          quota.meta.updated_at
            ? " · " +
                t("{time} ago", {
                  time: duration(now - quota.meta.updated_at),
                })
            : "",
        )}
      </span>
      <span className="divider">/</span>
      <span>
        {t("Local data ·")} {t(scanStatus)}
      </span>
    </div>
  );
}
export default function App() {
  useLanguage();
  const compact = new URLSearchParams(location.search).get("compact") === "1";
  const [data, setData] = useState<Data | null>(null);
  const [error, setError] = useState("");
  const [toast, setToast] = useState("");
  const [tab, setTab] = useState("overview");
  const [range, setRange] = useState<Range>({ period: "today" });
  const [metric, setMetric] = useState("tokens");
  const [customStart, setCustomStart] = useState(
    new Date().toISOString().slice(0, 10),
  );
  const [customEnd, setCustomEnd] = useState(
    new Date().toISOString().slice(0, 10),
  );
  const [detail, setDetail] = useState<Detail | null>(null);
  const [detailOpen, setDetailOpen] = useState(false);
  const [format, setFormat] = useState("html");
  const [dataset, setDataset] = useState("daily");
  const detailRequest = useRef(0);
  const { refresh: reload, invalidate } = useRefresh<Data>({
    key: JSON.stringify(range),
    read: () => call<Data>("dashboard", { range }),
    commit: (d) => {
      if (native && d.settings.language) applyLanguage(d.settings.language);
      setData(d);
      setError("");
    },
    error: (e) => setError(String(e)),
  });
  useEffect(() => {
    if (native) void call("window_ready").catch((e) => setError(String(e)));
    return () => {
      detailRequest.current += 1;
    };
  }, []);
  useEffect(() => {
    applyLanguage(getLanguage());
    if (!native) return;
    const unlisten = listen<Language>("language-changed", ({ payload }) => {
      invalidate();
      applyLanguage(payload);
      setData((previous) =>
        previous
          ? {
              ...previous,
              settings: { ...previous.settings, language: payload },
            }
          : previous,
      );
    }).catch(() => undefined);
    return () => {
      void unlisten.then((dispose) => dispose?.());
    };
  }, []);
  useEffect(() => {
    if (!native) return;
    const nav = listen<string>("navigate", (e) => setTab(e.payload)).catch(
      () => undefined,
    );
    return () => {
      void nav.then((dispose) => dispose?.());
    };
  }, []);
  useEffect(() => {
    if (toast) {
      const t = setTimeout(() => setToast(""), 6500);
      return () => clearTimeout(t);
    }
  }, [toast]);
  useEffect(() => {
    document.documentElement.dataset.theme = data?.settings.theme || "system";
  }, [data?.settings.theme]);
  const closeDetail = useCallback(() => {
    detailRequest.current += 1;
    setDetailOpen(false);
  }, []);
  const openSession = useCallback(async (id: string) => {
    const request = ++detailRequest.current;
    setDetailOpen(true);
    setDetail(null);
    try {
      const next = await call<Detail>("session_detail", { id });
      if (request === detailRequest.current) setDetail(next);
    } catch (e) {
      if (request === detailRequest.current) {
        setToast(String(e));
        setDetailOpen(false);
      }
    }
  }, []);
  if (!data)
    return (
      <div className="loading">
        <TokenGlyph />
        <h1>Codex Unified Monitor</h1>
        <p>
          {t(error || "Reading local counters and preparing your dashboard…")}
        </p>
        {error && <button onClick={() => void reload()}>{t("Retry")}</button>}
        <small>{t("Local first · Read only · No telemetry")}</small>
      </div>
    );
  if (compact)
    return (
      <Compact data={data} reload={() => void reload()} onToast={setToast} />
    );
  const r = data.report,
    q = data.quota,
    s = data.settings,
    a = r.total,
    mode = s.pricing_mode;
  const m = money(a, mode);
  const today = r.periods.today,
    month = r.periods.month;
  const cache = ratio(a);
  const quality =
    cache == null
      ? "Unavailable"
      : cache >= s.cache_thresholds[0]
        ? "Excellent"
        : cache >= s.cache_thresholds[1]
          ? "Good"
          : cache >= s.cache_thresholds[2]
            ? "Average"
            : "Poor";
  const monthValue = value(money(month, mode));
  const save = async (next: Settings) => {
    try {
      await call("save_settings", { settings: next });
      invalidate();
      await reload();
      setToast("Preferences saved.");
    } catch (e) {
      setToast(String(e));
    }
  };
  return (
    <div className="shell">
      <aside className="sidebar">
        <div className="brand">
          <TokenGlyph />
          <span>
            Codex<span className="brand-sub">{t("UNIFIED MONITOR")}</span>
          </span>
        </div>
        <nav aria-label={t("Main navigation")}>
          {[
            ["overview", "Overview"],
            ["sessions", "Sessions"],
            ["cache", "Cache health"],
            ["pricing", "API equivalent"],
            ["auditor", "Tier Auditor"],
            ["diagnostics", "Data health"],
            ["settings", "Settings"],
          ].map(([id, label]) => (
            <button
              className={tab === id ? "active" : ""}
              aria-current={tab === id ? "page" : undefined}
              key={id}
              onClick={() => setTab(id)}
            >
              {t(label)}
            </button>
          ))}
        </nav>
        <div className="sidebar-bottom">
          <span className="local-mark">{t("Local first")}</span>
          <p>
            {t("Read only.")} <br />
            {t("Your data stays here.")}{" "}
          </p>
          <small>v{data.version} · MIT</small>
        </div>
      </aside>
      <main>
        <header className="topbar">
          <div>
            <h1>
              {t(
                {
                  overview: "Usage overview",
                  sessions: "Session analytics",
                  cache: "Cache health",
                  pricing: "API equivalent value",
                  auditor: "Pro Tier Auditor",
                  diagnostics: "Data health",
                  settings: "Settings",
                }[tab],
              )}
            </h1>
            <QuotaStatus quota={q} scanStatus={r.diagnostics.scan_status} />
          </div>
          <div className="top-actions">
            <LanguageSwitch />
            <span className="account" title={q.account_label || ""}>
              {t(q.plan?.toUpperCase() || "ACCOUNT")}
              {q.account_label && <small>{q.account_label}</small>}
            </span>
            <button
              onClick={async () => {
                try {
                  await call("refresh");
                  await reload();
                  setToast(
                    "Refresh requested. Rate-limit backoff is respected.",
                  );
                } catch (e) {
                  setToast(String(e));
                }
              }}
            >
              {t("Refresh")}{" "}
            </button>
          </div>
        </header>
        {error && (
          <div className="notice error" role="alert">
            {t(error)}
          </div>
        )}
        {q.meta.status !== "LIVE" && (
          <div className="notice" role="status">
            {t("Live quota unavailable.")} {t(q.error)}{" "}
            {q.meta.updated_at &&
              t("Last successful reading: {time}.", {
                time: date(q.meta.updated_at, s.timezone),
              })}
          </div>
        )}
        {tab === "settings" ? (
          <Preferences data={data} onSave={save} onToast={setToast} />
        ) : tab === "auditor" ? (
          <TierAuditor data={data} onToast={setToast} />
        ) : (
          <>
            {tab === "overview" && (
              <>
                <div className="kpi-grid">
                  <QuotaCard
                    title={t("5-Hour")}
                    window={q.five_hour}
                    quota={q}
                    tz={s.timezone}
                    settings={s}
                  />
                  <QuotaCard
                    title={t("Weekly")}
                    window={q.weekly}
                    quota={q}
                    tz={s.timezone}
                    settings={s}
                  />
                  <section className="kpi">
                    <div className="card-top">
                      <span>{t("Reset credits")}</span>
                      <Badge
                        status={q.reset_credits ? q.meta.status : "UNAVAILABLE"}
                      />
                    </div>
                    <div className="quota-number">
                      {q.reset_credits?.available_count ?? "—"}
                      <span> {t("available")}</span>
                    </div>
                    <p className="muted">
                      {q.reset_credits?.details?.some((c) => c.expires_at)
                        ? t("Earliest expiry {time}", {
                            time: date(
                              Math.min(
                                ...q.reset_credits.details.flatMap((c) =>
                                  c.expires_at ? [c.expires_at] : [],
                                ),
                              ),
                              s.timezone,
                            ),
                          })
                        : q.reset_credits?.details === null
                          ? t("Grant / expiry details unavailable")
                          : q.reset_credits?.available_count === 0
                            ? t("No reset credits currently available.")
                            : t("Expiry unavailable")}
                    </p>
                  </section>
                  <section className="kpi value-kpi" title={t(priceNote)}>
                    <div className="card-top">
                      <span>{t("API equivalent")}</span>
                      <Badge status="ESTIMATED" />
                    </div>
                    <div className="quota-number">
                      {usd(value(money(today, mode)))}
                    </div>
                    <div className="card-foot">
                      <span>{t("Today")}</span>
                      <span>
                        {t("Month")} {usd(monthValue)}
                      </span>
                    </div>
                    <small className="reset-date">
                      {t("Base-rate equivalent estimate")}{" "}
                    </small>
                  </section>
                </div>
                <TokenStrip a={a} />
              </>
            )}
            <div className="rangebar">
              <div
                className="segmented"
                role="group"
                aria-label={t("Time range")}
              >
                {periods.map(([id, label]) => (
                  <button
                    key={id}
                    aria-pressed={range.period === id}
                    className={range.period === id ? "selected" : ""}
                    onClick={() =>
                      setRange(
                        id === "custom"
                          ? { period: id, start: customStart, end: customEnd }
                          : { period: id },
                      )
                    }
                  >
                    {t(label)}
                  </button>
                ))}
              </div>
              <span className="muted">{s.timezone}</span>
            </div>
            {range.period === "custom" && (
              <div className="custom-range">
                <label>
                  {t("From")}{" "}
                  <input
                    type="date"
                    value={customStart}
                    onChange={(e) => setCustomStart(e.target.value)}
                  />
                </label>
                <label>
                  {t("Through")}{" "}
                  <input
                    type="date"
                    value={customEnd}
                    onChange={(e) => setCustomEnd(e.target.value)}
                  />
                </label>
                <button
                  onClick={() =>
                    setRange({
                      period: "custom",
                      start: customStart,
                      end: customEnd,
                    })
                  }
                >
                  {t("Apply dates")}{" "}
                </button>
              </div>
            )}
            {tab === "overview" && (
              <>
                <section className="panel">
                  <div className="section-heading">
                    <div>
                      <h2>{t("Usage over time")}</h2>
                      <p className="muted">
                        {exact(a.tokens.total_tokens)} {t("tokens across")}{" "}
                        {a.responses} {t("recorded responses")}{" "}
                      </p>
                    </div>
                    <div className="segmented small">
                      {[
                        ["tokens", "Tokens"],
                        ["value", "Equivalent $"],
                        ["cache", "Cache hit %"],
                      ].map(([id, label]) => (
                        <button
                          key={id}
                          className={metric === id ? "selected" : ""}
                          onClick={() => setMetric(id)}
                        >
                          {t(label)}
                        </button>
                      ))}
                    </div>
                  </div>
                  <Trend points={r.trend} mode={mode} metric={metric} />
                  <div className="legend">
                    <span>
                      <i className="fresh" />
                      {t("Fresh input")}{" "}
                    </span>
                    <span>
                      <i className="cached" />
                      {t("Cached input")}{" "}
                    </span>
                    <span>
                      <i className="out" />
                      {t("Output")}{" "}
                    </span>
                    <small>
                      {percent(coverage(m))} {t("priced")}
                    </small>
                  </div>
                </section>
                <section className="panel">
                  <div className="section-heading">
                    <h2>{t("Model breakdown")}</h2>
                    <span className="muted">
                      {t("One accounting model, every view")}{" "}
                    </span>
                  </div>
                  <div className="table-wrap">
                    <table>
                      <thead>
                        <tr>
                          <th>{t("Model")}</th>
                          <th>{t("Tokens")}</th>
                          <th>{t("Fresh input")}</th>
                          <th>{t("Cached")}</th>
                          <th>{t("Output")}</th>
                          <th>{t("Cache hit")}</th>
                          <th>{t("Equivalent $")}</th>
                          <th>{t("Share")}</th>
                        </tr>
                      </thead>
                      <tbody>
                        {r.models.map((row) => (
                          <tr key={row.model}>
                            <td>
                              <b>{row.model}</b>
                            </td>
                            <td>{count(row.aggregate.tokens.total_tokens)}</td>
                            <td>
                              {count(
                                row.aggregate.tokens.uncached_input_tokens,
                              )}
                            </td>
                            <td>
                              {count(row.aggregate.tokens.cached_input_tokens)}
                            </td>
                            <td>{count(row.aggregate.tokens.output_tokens)}</td>
                            <td>{percent(ratio(row.aggregate))}</td>
                            <td>
                              {value(money(row.aggregate, mode)) == null ? (
                                <Badge status="UNPRICED" />
                              ) : (
                                usd(value(money(row.aggregate, mode)))
                              )}
                            </td>
                            <td>
                              {percent(
                                a.tokens.total_tokens
                                  ? row.aggregate.tokens.total_tokens /
                                      a.tokens.total_tokens
                                  : 0,
                              )}
                            </td>
                          </tr>
                        ))}
                      </tbody>
                    </table>
                  </div>
                </section>
                <div className="two-col">
                  <BurnPanel
                    title={t("5-hour burn rate")}
                    b={data.burn.five_hour}
                    tz={s.timezone}
                  />
                  <BurnPanel
                    title={t("Weekly burn rate")}
                    weekly
                    b={data.burn.weekly}
                    tz={s.timezone}
                  />
                </div>
                <p className="footnote">
                  {t(
                    "Observed efficiency compares account-wide quota changes with logs on this Mac. It is a local correlation, not OpenAI’s billing formula.",
                  )}{" "}
                </p>
                <section className="panel">
                  <div className="section-heading">
                    <h2>{t("Sessions worth a look")}</h2>
                    <button onClick={() => setTab("sessions")}>
                      {t("All sessions")}{" "}
                    </button>
                  </div>
                  <SessionTable
                    rows={r.sessions}
                    tz={s.timezone}
                    mode={mode}
                    onOpen={openSession}
                    limit={5}
                  />
                </section>
              </>
            )}
            {tab === "sessions" && (
              <section className="panel">
                <div className="section-heading">
                  <h2>{t("Find where the tokens went")}</h2>
                  <Badge status="LOCAL" />
                </div>
                <SessionTable
                  rows={r.sessions}
                  tz={s.timezone}
                  mode={mode}
                  onOpen={openSession}
                />
              </section>
            )}
            {tab === "cache" && (
              <>
                <div className="cache-hero panel">
                  <div>
                    <span className="eyebrow">{t("CACHE HIT RATIO")}</span>
                    <h2>{percent(cache)}</h2>
                    <span className="quality">{t(quality)}</span>
                    <p>
                      {t(
                        "Weighted by input tokens across the selected period.",
                      )}
                    </p>
                  </div>
                  <div>
                    <Stat label={t("Cached input / saved input tokens")}>
                      {count(a.tokens.cached_input_tokens)}
                    </Stat>
                    <Stat label={t("Fresh input")}>
                      {count(a.tokens.uncached_input_tokens)}
                    </Stat>
                    <Stat
                      label={t("Cache value saved")}
                      note={t(
                        "Additional base-rate API equivalent without cache hits",
                      )}
                    >
                      {t(
                        m.priced_tokens === 0 && m.unpriced_tokens > 0
                          ? "Unpriced"
                          : usd(m.cache_savings_usd),
                      )}
                    </Stat>
                  </div>
                </div>
                <div className="period-cache">
                  {["today", "5h", "week", "month", "all"].map((p) => (
                    <section key={p} className="panel">
                      <span>{t(periods.find(([id]) => id === p)?.[1])}</span>
                      <strong>{percent(ratio(r.periods[p]))}</strong>
                    </section>
                  ))}
                </div>
                <section className="panel">
                  <h2>{t("Cache trend")}</h2>
                  <Trend points={r.trend} mode={mode} metric="cache" />
                </section>
                <section className="panel">
                  <h2>{t("Cache by session")}</h2>
                  <SessionTable
                    rows={r.sessions}
                    tz={s.timezone}
                    mode={mode}
                    onOpen={openSession}
                  />
                </section>
              </>
            )}
            {tab === "pricing" && (
              <>
                <div className="two-col">
                  <section className="panel equivalent">
                    <div className="card-top">
                      <h2>{t("API equivalent value")}</h2>
                      <Badge status="ESTIMATED" />
                    </div>
                    <strong>{usd(value(m))}</strong>
                    <p>
                      {t(
                        mode === "codex_work"
                          ? "Codex / Work Rate-Card Equivalent"
                          : "Public API Equivalent",
                      )}
                    </p>
                    <select
                      aria-label={t("Pricing basis")}
                      value={mode}
                      onChange={(e) =>
                        save({ ...s, pricing_mode: e.target.value })
                      }
                    >
                      <option value="public_api">
                        {t("Public API Equivalent")}
                      </option>
                      <option value="codex_work">
                        {t("Codex / Work Rate-Card Equivalent")}{" "}
                      </option>
                    </select>
                    <p className="muted">{t(priceNote)}</p>
                  </section>
                  <section className="panel">
                    <h2>{t("Pricing coverage")}</h2>
                    <div className="coverage-number">
                      {percent(coverage(m))}
                    </div>
                    <p>
                      {count(m.priced_tokens)} {t("priced tokens ·")}{" "}
                      {count(m.unpriced_tokens)} {t("unpriced")}{" "}
                    </p>
                    <p className="muted">
                      {t(
                        "Unknown models stay unpriced until you explicitly assign an alias or custom rate.",
                      )}{" "}
                    </p>
                    {r.models
                      .filter(
                        (x) => money(x.aggregate, mode).unpriced_tokens > 0,
                      )
                      .map((x) => (
                        <div className="key-row" key={x.model}>
                          <span>{x.model}</span>
                          <Badge status="UNPRICED" />
                        </div>
                      ))}
                    <button onClick={() => setTab("settings")}>
                      {t("Manage model rates")}{" "}
                    </button>
                  </section>
                </div>
                <section className="panel">
                  <h2>{t("Subscription comparison")}</h2>
                  {s.monthly_subscription_cost ? (
                    <>
                      <div className="token-strip">
                        <Stat label={t("Monthly subscription")}>
                          {usd(s.monthly_subscription_cost)}
                        </Stat>
                        <Stat label={t("This month equivalent")}>
                          {usd(monthValue)}
                        </Stat>
                        <Stat label={t("Equivalent Value Multiple")}>
                          {t(
                            monthValue == null
                              ? "—"
                              : `${(monthValue / s.monthly_subscription_cost).toFixed(2)}×`,
                          )}
                        </Stat>
                        <Stat label={t("Break-even progress")}>
                          {t(
                            monthValue == null
                              ? "—"
                              : `${((monthValue / s.monthly_subscription_cost) * 100).toFixed(0)}%`,
                          )}
                        </Stat>
                      </div>
                      <small>
                        {t(
                          "Based on the priced portion of this month’s local logs.",
                        )}{" "}
                      </small>
                    </>
                  ) : (
                    <p className="muted">
                      {t(
                        "Enter your monthly subscription cost in Settings to enable this optional comparison.",
                      )}{" "}
                    </p>
                  )}
                </section>
                <section className="panel">
                  <div className="section-heading">
                    <h2>{t("Verified rate catalog")}</h2>
                    <span className="muted">
                      {t("USD / 1M tokens · verified")} {r.catalog.verified_at}
                    </span>
                  </div>
                  <div className="table-wrap">
                    <table>
                      <thead>
                        <tr>
                          <th>{t("Model")}</th>
                          <th>{t("Input")}</th>
                          <th>{t("Cached")}</th>
                          <th>{t("Output")}</th>
                          <th>{t("Source / verified")}</th>
                        </tr>
                      </thead>
                      <tbody>
                        {r.catalog.entries
                          .filter((p) => p.pricing_mode === mode)
                          .map((p) => (
                            <tr key={p.model} title={p.notes}>
                              <td>{p.model}</td>
                              <td>{p.input}</td>
                              <td>{p.cached_input}</td>
                              <td>{p.output}</td>
                              <td>
                                <a
                                  href={p.source}
                                  target="_blank"
                                  rel="noreferrer"
                                >
                                  {t("Official source")}{" "}
                                </a>
                                <small>{p.verified_at}</small>
                              </td>
                            </tr>
                          ))}
                      </tbody>
                    </table>
                  </div>
                  <p className="muted">
                    {t(
                      "Current catalog rates are applied to selected history. Promotional rates, historical price changes, cache writes and service adjustments may differ.",
                    )}{" "}
                  </p>
                  <label className="catalog-import">
                    {t("Import an updated pricing_catalog.json")}{" "}
                    <input
                      type="file"
                      accept=".json,application/json"
                      onChange={async (e) => {
                        const file = e.target.files?.[0];
                        if (!file) return;
                        try {
                          await call("import_catalog", {
                            text: await file.text(),
                          });
                          await reload();
                          setToast("Pricing catalog imported.");
                        } catch (err) {
                          setToast(String(err));
                        }
                      }}
                    />
                  </label>
                </section>
              </>
            )}
            {tab === "diagnostics" && (
              <>
                <section className="panel">
                  <div className="section-heading">
                    <h2>{t("Local data pipeline")}</h2>
                    <Badge status="LOCAL" />
                  </div>
                  <div className="diagnostic-grid">
                    {[
                      ["Session files", r.diagnostics.session_files],
                      ["Parsed files", r.diagnostics.parsed_files],
                      ["Metadata events", r.diagnostics.events],
                      [
                        "Duplicates rejected",
                        r.diagnostics.duplicates_rejected,
                      ],
                      ["Parser errors", r.diagnostics.parser_errors],
                      [
                        "Oversized body lines skipped",
                        r.diagnostics.oversized_lines,
                      ],
                      [
                        "Largest committed byte offset",
                        r.diagnostics.last_offset,
                      ],
                      [
                        "Database + journal bytes",
                        r.diagnostics.database_bytes,
                      ],
                    ].map(([label, n]) => (
                      <Stat key={String(label)} label={String(label)}>
                        {exact(Number(n))}
                      </Stat>
                    ))}
                  </div>
                  <div className="key-row">
                    <span>{t("Local source")}</span>
                    <code>~/.codex/sessions + archived_sessions</code>
                  </div>
                  <div className="key-row">
                    <span>{t("Last ingest")}</span>
                    <span>
                      {date(r.diagnostics.last_local_update, s.timezone)}
                    </span>
                  </div>
                  <div className="key-row">
                    <span>{t("Account source")}</span>
                    <span>{t(q.meta.source)}</span>
                  </div>
                  <div className="key-row">
                    <span>{t("Last successful quota read")}</span>
                    <span>
                      {date(q.meta.updated_at, s.timezone)} ·{" "}
                      {q.latency_ms ?? "—"} ms
                    </span>
                  </div>
                  <div className="key-row">
                    <span>{t("Next quota attempt")}</span>
                    <span>{date(q.next_attempt_at, s.timezone)}</span>
                  </div>
                  <p className="muted">
                    {t(
                      "Files are read incrementally. Oversized body lines are excluded from metadata storage. Parser errors remain visible here.",
                    )}{" "}
                  </p>
                </section>
                <section className="panel">
                  <h2>{t("Official account buckets")}</h2>
                  {q.buckets.map((b) => (
                    <div className="bucket" key={b.id}>
                      <h3>{b.name || b.id}</h3>
                      {[b.primary, b.secondary]
                        .filter((w): w is Window => w !== null)
                        .map((w, i) => (
                          <div className="key-row" key={i}>
                            <span>
                              {w.window_minutes ?? t("Unknown")}{" "}
                              {t("minute window")}{" "}
                            </span>
                            <b>
                              {w.used_percent}
                              {t("% used ·")} {w.remaining_percent}
                              {t("% remaining ·")}{" "}
                              {date(w.resets_at, s.timezone)}
                            </b>
                          </div>
                        ))}
                    </div>
                  ))}
                  <div className="key-row">
                    <span>{t("Credit balance reported")}</span>
                    <span>
                      {t(q.credits?.balance ?? "Unavailable")}{" "}
                      {t(q.credits?.unlimited ? "(unlimited)" : "")}
                    </span>
                  </div>
                  <div className="key-row">
                    <span>{t("Reset credits")}</span>
                    <span>
                      {q.reset_credits?.available_count ?? t("Unavailable")}
                    </span>
                  </div>
                  {q.reset_credits?.details?.map((c, i) => (
                    <div key={i} className="key-row">
                      <span>
                        {t(c.title || "Reset credit")} ·{" "}
                        {t(c.status || "Unknown")}
                      </span>
                      <span>
                        {t("Granted")} {date(c.granted_at, s.timezone)}{" "}
                        {t("· Expires")} {date(c.expires_at, s.timezone)}
                      </span>
                    </div>
                  ))}
                  <details>
                    <summary>
                      {t("Separate official account usage summary")}{" "}
                      <Badge status={q.usage_status} />
                    </summary>
                    <p className="muted">
                      {t(
                        "Account-wide totals are separate from the local Token accounting above.",
                      )}{" "}
                    </p>
                    <pre>
                      {t(
                        q.usage
                          ? JSON.stringify(q.usage, null, 2)
                          : "Unavailable",
                      )}
                    </pre>
                  </details>
                </section>
              </>
            )}
            <section className="export-bar">
              <div>
                <h3>{t("Take your data with you")}</h3>
                <span className="muted">
                  {t("Local reports, metadata only.")}
                </span>
              </div>
              <div className="export-actions">
                <select
                  aria-label={t("Export dataset")}
                  value={dataset}
                  onChange={(e) => setDataset(e.target.value)}
                >
                  <option value="daily">{t("Daily usage")}</option>
                  <option value="models">{t("Model usage")}</option>
                  <option value="sessions">{t("Session usage")}</option>
                  <option value="quota">{t("Quota history")}</option>
                </select>
                <select
                  aria-label={t("Export format")}
                  value={format}
                  onChange={(e) => setFormat(e.target.value)}
                >
                  <option value="html">HTML</option>
                  <option value="csv">CSV</option>
                  <option value="json">JSON</option>
                </select>
                <button
                  onClick={async () => {
                    try {
                      const path = await call<string>("export_report", {
                        range,
                        format,
                        dataset,
                      });
                      setToast(t("Report saved: {path}", { path: path }));
                    } catch (e) {
                      setToast(String(e));
                    }
                  }}
                >
                  {t("Export report")}{" "}
                </button>
                <button
                  onClick={() =>
                    call("open_exports").catch((e) => setToast(String(e)))
                  }
                >
                  {t("Show files")}{" "}
                </button>
              </div>
            </section>
          </>
        )}
        <footer className="page-footer">
          <span>{t("Local first · Read only · No telemetry")}</span>
          <span>
            {t(
              "Independent open-source project. Not affiliated with OpenAI.",
            )}{" "}
          </span>
        </footer>
      </main>
      {toast && (
        <div className="toast" role="status" onClick={() => setToast("")}>
          {t(toast)}
        </div>
      )}
      {detailOpen && (
        <DetailDialog
          detail={detail}
          onClose={closeDetail}
          mode={mode}
          tz={s.timezone}
        />
      )}
    </div>
  );
}
