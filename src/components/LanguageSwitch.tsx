import { useState } from "react";
import { call, native } from "../data";
import { applyLanguage, t, useLanguage, type Language } from "../i18n";

export default function LanguageSwitch() {
  const language = useLanguage();
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  return (
    <div className="language-control">
      <select
        aria-label={t("Language")}
        value={language}
        disabled={busy}
        onChange={async (event) => {
          const next = event.target.value as Language;
          setBusy(true);
          setError("");
          try {
            if (native) await call("set_language", { language: next });
            applyLanguage(next);
          } catch (e) {
            setError(String(e));
          } finally {
            setBusy(false);
          }
        }}
      >
        <option value="zh-CN" lang="zh-CN">
          中文
        </option>
        <option value="en" lang="en">
          English
        </option>
      </select>
      {error && <small role="alert">{t(error)}</small>}
    </div>
  );
}
