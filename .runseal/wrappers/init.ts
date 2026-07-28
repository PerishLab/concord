import { bin, exists } from "@perish/sealkit/cmd";
import { fs } from "@perish/sealkit/fs";
import { io } from "@perish/sealkit/io";
import { path } from "@perish/sealkit/path";

const tools = ["git", "deno", "cargo", "plumb", "runseal", "ectropy"];
const paths = [
  "Cargo.toml",
  "ectropy.toml",
  "runseal.toml",
  ".runseal/deno.json",
  ".runseal/deno.lock",
  ".runseal/hooks/pre-commit",
  ".runseal/hooks/commit-msg",
  ".runseal/wrappers/guard.ts",
  ".runseal/wrappers/init.ts",
  ".runseal/wrappers/land.ts",
  ".runseal/wrappers/release.ts",
  ".forgejo/workflows/guard.yml",
  ".forgejo/workflows/release-beta.yml",
  ".forgejo/workflows/release-stable.yml",
];

for (const tool of tools) {
  if (!(await exists(tool))) {
    io.fail(`init: missing required tool: ${tool}`);
  }
}

const root = await bin("git").text(["rev-parse", "--show-toplevel"]);
for (const entry of paths) {
  if (!(await fs.file.exists(path.join(root, entry)))) {
    io.fail(`init: missing required path: ${entry}`);
  }
}
await bin("git").run(["config", "core.hooksPath", ".runseal/hooks"], { cwd: root });
io.print(`repository ready: ${root}`);
