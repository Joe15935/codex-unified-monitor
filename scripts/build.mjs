// Build with the installed Tauri CLI directly, without a platform shell/npm shim.
import { homedir } from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { createRequire } from "node:module";
import { spawnSync } from "node:child_process";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const require = createRequire(import.meta.url);

export function buildEnvironment(
  source = process.env,
  platform = process.platform,
  home = homedir(),
  workspace = root,
) {
  const inherited =
    source.CARGO_ENCODED_RUSTFLAGS ||
    (source.RUSTFLAGS || "").trim().split(/\s+/).filter(Boolean).join("\x1f");
  const remaps = [
    `--remap-path-prefix=${home}=/build/home`,
    `--remap-path-prefix=${workspace}=/build/src`,
  ];
  const env = {
    ...source,
    CARGO_ENCODED_RUSTFLAGS: [inherited, ...remaps]
      .filter(Boolean)
      .join("\x1f"),
  };
  // Native MSVC does not support GCC's file-prefix-map switch. Release Rust
  // paths are still remapped; do not inject an ignored/invalid flag into SQLite.
  if (platform !== "win32") {
    env.CC_SHELL_ESCAPED_FLAGS = "1";
    const quote = (value) => `'${value.replaceAll("'", "'\\''")}'`;
    env.CFLAGS = [
      source.CFLAGS || "",
      ...[home, workspace].map((prefix) =>
        quote(`-ffile-prefix-map=${prefix}=/build`),
      ),
    ]
      .filter(Boolean)
      .join(" ");
  }
  return env;
}

export function buildArguments(args, platform = process.platform) {
  const result = ["build", ...args];
  const separator = args.indexOf("--");
  const tauriArgs = separator === -1 ? args : args.slice(0, separator);
  // Tauri also detects tauri.windows.conf.json automatically; supplying it here
  // explicitly makes the packaging contract visible and testable.
  if (
    platform === "win32" &&
    !tauriArgs.some(
      (arg) =>
        arg === "--config" || arg === "-c" || arg.startsWith("--config="),
    )
  ) {
    result.splice(
      separator === -1 ? result.length : separator + 1,
      0,
      "--config",
      "src-tauri/tauri.windows.conf.json",
    );
  }
  return result;
}

export function runBuild(args = process.argv.slice(2)) {
  const cli = require.resolve("@tauri-apps/cli/tauri.js");
  const result = spawnSync(process.execPath, [cli, ...buildArguments(args)], {
    cwd: root,
    stdio: "inherit",
    env: buildEnvironment(),
    windowsHide: true,
  });
  if (result.error)
    process.stderr.write(
      `Unable to start Tauri build: ${result.error.message}\n`,
    );
  return result.status ?? 1;
}

if (
  process.argv[1] &&
  path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)
) {
  process.exitCode = runBuild();
}
