import type { Meta, Settings, Window } from "./types";

/** Compare a reading with its own cycle timestamp, never a stale value with today's clock. */
export function quotaInsight(
  window: Window | null,
  meta: Meta,
  now: number,
  settings: Pick<
    Settings,
    "quota_poll_seconds" | "adaptive_refresh" | "low_quota_threshold"
  >,
) {
  const base = settings.quota_poll_seconds;
  const freshFor = settings.adaptive_refresh
    ? Math.max(base * 3, Math.max(base, 300) + base)
    : base * 3;
  const at = meta.updated_at;
  if (
    !window ||
    meta.status !== "LIVE" ||
    at == null ||
    !Number.isFinite(at) ||
    !Number.isFinite(now) ||
    at > now ||
    now - at > freshFor ||
    !Number.isFinite(window.used_percent) ||
    window.used_percent < 0 ||
    window.used_percent > 100
  )
    return null;
  const reset = window.resets_at;
  if (reset != null && (!Number.isFinite(reset) || reset <= now)) return null;
  const remaining = 100 - window.used_percent;
  const low =
    settings.low_quota_threshold > 0 &&
    remaining <= settings.low_quota_threshold;
  const minutes = window.window_minutes;
  let elapsed: number | null = null;
  if (
    reset != null &&
    minutes != null &&
    Number.isFinite(minutes) &&
    minutes > 0
  ) {
    const started = reset - minutes * 60;
    if (at >= started && at < reset)
      elapsed = ((at - started) / (minutes * 60)) * 100;
  }
  const difference = elapsed == null ? null : window.used_percent - elapsed;
  const pace =
    difference == null
      ? null
      : difference > 5
        ? "ahead"
        : difference < -5
          ? "below"
          : "near";
  return { remaining, low, elapsed, pace };
}
