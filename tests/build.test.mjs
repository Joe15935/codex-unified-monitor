import test from "node:test";
import assert from "node:assert/strict";
import { buildArguments, buildEnvironment } from "../scripts/build.mjs";

test("Windows packaging selects NSIS configuration and avoids GCC-only flags", () => {
  assert.deepEqual(
    buildArguments(["--target", "x86_64-pc-windows-msvc"], "win32"),
    [
      "build",
      "--target",
      "x86_64-pc-windows-msvc",
      "--config",
      "src-tauri/tauri.windows.conf.json",
    ],
  );
  const env = buildEnvironment(
    { CFLAGS: "/O2", CARGO_ENCODED_RUSTFLAGS: "-C\x1flink-arg=/DEBUG:NONE" },
    "win32",
    "C:\\Users\\A Person",
    "D:\\源码 path",
  );
  assert.equal(env.CFLAGS, "/O2");
  assert.ok(
    env.CARGO_ENCODED_RUSTFLAGS.includes(
      "--remap-path-prefix=C:\\Users\\A Person=/build/home",
    ),
  );
  assert.ok(
    env.CARGO_ENCODED_RUSTFLAGS.includes(
      "--remap-path-prefix=D:\\源码 path=/build/src",
    ),
  );
  assert.ok(
    env.CARGO_ENCODED_RUSTFLAGS.startsWith("-C\x1flink-arg=/DEBUG:NONE\x1f"),
  );
});

test("explicit config and inherited flags survive platform packaging", () => {
  assert.deepEqual(
    buildArguments(["--config", "custom.json", "--bundles", "nsis"], "win32"),
    ["build", "--config", "custom.json", "--bundles", "nsis"],
  );
  assert.deepEqual(buildArguments(["--bundles", "app,dmg"], "darwin"), [
    "build",
    "--bundles",
    "app,dmg",
  ]);
  const env = buildEnvironment(
    { RUSTFLAGS: "-C opt-level=s", CFLAGS: "-O2" },
    "darwin",
    "/home/someone's files",
    "/work/src",
  );
  assert.ok(env.CARGO_ENCODED_RUSTFLAGS.startsWith("-C\x1fopt-level=s\x1f"));
  assert.ok(env.CFLAGS.startsWith("-O2 "));
  assert.ok(env.CFLAGS.includes("someone'\\''s files"));
  assert.equal(env.CC_SHELL_ESCAPED_FLAGS, "1");
});

test("Windows configuration is kept before arguments forwarded to Cargo", () => {
  assert.deepEqual(
    buildArguments(["--bundles", "nsis", "--", "--locked"], "win32"),
    [
      "build",
      "--bundles",
      "nsis",
      "--config",
      "src-tauri/tauri.windows.conf.json",
      "--",
      "--locked",
    ],
  );
});
