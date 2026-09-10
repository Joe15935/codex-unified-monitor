import { useState, useEffect, useCallback, useRef } from "react";
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
  return <span className={`badge ${status.toLowerCase()}`}>{status}</span>;
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
      <span className="muted">{label}</span>
      <strong>{children}</strong>
      {note && <small>{note}</small>}
    </div>
  );
}
function useClock() {
  const [now, setNow] = useState(Date.now() / 1000);
  useEffect(() => {
    const timer = setInterval(() => {
      if (!document.hidden) setNow(Date.now() / 1000);
    }, 1000);
    return () => clearInterval(timer);
  }, []);
  return now;
}
function QuotaCard({
  title,
  window: w,
  quota,
  tz,
  now,
}: {
  title: string;
  window: Window | null;
  quota: Quota;
  tz: string;
  now: number;
}) {
  const live = quota.meta.status === "LIVE";
  return (
    <section className={`kpi ${!live ? "muted-kpi" : ""}`}>
      <div className="card-top">
        <span>{title}</span>
        <Badge status={w ? quota.meta.status : "UNAVAILABLE"} />
      </div>
      {w ? (
        <>
          <div className="quota-number">
            {w.remaining_percent.toFixed(0)}
            <span>% remaining</span>
          </div>
          <div
            className="meter"
            role="meter"
            aria-label={`${title} used`}
            aria-valuenow={w.used_percent}
            aria-valuemin={0}
            aria-valuemax={100}
          >
            <i style={{ width: `${w.used_percent}%` }} />
          </div>
          <div className="card-foot">
            <span>{w.used_percent.toFixed(0)}% used</span>
            <span title={date(w.resets_at, tz)}>
              {w.resets_at
                ? `Reset ${duration(w.resets_at - now)}`
                : "Reset unavailable"}
            </span>
          </div>
          <small className="reset-date">{date(w.resets_at, tz)}</small>
        </>
      ) : (
        <>
          <div className="quota-number">—</div>
          <p className="muted">
            This window was not returned by the account service.
          </p>
        </>
      )}
    </section>
  );
}
function TokenStrip({ a }: { a: Aggregate }) {
  return (
    <div className="token-strip">
      <Stat label="Total tokens" note="Raw input + output">
        {count(a.tokens.total_tokens)}
      </Stat>
      <Stat label="Fresh input">{count(a.tokens.uncached_input_tokens)}</Stat>
      <Stat label="Cached input">{count(a.tokens.cached_input_tokens)}</Stat>
      <Stat
        label="Output"
        note={`${count(a.tokens.reasoning_output_tokens)} reasoning, included`}
      >
        {count(a.tokens.output_tokens)}
      </Stat>
      <Stat label="Cache hit" note="Cached / raw input">
        {percent(ratio(a))}
      </Stat>
    </div>
  );
}
function BurnPanel({ b, title, tz }: { b: Burn; title: string; tz: string }) {
  return (
    <section className="panel burn">
      <div className="card-top">
        <h3>{title}</h3>
        <Badge status="ESTIMATED" />
      </div>
      {b.percent_per_hour == null ? (
        <>
          <strong>{b.status}</strong>
          <p className="muted">
            {b.status.toLowerCase() === "unavailable"
              ? "The account provider has not supplied this quota window."
              : `${b.samples} samples · Requires 4 readings over at least 30 minutes and a measurable quota change.`}
          </p>
        </>
      ) : (
        <>
          <strong>
            {title.startsWith("Weekly")
              ? `${b.percent_per_day?.toFixed(1)}% / day`
              : `${b.percent_per_hour.toFixed(1)}% / hour`}
          </strong>
          <p>
            {b.resets_before_exhaustion
              ? "Window resets before projected exhaustion."
              : `Projected exhaustion ${date(b.exhausted_at, tz)}`}
          </p>
          <div className="observed">
            <span>Observed efficiency / 1% quota</span>
            <b>
              {b.tokens_per_percent == null ? "—" : count(b.tokens_per_percent)}{" "}
              tokens · {usd(b.equivalent_usd_per_percent)}
            </b>
            <small>
              {b.output_per_percent == null ? "—" : count(b.output_per_percent)}{" "}
              output tokens · {b.samples} readings over{" "}
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
  const filtered = rows
    .filter((s) =>
      `${s.id} ${s.project} ${s.models.join(" ")}`
        .toLowerCase()
        .includes(search.toLowerCase()),
    )
    .sort((a, b) => key(b) - key(a));
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
      {label}
      {sort === name ? " ↓" : ""}
    </button>
  );
  return (
    <>
      <div className="table-controls">
        <input
          aria-label="Search sessions"
          placeholder="Find a project, session or model…"
          value={search}
          onChange={(e) => {
            setSearch(e.target.value);
            setPage(0);
          }}
        />
        <select
          aria-label="Session ranking"
          value={sort}
          onChange={(e) => {
            setSort(e.target.value);
            setPage(0);
          }}
        >
          <option value="value">Top API equivalent</option>
          <option value="tokens">Most tokens</option>
          <option value="cache">Lowest cache hit</option>
          <option value="longest">Longest sessions</option>
          <option value="output">Most output</option>
        </select>
      </div>
      <div className="table-wrap">
        <table className="session-table">
          <thead>
            <tr>
              <th>Session / project</th>
              <th>Models / reasoning</th>
              <th>{sortButton("tokens", "Tokens")}</th>
              <th>Fresh / cached</th>
              <th>{sortButton("output", "Output")}</th>
              <th>{sortButton("cache", "Cache hit")}</th>
              <th>{sortButton("value", "Equivalent $")}</th>
              <th>Turns / tools</th>
              <th>{sortButton("longest", "Duration")}</th>
              <th>Started / last active</th>
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
                      "Codex session"}
                  </button>
                  <small>
                    {s.id.slice(0, 8)}…{s.id.slice(-6)}
                  </small>
                </td>
                <td>
                  {s.models.join(", ") || "—"}
                  <small>{s.efforts.join(", ") || "Unknown"}</small>
                </td>
                <td>{count(s.aggregate.tokens.total_tokens)}</td>
                <td>
                  {count(s.aggregate.tokens.uncached_input_tokens)}
                  <small>
                    {count(s.aggregate.tokens.cached_input_tokens)} cached
                  </small>
                </td>
                <td>{count(s.aggregate.tokens.output_tokens)}</td>
                <td>{percent(ratio(s.aggregate))}</td>
                <td title={priceNote}>
                  {usd(value(money(s.aggregate, mode)))}
                  {money(s.aggregate, mode).unpriced_tokens > 0 && (
                    <small className="amber">Partly unpriced</small>
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
          <div className="empty-chart">No sessions match this view.</div>
        )}
      </div>
      <div className="pagination">
        <span>{filtered.length} sessions · metadata only</span>
        <div>
          <button onClick={() => setPage(current - 1)} disabled={current === 0}>
            Previous
          </button>
          <span>
            {current + 1} / {Math.max(1, Math.ceil(filtered.length / limit))}
          </span>
          <button
            onClick={() => setPage(current + 1)}
            disabled={(current + 1) * limit >= filtered.length}
          >
            Next
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
          <span className="eyebrow">SESSION DETAILS</span>
          <h2>{detail?.id || "Loading session…"}</h2>
        </div>
        <button onClick={onClose} aria-label="Close session details">
          Close
        </button>
      </div>
      {detail && (
        <>
          <TokenStrip a={detail.aggregate} />
          <div className="inline-summary">
            <b>API equivalent {usd(value(money(detail.aggregate, mode)))}</b>
            <span>
              {percent(coverage(money(detail.aggregate, mode)))} priced
            </span>
          </div>
          <Trend points={detail.timeline} mode={mode} />
          <div className="two-col">
            <section>
              <h3>Model & reasoning changes</h3>
              {detail.model_changes.map(([t, m, e], i) => (
                <p key={i}>
                  <small>{date(t, tz)}</small>
                  <br />
                  {m} · {e || "Unknown effort"}
                </p>
              ))}
            </section>
            <section>
              <h3>Tool calls</h3>
              {detail.tools.map(([tool, n]) => (
                <div className="key-row" key={tool}>
                  <span>{tool}</span>
                  <b>{n}</b>
                </div>
              ))}
            </section>
          </div>
          <p className="muted">
            Only counters and metadata are stored. Prompts, reasoning text and
            tool outputs are excluded.
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
        <h2>Preferences</h2>
        <label>
          Menu bar metric
          <select
            value={draft.tray_metric}
            onChange={(e) => set("tray_metric", e.target.value)}
          >
            <option value="weekly">Weekly remaining</option>
            <option value="five_hour">5-hour remaining</option>
            <option value="today_value">Today API equivalent</option>
            <option value="today_tokens">Today tokens</option>
          </select>
        </label>
        <label>
          Time zone
          <input
            value={draft.timezone}
            onChange={(e) => set("timezone", e.target.value)}
          />
          <small>
            Calendar days and Monday-start weeks use this time zone.
          </small>
        </label>
        <label>
          Quota refresh
          <select
            value={draft.quota_poll_seconds}
            onChange={(e) => set("quota_poll_seconds", Number(e.target.value))}
          >
            {[60, 90, 120, 300, 600].map((n) => (
              <option key={n} value={n}>
                {n} seconds
              </option>
            ))}
          </select>
        </label>
        <label>
          Appearance
          <select
            value={draft.theme}
            onChange={(e) => set("theme", e.target.value)}
          >
            <option value="system">System</option>
            <option value="light">Light</option>
            <option value="dark">Dark</option>
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
          Launch at Login
        </label>
        <label className="check">
          <input
            type="checkbox"
            checked={draft.account_enabled}
            onChange={(e) => set("account_enabled", e.target.checked)}
          />
          Read live account quota
        </label>
        <small>
          Turning this off disconnects this monitor. It does not sign you out of
          Codex.
        </small>
      </section>
      <section className="panel">
        <h2>Equivalent value</h2>
        <label>
          Pricing basis
          <select
            value={draft.pricing_mode}
            onChange={(e) => set("pricing_mode", e.target.value)}
          >
            <option value="public_api">Public API Equivalent</option>
            <option value="codex_work">
              Codex / Work Rate-Card Equivalent
            </option>
          </select>
        </label>
        <label>
          Monthly subscription cost (USD)
          <input
            type="number"
            min="0.01"
            step="0.01"
            placeholder="Optional; enter your own price"
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
          Enables the Equivalent Value Multiple and break-even comparison. No
          subscription price is assumed.
        </p>
        <h3>Cache health thresholds</h3>
        <div className="three-col">
          {["Excellent ≥", "Good ≥", "Average ≥"].map((x, i) => (
            <label key={x}>
              {x}
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
          Below the Average threshold is Poor. Ratios are weighted by raw input
          tokens.
        </small>
        <h3>Model aliases</h3>
        <div className="alias-row">
          <input
            aria-label="Unknown model alias"
            placeholder="Unknown model name"
            value={alias}
            onChange={(e) => setAlias(e.target.value)}
          />
          <select
            aria-label="Alias target"
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
            Add
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
              Remove
            </button>
          </div>
        ))}
      </section>
      <section className="panel full">
        <h2>Custom model rates</h2>
        <p className="muted">
          USD per 1 million tokens, for the selected pricing basis. These are
          explicitly user supplied estimates.
        </p>
        <div className="rate-form">
          <label>
            Model
            <input
              value={model}
              onChange={(e) => setModel(e.target.value)}
              placeholder="Exact model name"
            />
          </label>
          {["Fresh input", "Cached input", "Output"].map((x, i) => (
            <label key={x}>
              {x}
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
            Add rate
          </button>
        </div>
        {draft.custom_rates.map((p, i) => (
          <div className="key-row" key={`${p.model}:${p.pricing_mode}`}>
            <span>
              {p.model} · {p.pricing_mode} · {p.input} / {p.cached_input} /{" "}
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
              Remove
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
          {busy ? "Saving…" : "Save preferences"}
        </button>
      </div>
    </div>
  );
}
function Compact({
  data,
  now,
  reload,
  onToast,
}: {
  data: Data;
  now: number;
  reload: () => void;
  onToast: (s: string) => void;
}) {
  const q = data.quota;
  const a = data.report.periods.today;
  const mode = data.settings.pricing_mode;
  const row = (title: string, w: Window | null) => (
    <div className="compact-row">
      <span>{title}</span>
      <b>
        {w ? `${w.remaining_percent.toFixed(0)}% remaining` : "Unavailable"}
      </b>
      <small>
        {w?.resets_at
          ? `reset ${duration(w.resets_at - now)}`
          : "Window not returned"}
      </small>
    </div>
  );
  return (
    <div className="compact">
      <header>
        <TokenGlyph />
        <div>
          <h2>Codex</h2>
          <small>Unified Monitor</small>
        </div>
        <Badge status={q.meta.status} />
      </header>
      {row("5-hour", q.five_hour)}
      {row("Weekly", q.weekly)}
      <div className="compact-pair">
        <Stat label="Today equivalent">{usd(value(money(a, mode)))}</Stat>
        <Stat label="Cache hit">{percent(ratio(a))}</Stat>
      </div>
      <div className="key-row">
        <span>Reset credits</span>
        <b>{q.reset_credits?.available_count ?? "Unavailable"}</b>
      </div>
      <small className="muted">
        {q.error ||
          `${q.meta.updated_at ? `Last quota update ${duration(now - q.meta.updated_at)} ago` : "Connecting to account…"}`}
      </small>
      <button
        className="primary wide"
        onClick={() => call("open_dashboard").catch((e) => onToast(String(e)))}
      >
        Open Dashboard
      </button>
      <footer>
        <button
          onClick={async () => {
            await call("refresh");
            reload();
          }}
        >
          Refresh
        </button>
        <button onClick={() => call("open_dashboard", { tab: "settings" })}>
          Settings
        </button>
        <button onClick={() => call("quit")}>Quit</button>
      </footer>
    </div>
  );
}
export default function App() {
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
  const request = useRef(0);
  const now = useClock();
  const reload = useCallback(async () => {
    const id = ++request.current;
    try {
      const d = await call<Data>("dashboard", { range });
      if (id === request.current) {
        setData(d);
        setError("");
      }
    } catch (e) {
      if (id === request.current) setError(String(e));
    }
  }, [range]);
  useEffect(() => {
    void reload();
    if (!native) return;
    let timer: ReturnType<typeof setTimeout>;
    const unsub = listen("monitor-updated", () => {
      clearTimeout(timer);
      timer = setTimeout(() => {
        if (!document.hidden) void reload();
      }, 350);
    });
    const nav = listen<string>("navigate", (e) => setTab(e.payload));
    const visible = () => {
      if (!document.hidden) void reload();
    };
    document.addEventListener("visibilitychange", visible);
    return () => {
      clearTimeout(timer);
      void unsub.then((f) => f());
      void nav.then((f) => f());
      document.removeEventListener("visibilitychange", visible);
    };
  }, [reload]);
  useEffect(() => {
    if (toast) {
      const t = setTimeout(() => setToast(""), 6500);
      return () => clearTimeout(t);
    }
  }, [toast]);
  useEffect(() => {
    document.documentElement.dataset.theme = data?.settings.theme || "system";
  }, [data?.settings.theme]);
  const openSession = async (id: string) => {
    setDetailOpen(true);
    setDetail(null);
    try {
      setDetail(await call<Detail>("session_detail", { id }));
    } catch (e) {
      setToast(String(e));
      setDetailOpen(false);
    }
  };
  if (!data)
    return (
      <div className="loading">
        <TokenGlyph />
        <h1>Codex Unified Monitor</h1>
        <p>{error || "Reading local counters and preparing your dashboard…"}</p>
        {error && <button onClick={() => void reload()}>Retry</button>}
        <small>Local first · Read only · No telemetry</small>
      </div>
    );
  if (compact)
    return (
      <Compact
        data={data}
        now={now}
        reload={() => void reload()}
        onToast={setToast}
      />
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
            Codex<span className="brand-sub">UNIFIED MONITOR</span>
          </span>
        </div>
        <nav aria-label="Main navigation">
          {[
            ["overview", "Overview"],
            ["sessions", "Sessions"],
            ["cache", "Cache health"],
            ["pricing", "API equivalent"],
            ["diagnostics", "Data health"],
            ["settings", "Settings"],
          ].map(([id, label]) => (
            <button
              className={tab === id ? "active" : ""}
              aria-current={tab === id ? "page" : undefined}
              key={id}
              onClick={() => setTab(id)}
            >
              {label}
            </button>
          ))}
        </nav>
        <div className="sidebar-bottom">
          <span className="local-mark">Local first</span>
          <p>
            Read only.
            <br />
            Your data stays here.
          </p>
          <small>v{data.version} · MIT</small>
        </div>
      </aside>
      <main>
        <header className="topbar">
          <div>
            <h1>
              {
                {
                  overview: "Usage overview",
                  sessions: "Session analytics",
                  cache: "Cache health",
                  pricing: "API equivalent value",
                  diagnostics: "Data health",
                  settings: "Settings",
                }[tab]
              }
            </h1>
            <div className="status-line">
              <span
                className={`status-dot ${q.meta.status === "LIVE" ? "live" : ""}`}
              />
              <span>
                {q.meta.status === "LIVE"
                  ? "Live quota"
                  : `Quota ${q.meta.status.toLowerCase()}`}
                {q.meta.updated_at
                  ? ` · ${duration(now - q.meta.updated_at)} ago`
                  : ""}
              </span>
              <span className="divider">/</span>
              <span>Local data · {r.diagnostics.scan_status}</span>
            </div>
          </div>
          <div className="top-actions">
            <span className="account" title={q.account_label || ""}>
              {q.plan?.toUpperCase() || "ACCOUNT"}
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
              Refresh
            </button>
          </div>
        </header>
        {error && (
          <div className="notice error" role="alert">
            {error}
          </div>
        )}
        {q.meta.status !== "LIVE" && (
          <div className="notice" role="status">
            Live quota unavailable. {q.error}{" "}
            {q.meta.updated_at &&
              `Last successful reading: ${date(q.meta.updated_at, s.timezone)}.`}
          </div>
        )}
        {tab === "settings" ? (
          <Preferences data={data} onSave={save} onToast={setToast} />
        ) : (
          <>
            {tab === "overview" && (
              <>
                <div className="kpi-grid">
                  <QuotaCard
                    title="5-Hour"
                    window={q.five_hour}
                    quota={q}
                    tz={s.timezone}
                    now={now}
                  />
                  <QuotaCard
                    title="Weekly"
                    window={q.weekly}
                    quota={q}
                    tz={s.timezone}
                    now={now}
                  />
                  <section className="kpi">
                    <div className="card-top">
                      <span>Reset credits</span>
                      <Badge
                        status={q.reset_credits ? q.meta.status : "UNAVAILABLE"}
                      />
                    </div>
                    <div className="quota-number">
                      {q.reset_credits?.available_count ?? "—"}
                      <span>available</span>
                    </div>
                    <p className="muted">
                      {q.reset_credits?.details?.some((c) => c.expires_at)
                        ? `Earliest expiry ${date(Math.min(...q.reset_credits.details.flatMap((c) => (c.expires_at ? [c.expires_at] : []))), s.timezone)}`
                        : q.reset_credits?.details === null
                          ? "Grant / expiry details unavailable"
                          : q.reset_credits?.available_count === 0
                            ? "No reset credits currently available."
                            : "Expiry unavailable"}
                    </p>
                  </section>
                  <section className="kpi value-kpi" title={priceNote}>
                    <div className="card-top">
                      <span>API equivalent</span>
                      <Badge status="ESTIMATED" />
                    </div>
                    <div className="quota-number">
                      {usd(value(money(today, mode)))}
                    </div>
                    <div className="card-foot">
                      <span>Today</span>
                      <span>Month {usd(monthValue)}</span>
                    </div>
                    <small className="reset-date">
                      Base-rate equivalent estimate
                    </small>
                  </section>
                </div>
                <TokenStrip a={a} />
              </>
            )}
            <div className="rangebar">
              <div className="segmented" role="group" aria-label="Time range">
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
                    {label}
                  </button>
                ))}
              </div>
              <span className="muted">{s.timezone}</span>
            </div>
            {range.period === "custom" && (
              <div className="custom-range">
                <label>
                  From
                  <input
                    type="date"
                    value={customStart}
                    onChange={(e) => setCustomStart(e.target.value)}
                  />
                </label>
                <label>
                  Through
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
                  Apply dates
                </button>
              </div>
            )}
            {tab === "overview" && (
              <>
                <section className="panel">
                  <div className="section-heading">
                    <div>
                      <h2>Usage over time</h2>
                      <p className="muted">
                        {exact(a.tokens.total_tokens)} tokens across{" "}
                        {a.responses} recorded responses
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
                          {label}
                        </button>
                      ))}
                    </div>
                  </div>
                  <Trend points={r.trend} mode={mode} metric={metric} />
                  <div className="legend">
                    <span>
                      <i className="fresh" />
                      Fresh input
                    </span>
                    <span>
                      <i className="cached" />
                      Cached input
                    </span>
                    <span>
                      <i className="out" />
                      Output
                    </span>
                    <small>{percent(coverage(m))} priced</small>
                  </div>
                </section>
                <section className="panel">
                  <div className="section-heading">
                    <h2>Model breakdown</h2>
                    <span className="muted">
                      One accounting model, every view
                    </span>
                  </div>
                  <div className="table-wrap">
                    <table>
                      <thead>
                        <tr>
                          <th>Model</th>
                          <th>Tokens</th>
                          <th>Fresh input</th>
                          <th>Cached</th>
                          <th>Output</th>
                          <th>Cache hit</th>
                          <th>Equivalent $</th>
                          <th>Share</th>
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
                    title="5-hour burn rate"
                    b={data.burn.five_hour}
                    tz={s.timezone}
                  />
                  <BurnPanel
                    title="Weekly burn rate"
                    b={data.burn.weekly}
                    tz={s.timezone}
                  />
                </div>
                <p className="footnote">
                  Observed efficiency compares account-wide quota changes with
                  logs on this Mac. It is a local correlation, not OpenAI’s
                  billing formula.
                </p>
                <section className="panel">
                  <div className="section-heading">
                    <h2>Sessions worth a look</h2>
                    <button onClick={() => setTab("sessions")}>
                      All sessions
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
                  <h2>Find where the tokens went</h2>
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
                    <span className="eyebrow">CACHE HIT RATIO</span>
                    <h2>{percent(cache)}</h2>
                    <span className="quality">{quality}</span>
                    <p>Weighted by input tokens across the selected period.</p>
                  </div>
                  <div>
                    <Stat label="Cached input / saved input tokens">
                      {count(a.tokens.cached_input_tokens)}
                    </Stat>
                    <Stat label="Fresh input">
                      {count(a.tokens.uncached_input_tokens)}
                    </Stat>
                    <Stat
                      label="Cache value saved"
                      note="Additional base-rate API equivalent without cache hits"
                    >
                      {m.priced_tokens === 0 && m.unpriced_tokens > 0
                        ? "Unpriced"
                        : usd(m.cache_savings_usd)}
                    </Stat>
                  </div>
                </div>
                <div className="period-cache">
                  {["today", "5h", "week", "month", "all"].map((p) => (
                    <section key={p} className="panel">
                      <span>{periods.find(([id]) => id === p)?.[1]}</span>
                      <strong>{percent(ratio(r.periods[p]))}</strong>
                    </section>
                  ))}
                </div>
                <section className="panel">
                  <h2>Cache trend</h2>
                  <Trend points={r.trend} mode={mode} metric="cache" />
                </section>
                <section className="panel">
                  <h2>Cache by session</h2>
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
                      <h2>API equivalent value</h2>
                      <Badge status="ESTIMATED" />
                    </div>
                    <strong>{usd(value(m))}</strong>
                    <p>
                      {mode === "codex_work"
                        ? "Codex / Work Rate-Card Equivalent"
                        : "Public API Equivalent"}
                    </p>
                    <select
                      aria-label="Pricing basis"
                      value={mode}
                      onChange={(e) =>
                        save({ ...s, pricing_mode: e.target.value })
                      }
                    >
                      <option value="public_api">Public API Equivalent</option>
                      <option value="codex_work">
                        Codex / Work Rate-Card Equivalent
                      </option>
                    </select>
                    <p className="muted">{priceNote}</p>
                  </section>
                  <section className="panel">
                    <h2>Pricing coverage</h2>
                    <div className="coverage-number">
                      {percent(coverage(m))}
                    </div>
                    <p>
                      {count(m.priced_tokens)} priced tokens ·{" "}
                      {count(m.unpriced_tokens)} unpriced
                    </p>
                    <p className="muted">
                      Unknown models stay unpriced until you explicitly assign
                      an alias or custom rate.
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
                      Manage model rates
                    </button>
                  </section>
                </div>
                <section className="panel">
                  <h2>Subscription comparison</h2>
                  {s.monthly_subscription_cost ? (
                    <>
                      <div className="token-strip">
                        <Stat label="Monthly subscription">
                          {usd(s.monthly_subscription_cost)}
                        </Stat>
                        <Stat label="This month equivalent">
                          {usd(monthValue)}
                        </Stat>
                        <Stat label="Equivalent Value Multiple">
                          {monthValue == null
                            ? "—"
                            : `${(monthValue / s.monthly_subscription_cost).toFixed(2)}×`}
                        </Stat>
                        <Stat label="Break-even progress">
                          {monthValue == null
                            ? "—"
                            : `${((monthValue / s.monthly_subscription_cost) * 100).toFixed(0)}%`}
                        </Stat>
                      </div>
                      <small>
                        Based on the priced portion of this month’s local logs.
                      </small>
                    </>
                  ) : (
                    <p className="muted">
                      Enter your monthly subscription cost in Settings to enable
                      this optional comparison.
                    </p>
                  )}
                </section>
                <section className="panel">
                  <div className="section-heading">
                    <h2>Verified rate catalog</h2>
                    <span className="muted">
                      USD / 1M tokens · verified {r.catalog.verified_at}
                    </span>
                  </div>
                  <div className="table-wrap">
                    <table>
                      <thead>
                        <tr>
                          <th>Model</th>
                          <th>Input</th>
                          <th>Cached</th>
                          <th>Output</th>
                          <th>Source / verified</th>
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
                                  Official source
                                </a>
                                <small>{p.verified_at}</small>
                              </td>
                            </tr>
                          ))}
                      </tbody>
                    </table>
                  </div>
                  <p className="muted">
                    Current catalog rates are applied to selected history.
                    Promotional rates, historical price changes, cache writes
                    and service adjustments may differ.
                  </p>
                  <label className="catalog-import">
                    Import an updated pricing_catalog.json
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
                    <h2>Local data pipeline</h2>
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
                    <span>Local source</span>
                    <code>~/.codex/sessions + archived_sessions</code>
                  </div>
                  <div className="key-row">
                    <span>Last ingest</span>
                    <span>
                      {date(r.diagnostics.last_local_update, s.timezone)}
                    </span>
                  </div>
                  <div className="key-row">
                    <span>Account source</span>
                    <span>{q.meta.source}</span>
                  </div>
                  <div className="key-row">
                    <span>Last successful quota read</span>
                    <span>
                      {date(q.meta.updated_at, s.timezone)} ·{" "}
                      {q.latency_ms ?? "—"} ms
                    </span>
                  </div>
                  <div className="key-row">
                    <span>Next quota attempt</span>
                    <span>{date(q.next_attempt_at, s.timezone)}</span>
                  </div>
                  <p className="muted">
                    Files are read incrementally. Oversized body lines are
                    excluded from metadata storage. Parser errors remain visible
                    here.
                  </p>
                </section>
                <section className="panel">
                  <h2>Official account buckets</h2>
                  {q.buckets.map((b) => (
                    <div className="bucket" key={b.id}>
                      <h3>{b.name || b.id}</h3>
                      {[b.primary, b.secondary]
                        .filter((w): w is Window => w !== null)
                        .map((w, i) => (
                          <div className="key-row" key={i}>
                            <span>
                              {w.window_minutes ?? "Unknown"} minute window
                            </span>
                            <b>
                              {w.used_percent}% used · {w.remaining_percent}%
                              remaining · {date(w.resets_at, s.timezone)}
                            </b>
                          </div>
                        ))}
                    </div>
                  ))}
                  <div className="key-row">
                    <span>Credit balance reported</span>
                    <span>
                      {q.credits?.balance ?? "Unavailable"}{" "}
                      {q.credits?.unlimited ? "(unlimited)" : ""}
                    </span>
                  </div>
                  <div className="key-row">
                    <span>Reset credits</span>
                    <span>
                      {q.reset_credits?.available_count ?? "Unavailable"}
                    </span>
                  </div>
                  {q.reset_credits?.details?.map((c, i) => (
                    <div key={i} className="key-row">
                      <span>
                        {c.title || "Reset credit"} · {c.status || "Unknown"}
                      </span>
                      <span>
                        Granted {date(c.granted_at, s.timezone)} · Expires{" "}
                        {date(c.expires_at, s.timezone)}
                      </span>
                    </div>
                  ))}
                  <details>
                    <summary>
                      Separate official account usage summary{" "}
                      <Badge status={q.usage_status} />
                    </summary>
                    <p className="muted">
                      Account-wide totals are separate from the local Token
                      accounting above.
                    </p>
                    <pre>
                      {q.usage
                        ? JSON.stringify(q.usage, null, 2)
                        : "Unavailable"}
                    </pre>
                  </details>
                </section>
              </>
            )}
            <section className="export-bar">
              <div>
                <h3>Take your data with you</h3>
                <span className="muted">Local reports, metadata only.</span>
              </div>
              <div className="export-actions">
                <select
                  aria-label="Export dataset"
                  value={dataset}
                  onChange={(e) => setDataset(e.target.value)}
                >
                  <option value="daily">Daily usage</option>
                  <option value="models">Model usage</option>
                  <option value="sessions">Session usage</option>
                  <option value="quota">Quota history</option>
                </select>
                <select
                  aria-label="Export format"
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
                      setToast(`Report saved: ${path}`);
                    } catch (e) {
                      setToast(String(e));
                    }
                  }}
                >
                  Export report
                </button>
                <button
                  onClick={() =>
                    call("open_exports").catch((e) => setToast(String(e)))
                  }
                >
                  Show files
                </button>
              </div>
            </section>
          </>
        )}
        <footer className="page-footer">
          <span>Local first · Read only · No telemetry</span>
          <span>
            Independent open-source project. Not affiliated with OpenAI.
          </span>
        </footer>
      </main>
      {toast && (
        <div className="toast" role="status" onClick={() => setToast("")}>
          {toast}
        </div>
      )}
      {detailOpen && (
        <DetailDialog
          detail={detail}
          onClose={() => setDetailOpen(false)}
          mode={mode}
          tz={s.timezone}
        />
      )}
    </div>
  );
}
