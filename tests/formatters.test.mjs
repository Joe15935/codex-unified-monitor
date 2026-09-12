import test from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import ts from "typescript";

const root = path.resolve(import.meta.dirname, "..");

function compile(file) {
  return ts.transpileModule(fs.readFileSync(path.join(root, file), "utf8"), {
    compilerOptions: {
      module: ts.ModuleKind.CommonJS,
      target: ts.ScriptTarget.ES2021,
    },
  }).outputText;
}

function harness() {
  const constructed = { numbers: 0, dates: 0 };
  const measuredIntl = {
    NumberFormat: class extends Intl.NumberFormat {
      constructor(...args) {
        super(...args);
        constructed.numbers++;
      }
    },
    DateTimeFormat: class extends Intl.DateTimeFormat {
      constructor(...args) {
        super(...args);
        constructed.dates++;
      }
    },
  };
  const formatters = {};
  new Function("exports", "Intl", compile("src/formatters.ts"))(
    formatters,
    measuredIntl,
  );
  let language = "en";
  const data = {};
  new Function("exports", "require", compile("src/data.ts"))(data, (id) => {
    if (id === "./formatters") return formatters;
    if (id === "./i18n") {
      return {
        getLanguage: () => language,
        t: (value) =>
          value === "Unavailable" && language === "zh-CN" ? "不可用" : value,
      };
    }
    if (id === "@tauri-apps/api/core") return { isTauri: () => false };
    throw new Error(`Unexpected runtime import: ${id}`);
  });
  return { data, constructed, setLanguage: (next) => (language = next) };
}

function originalDate(seconds, timeZone, language) {
  return new Intl.DateTimeFormat(language, {
    timeZone,
    month: "short",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  }).format(new Date(seconds * 1000));
}

test("cached numbers preserve the original Intl output in both languages", () => {
  const h = harness();
  const values = [
    0,
    -0,
    0.0001,
    0.125,
    1.005,
    -73.4567,
    999,
    1000,
    9999.999,
    14800000,
    186200000,
    Number.MAX_SAFE_INTEGER,
    NaN,
    Infinity,
    -Infinity,
  ];
  for (const language of ["en", "zh-CN"]) {
    h.setLanguage(language);
    for (const n of values) {
      assert.equal(
        h.data.count(n),
        new Intl.NumberFormat(language, {
          notation: "compact",
          maximumFractionDigits: 2,
        }).format(n),
      );
      assert.equal(h.data.exact(n), new Intl.NumberFormat(language).format(n));
      assert.equal(
        h.data.usd(n),
        new Intl.NumberFormat(language, {
          style: "currency",
          currency: "USD",
          maximumFractionDigits: 2,
        }).format(n),
      );
    }
  }
  assert.equal(h.constructed.numbers, 6);
});

test("cached dates preserve timezone, day boundaries and DST output", () => {
  const h = harness();
  const timestamps = [
    -1,
    Date.parse("2026-09-12T00:15:00Z") / 1000,
    Date.parse("2026-03-08T06:59:00Z") / 1000,
    Date.parse("2026-03-08T07:01:00Z") / 1000,
    Date.parse("2026-11-01T05:59:00Z") / 1000,
    Date.parse("2026-11-01T06:01:00Z") / 1000,
  ];
  for (const language of ["en", "zh-CN"]) {
    h.setLanguage(language);
    for (const zone of [
      "UTC",
      "America/New_York",
      "Asia/Shanghai",
      "Asia/Kathmandu",
    ]) {
      for (const timestamp of timestamps) {
        assert.equal(
          h.data.date(timestamp, zone),
          originalDate(timestamp, zone, language),
        );
      }
    }
    const midnight = timestamps[1];
    assert.notEqual(
      h.data.date(midnight, "UTC"),
      h.data.date(midnight, "America/New_York"),
    );
    assert.equal(
      new Intl.DateTimeFormat(language, {
        timeZone: "America/New_York",
        day: "numeric",
      })
        .formatToParts(new Date(midnight * 1000))
        .find((part) => part.type === "day").value,
      "11",
    );
  }
  assert.equal(h.constructed.dates, 8);
});

test("null money and falsey timestamps keep their existing unavailable semantics", () => {
  const h = harness();
  for (const language of ["en", "zh-CN"]) {
    h.setLanguage(language);
    for (const n of [null, undefined]) assert.equal(h.data.usd(n), "—");
    for (const timestamp of [null, undefined, 0, -0, NaN]) {
      assert.equal(
        h.data.date(timestamp, "invalid-zone"),
        language === "en" ? "Unavailable" : "不可用",
      );
    }
  }
  assert.deepEqual(h.constructed, { numbers: 0, dates: 0 });
});

test("switching locale reuses only correctly keyed number and date formatters", () => {
  const h = harness();
  const timestamp = Date.parse("2026-09-12T15:00:00Z") / 1000;
  for (const language of ["en", "zh-CN", "en", "zh-CN", "en"]) {
    h.setLanguage(language);
    assert.equal(
      h.data.date(timestamp, "UTC"),
      originalDate(timestamp, "UTC", language),
    );
    assert.equal(
      h.data.exact(1234567.89),
      new Intl.NumberFormat(language).format(1234567.89),
    );
  }
  assert.deepEqual(h.constructed, { numbers: 2, dates: 2 });
});

test("timezone cache is bounded and retains recently used entries", () => {
  const h = harness();
  const zones = Intl.supportedValuesOf("timeZone").slice(0, 17);
  assert.equal(zones.length, 17);
  for (const zone of zones.slice(0, 16)) h.data.date(1, zone);
  assert.equal(h.constructed.dates, 16);
  h.data.date(1, zones[0]); // Keep the first entry recent.
  h.data.date(1, zones[16]);
  h.data.date(1, zones[0]);
  assert.equal(h.constructed.dates, 17);
  h.data.date(1, zones[1]); // The oldest untouched entry was evicted.
  assert.equal(h.constructed.dates, 18);
});

test("invalid timestamps and timezones keep Intl errors without evicting valid entries", () => {
  const h = harness();
  const zones = Intl.supportedValuesOf("timeZone").slice(0, 16);
  for (const zone of zones) h.data.date(1, zone);
  assert.throws(() => h.data.date(1, "not/a/timezone"), RangeError);
  h.data.date(1, zones[0]);
  assert.equal(h.constructed.dates, 16);
  for (const n of [Infinity, -Infinity, 1e20]) {
    assert.throws(() => h.data.date(n, zones[0]), RangeError);
  }
});
