import test from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import { createRequire } from "node:module";
import ts from "typescript";

const root = path.resolve(import.meta.dirname, "..");
const catalog = JSON.parse(
  fs.readFileSync(path.join(root, "locales/zh-CN.json"), "utf8"),
);
function translator(saved) {
  const storage = new Map(saved ? [["codex-monitor-language", saved]] : []);
  const document = { documentElement: { lang: "" } };
  const localStorage = {
    getItem: (key) => storage.get(key),
    setItem: (key, value) => storage.set(key, value),
  };
  const exports = {};
  const filename = path.join(root, "src/i18n.ts");
  const { outputText } = ts.transpileModule(fs.readFileSync(filename, "utf8"), {
    compilerOptions: {
      module: ts.ModuleKind.CommonJS,
      target: ts.ScriptTarget.ES2021,
      esModuleInterop: true,
    },
  });
  new Function("require", "exports", "document", "localStorage", outputText)(
    createRequire(filename),
    exports,
    document,
    localStorage,
  );
  return { ...exports, storage, document };
}

test("language switches in both directions without changing model IDs or interpolation values", () => {
  const locale = translator();
  assert.equal(locale.t("Weekly"), "每周");
  assert.equal(locale.t("gpt-6-astra"), "gpt-6-astra");
  assert.equal(
    locale.t("Account request failed (code 401); check official Codex sign-in"),
    "账号请求失败（代码 401），请检查官方 Codex 的登录状态",
  );
  assert.equal(
    locale.t("Saved {count} report files.", { count: 3 }),
    "已保存 3 份报告文件。",
  );
  locale.applyLanguage("en");
  assert.equal(locale.t("Weekly"), "Weekly");
  assert.equal(
    locale.t("Saved {count} report files.", { count: 3 }),
    "Saved 3 report files.",
  );
  assert.equal(locale.document.documentElement.lang, "en");
  assert.equal(locale.storage.get("codex-monitor-language"), "en");
  locale.applyLanguage("zh-CN");
  assert.equal(locale.t("INCONCLUSIVE"), "证据不足（INCONCLUSIVE）");
  assert.equal(
    locale.t("Report saved: {path}", { path: "$&/private/{count}" }),
    "报告已保存：$&/private/{count}",
  );
});

test("saved locale is restored and unsupported preferences fall back safely", () => {
  assert.equal(translator("en").getLanguage(), "en");
  assert.equal(translator("zh-CN").getLanguage(), "zh-CN");
  const locale = translator("unexpected");
  assert.equal(locale.getLanguage(), "zh-CN");
  locale.applyLanguage("unexpected");
  assert.equal(locale.getLanguage(), "zh-CN");
});

test("translated messages preserve every named placeholder", () => {
  const placeholders = (text) =>
    [...text.matchAll(/\{(\w+)\}/g)].map((m) => m[1]).sort();
  for (const [source, translated] of Object.entries(catalog)) {
    assert.ok(translated.trim(), source);
    assert.deepEqual(placeholders(translated), placeholders(source), source);
  }
});

test("all literal UI translation calls have a catalog entry or an explicit brand exemption", () => {
  const brands = new Set([
    "Codex",
    "Codex Unified Monitor",
    "ChatGPT Pro",
    "Pro 20x",
    "Pro 5x",
    "PRO TIER AUDITOR",
  ]);
  for (const file of [
    "src/App.tsx",
    "src/components/TierAuditor.tsx",
    "src/components/Charts.tsx",
    "src/components/LanguageSwitch.tsx",
    "src/data.ts",
  ]) {
    const source = ts.createSourceFile(
      file,
      fs.readFileSync(path.join(root, file), "utf8"),
      ts.ScriptTarget.Latest,
      true,
      ts.ScriptKind.TSX,
    );
    const visit = (node) => {
      if (
        ts.isCallExpression(node) &&
        node.expression.getText(source) === "t" &&
        ts.isStringLiteral(node.arguments[0])
      ) {
        const key = node.arguments[0].text;
        assert.ok(catalog[key] || brands.has(key), `${file}: ${key}`);
      }
      ts.forEachChild(node, visit);
    };
    visit(source);
  }
});
