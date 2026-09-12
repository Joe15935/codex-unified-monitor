// Compatibility entrypoint for existing source-build instructions.
import { runBuild } from "./build.mjs";
process.exitCode = runBuild();
