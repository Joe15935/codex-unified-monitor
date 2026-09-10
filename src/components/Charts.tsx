import type { Point } from "../types";
import { count, money, value, ratio, percent, usd } from "../data";
// TokenGlyph and stacked-column structure adapted from CodexScope / HduSy tokenscope, MIT.
export function TokenGlyph() {
  return (
    <svg width="27" height="27" viewBox="0 0 14 14" aria-hidden="true">
      <rect
        x=".6"
        y=".6"
        width="12.8"
        height="12.8"
        rx="3.2"
        fill="none"
        stroke="currentColor"
        strokeWidth="1.3"
      />
      <rect
        x="3"
        y="7.5"
        width="1.7"
        height="3.2"
        rx=".6"
        fill="currentColor"
      />
      <rect
        x="6.15"
        y="5"
        width="1.7"
        height="5.7"
        rx=".6"
        fill="currentColor"
      />
      <rect
        x="9.3"
        y="3"
        width="1.7"
        height="7.7"
        rx=".6"
        fill="currentColor"
      />
    </svg>
  );
}
export function Trend({
  points,
  mode,
  metric = "tokens",
}: {
  points: Point[];
  mode: string;
  metric?: string;
}) {
  if (!points.length)
    return <div className="empty-chart">No token events in this period.</div>;
  const n = (p: Point) =>
    metric === "value"
      ? (value(money(p.aggregate, mode)) ?? 0)
      : metric === "cache"
        ? (ratio(p.aggregate) ?? 0)
        : p.aggregate.tokens.total_tokens;
  const max = Math.max(...points.map(n), 1e-9);
  const label = (v: number) =>
    metric === "value" ? usd(v) : metric === "cache" ? percent(v) : count(v);
  return (
    <div
      className="chart"
      role="img"
      aria-label={`${metric} trend across ${points.length} recorded time buckets`}
    >
      <div className="chart-scale">
        <span>{label(max)}</span>
        <span>{label(max / 2)}</span>
        <span>0</span>
      </div>
      <div className="chart-body">
        <div className="gridlines">
          <i />
          <i />
          <i />
        </div>
        <div className="columns">
          {points.map((p) => {
            const a = p.aggregate;
            const t = a.tokens;
            const total = n(p);
            const stack = metric === "tokens";
            const title = `${p.label}\n${count(t.total_tokens)} tokens · ${percent(ratio(a))} cache hit\nAPI equivalent ${usd(value(money(a, mode)))}`;
            return (
              <button
                key={p.timestamp}
                className="column"
                aria-label={title}
                title={title}
                style={{
                  height: `${Math.max((total / max) * 100, total ? 1 : 0)}%`,
                }}
              >
                {stack ? (
                  <>
                    <i className="out" style={{ flex: t.output_tokens }} />
                    <i
                      className="cached"
                      style={{ flex: t.cached_input_tokens }}
                    />
                    <i
                      className="fresh"
                      style={{ flex: t.uncached_input_tokens }}
                    />
                  </>
                ) : (
                  <i
                    className={metric === "cache" ? "cached" : "fresh"}
                    style={{ flex: 1 }}
                  />
                )}
              </button>
            );
          })}
        </div>
        <div className="chart-labels">
          <span>{points[0].label}</span>
          <span>{points[Math.floor(points.length / 2)].label}</span>
          <span>{points[points.length - 1]?.label}</span>
        </div>
      </div>
    </div>
  );
}
