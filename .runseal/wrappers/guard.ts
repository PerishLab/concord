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

io.print("==> cargo check release");
await bin("cargo").run([
  "check",
  "--locked",
  "--workspace",
  "--all-targets",
  "--release",
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
]);

io.print("==> plumb doctor");
await bin("plumb").run(["doctor", "."]);

io.print("==> ectropy");
await bin("ectropy").run(["."]);
