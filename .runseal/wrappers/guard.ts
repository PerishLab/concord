import { bin } from "@perish/sealkit/cmd";
import { io } from "@perish/sealkit/io";

io.print("==> cargo fmt");
await bin("cargo").run(["fmt", "--all", "--check"]);

io.print("==> cargo clippy");
await bin("cargo").run([
  "clippy",
  "--locked",
  "--workspace",
  "--all-targets",
  "--",
  "-D",
  "warnings",
]);

io.print("==> cargo test");
await bin("cargo").run(["test", "--locked", "--workspace"]);

io.print("==> deno fmt");
await bin("deno").run(["fmt", "--check", ".runseal"]);

io.print("==> deno check");
await bin("deno").run([
  "check",
  "--config",
  ".runseal/deno.json",
  "--lock",
  ".runseal/deno.lock",
  "--frozen=true",
  ".runseal/wrappers/guard.ts",
  ".runseal/wrappers/init.ts",
  ".runseal/wrappers/land.ts",
  ".runseal/wrappers/release.ts",
  ".forgejo/scripts/release/metadata.ts",
  ".forgejo/scripts/release/presign.ts",
]);

io.print("==> shell syntax");
await bin("sh").run(["-n", "manage.sh"]);
await bin("sh").run(["-n", ".forgejo/scripts/release/package.sh"]);
await bin("sh").run(["-n", ".forgejo/scripts/release/skill.sh"]);
await bin("sh").run(["-n", ".forgejo/scripts/release/skill-smoke.sh"]);
await bin("sh").run(["-n", ".forgejo/scripts/release/smoke.sh"]);
await bin("bash").run(["-n", ".forgejo/scripts/release/absent.sh"]);
await bin("bash").run(["-n", ".forgejo/scripts/release/publish.sh"]);

io.print("==> skill package");
await bin("sh").run([".forgejo/scripts/release/skill-smoke.sh"]);

io.print("==> plumb doctor");
await bin("plumb").run(["doctor", "."]);

io.print("==> ectropy");
await bin("ectropy").run(["--strict", "."]);
