// Avoid embedding the build user's private home/source paths in release binaries.
import { homedir } from "node:os";
import { spawnSync } from "node:child_process";
const remap = `--remap-path-prefix=${homedir()}=/build`;
const inherited = process.env.CARGO_ENCODED_RUSTFLAGS || "";
const env = {
  ...process.env,
  CARGO_ENCODED_RUSTFLAGS: inherited ? `${inherited}\x1f${remap}` : remap,
  CFLAGS: `${process.env.CFLAGS || ""} -ffile-prefix-map=${JSON.stringify(homedir())}=/build`,
};
const args = process.argv.slice(2);
const result = spawnSync("npm", ["run", "tauri", "--", "build", ...args], {
  stdio: "inherit",
  env,
});
process.exit(result.status ?? 1);
