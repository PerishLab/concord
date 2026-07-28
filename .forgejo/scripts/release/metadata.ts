const root = Deno.env.get("RELEASE_ROOT") ?? "dist";
const channel = required("RELEASE_CHANNEL");
const version = required("RELEASE_VERSION");
const publicUrl = required("CONCORD_RELEASES_PUBLIC_URL").replace(/\/$/, "");
const names = [
  "concord-x86_64-unknown-linux-gnu.tar.gz",
  "concord-aarch64-apple-darwin.tar.gz",
  "concord-x86_64-pc-windows-msvc.zip",
];

const assets = [];
for (const name of names) {
  const bytes = await Deno.readFile(`${root}/${name}`);
  const digest = await crypto.subtle.digest("SHA-256", bytes);
  assets.push({
    name,
    sha256: Array.from(new Uint8Array(digest))
      .map((byte) => byte.toString(16).padStart(2, "0"))
      .join(""),
    url: `${publicUrl}/${channel}/versions/${version}/${name}`,
  });
}

const metadata = {
  schema: 1,
  product: "concord",
  channel,
  version,
  commit: Deno.env.get("CI_COMMIT") ?? "",
  assets,
};
await Deno.writeTextFile(`${root}/metadata.json`, `${JSON.stringify(metadata, null, 2)}\n`);

function required(name: string): string {
  const value = Deno.env.get(name) ?? "";
  if (value === "") {
    throw new Error(`${name} is required`);
  }
  return value;
}
