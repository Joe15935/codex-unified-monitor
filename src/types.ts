export interface Tokens {
  raw_input_tokens: number;
  cached_input_tokens: number;
  uncached_input_tokens: number;
  output_tokens: number;
  reasoning_output_tokens: number;
  total_tokens: number;
  cache_write_input_tokens: number;
}
export interface Money {
  known_value_usd: number;
  priced_tokens: number;
  unpriced_tokens: number;
  cache_savings_usd: number;
}
export interface Aggregate {
  tokens: Tokens;
  public_api: Money;
  codex_work: Money;
  responses: number;
  tool_calls: number;
}
export interface Range {
  period: string;
  start?: string;
  end?: string;
}
export interface Point {
  timestamp: number;
  label: string;
  aggregate: Aggregate;
}
export interface ModelRow {
  model: string;
  aggregate: Aggregate;
}
export interface SessionRow {
  id: string;
  project: string;
  started_at: number;
  last_active: number;
  models: string[];
  efforts: string[];
  turns: string[];
  aggregate: Aggregate;
}
export interface Price {
  model: string;
  input: number;
  cached_input: number;
  output: number;
  cache_write: number | null;
  source: string;
  effective_date: string | null;
  verified_at: string;
  pricing_mode: string;
  processing: string;
  context: string;
  notes: string;
}
export interface Settings {
  language: "en" | "zh-CN";
  timezone: string;
  quota_poll_seconds: number;
  adaptive_refresh: boolean;
  low_quota_threshold: number;
  tray_metric: string;
  pricing_mode: string;
  monthly_subscription_cost: number | null;
  cache_thresholds: number[];
  aliases: Record<string, string>;
  custom_rates: Price[];
  account_enabled: boolean;
  theme: string;
}
export interface Meta {
  source: string;
  updated_at: number | null;
  status: string;
  confidence: string;
}
export interface Window {
  used_percent: number;
  remaining_percent: number;
  window_minutes: number | null;
  resets_at: number | null;
}
export interface Credit {
  status: string | null;
  reset_type: string | null;
  granted_at: number | null;
  expires_at: number | null;
  title: string | null;
}
export interface Quota {
  account_key?: string;
  meta: Meta;
  account_label: string | null;
  plan: string | null;
  five_hour: Window | null;
  weekly: Window | null;
  buckets: {
    id: string;
    name: string | null;
    primary: Window | null;
    secondary: Window | null;
  }[];
  reset_credits: { available_count: number; details: Credit[] | null } | null;
  credits: {
    balance: string | null;
    hasCredits: boolean;
    unlimited: boolean;
  } | null;
  error: string | null;
  latency_ms: number | null;
  usage: unknown;
  usage_status: string;
  next_attempt_at: number | null;
}
export interface Burn {
  status: string;
  samples: number;
  observed_seconds: number;
  percent_per_hour: number | null;
  percent_per_day: number | null;
  exhausted_at: number | null;
  resets_before_exhaustion: boolean;
  tokens_per_percent: number | null;
  output_per_percent: number | null;
  equivalent_usd_per_percent: number | null;
  pricing_coverage: number | null;
}
export interface Report {
  meta: Meta;
  range: Range;
  timezone: string;
  start: number;
  end: number;
  total: Aggregate;
  periods: Record<string, Aggregate>;
  models: ModelRow[];
  sessions: SessionRow[];
  trend: Point[];
  tools: [string, number][];
  diagnostics: {
    session_files: number;
    parsed_files: number;
    events: number;
    duplicates_rejected: number;
    parser_errors: number;
    oversized_lines: number;
    last_offset: number;
    database_bytes: number;
    last_local_update: number | null;
    scan_status: string;
  };
  catalog: { schema_version: number; verified_at: string; entries: Price[] };
}
export interface Data {
  report: Report;
  quota: Quota;
  settings: Settings;
  burn: { five_hour: Burn; weekly: Burn };
  autostart: boolean;
  version: string;
}
export interface Detail {
  id: string;
  timeline: Point[];
  model_changes: [number, string, string][];
  tools: [string, number][];
  aggregate: Aggregate;
}
