import test from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import ts from "typescript";
const exports = {};
const source = fs.readFileSync(
  new URL("../src/quotaInsight.ts", import.meta.url),
  "utf8",
);
new Function(
  "exports",
  ts.transpileModule(source, {
    compilerOptions: { module: ts.ModuleKind.CommonJS },
  }).outputText,
)(exports);
const { quotaInsight } = exports;
const settings = {
  quota_poll_seconds: 60,
  adaptive_refresh: true,
  low_quota_threshold: 10,
};
const window = {
  used_percent: 75,
  remaining_percent: 25,
  window_minutes: 60,
  resets_at: 4600,
};
const meta = { status: "LIVE", updated_at: 2800 };

test("pace compares usage with elapsed cycle at the reading, not the view clock", () => {
  const result = quotaInsight(window, meta, 3000, settings);
  assert.equal(result.elapsed, 50);
  assert.equal(result.pace, "ahead");
  assert.equal(result.low, false);
  assert.equal(
    quotaInsight({ ...window, used_percent: 50 }, meta, 3000, settings).pace,
    "near",
  );
  assert.equal(
    quotaInsight({ ...window, used_percent: 30 }, meta, 3000, settings).pace,
    "below",
  );
});
test("stale, expired, future and invalid readings cannot produce actionable hints", () => {
  for (const m of [
    { ...meta, status: "STALE" },
    { ...meta, updated_at: null },
    { ...meta, updated_at: 4000 },
  ])
    assert.equal(quotaInsight(window, m, 3000, settings), null);
  assert.equal(quotaInsight(window, meta, 3200, settings), null);
  assert.equal(
    quotaInsight(window, meta, 3000, { ...settings, adaptive_refresh: false }),
    null,
  );
  assert.equal(
    quotaInsight({ ...window, resets_at: 3000 }, meta, 3000, settings),
    null,
  );
  for (const used_percent of [-1, 101, NaN])
    assert.equal(
      quotaInsight({ ...window, used_percent }, meta, 3000, settings),
      null,
    );
});
test("quota hints can be disabled and missing cycle metadata never invents a pace", () => {
  const low = { ...window, used_percent: 90, remaining_percent: 10 };
  assert.equal(quotaInsight(low, meta, 3000, settings).low, true);
  assert.equal(
    quotaInsight(low, meta, 3000, { ...settings, low_quota_threshold: 0 }).low,
    false,
  );
  for (const w of [
    { ...low, resets_at: null },
    { ...low, window_minutes: null },
    { ...low, window_minutes: 0 },
    { ...low, resets_at: 10000 },
  ]) {
    const result = quotaInsight(w, meta, 3000, settings);
    assert.equal(result.elapsed, null);
    assert.equal(result.pace, null);
  }
});
