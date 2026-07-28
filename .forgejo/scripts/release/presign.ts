const EXPIRES = 3600;
const archives: Record<string, string> = {
  put_linux: "concord-x86_64-unknown-linux-gnu.tar.gz",
  put_macos: "concord-aarch64-apple-darwin.tar.gz",
  put_windows: "concord-x86_64-pc-windows-msvc.zip",
};
const encoder = new TextEncoder();

const access = need("CONCORD_RELEASES_S3_AK");
const secret = need("CONCORD_RELEASES_S3_SK");
const bucket = need("CONCORD_RELEASES_S3_BUCKET");
const endpoint = new URL(need("CONCORD_RELEASES_S3_URL"));
const channel = need("RELEASE_CHANNEL");
const version = need("RELEASE_VERSION");
const when = new Date();

for (const [output, archive] of Object.entries(archives)) {
  const key = `${channel}/versions/${version}/${archive}`;
  const value = await presign(endpoint, bucket, key, access, secret, when);
  const path = Deno.env.get("GITHUB_OUTPUT");
  if (path) {
    await Deno.writeTextFile(path, `${output}=${value}\n`, { append: true });
  }
}

async function presign(
  endpoint: URL,
  bucket: string,
  key: string,
  access: string,
  secret: string,
  when: Date,
): Promise<string> {
  const stamp = when.toISOString().replace(/[-:]|\.\d{3}/g, "");
  const day = stamp.slice(0, 8);
  const scope = `${day}/auto/s3/aws4_request`;
  const resource = escape(`/${bucket}/${key}`, true);
  const signed = "cache-control;host";
  const query = [
    ["X-Amz-Algorithm", "AWS4-HMAC-SHA256"],
    ["X-Amz-Credential", `${access}/${scope}`],
    ["X-Amz-Date", stamp],
    ["X-Amz-Expires", String(EXPIRES)],
    ["X-Amz-SignedHeaders", signed],
  ].map(([name, value]) => `${escape(name, false)}=${escape(value, false)}`)
    .sort().join("&");
  const headers =
    `cache-control:public,max-age=31536000,immutable\nhost:${endpoint.host}\n`;
  const request = ["PUT", resource, query, headers, signed, "UNSIGNED-PAYLOAD"]
    .join("\n");
  const material = ["AWS4-HMAC-SHA256", stamp, scope, await digest(request)]
    .join("\n");
  let key4: Uint8Array = encoder.encode(`AWS4${secret}`);
  for (const part of [day, "auto", "s3", "aws4_request"]) {
    key4 = await hmac(key4, part);
  }
  return `${endpoint.origin}${resource}?${query}&X-Amz-Signature=${
    hex(await hmac(key4, material))
  }`;
}

function need(name: string): string {
  const value = (Deno.env.get(name) ?? "").trim();
  if (value === "") {
    throw new Error(`${name} is required`);
  }
  return value;
}

function escape(value: string, slash: boolean): string {
  return [...value].map((character) => {
    if (/[A-Za-z0-9\-._~]/.test(character) || (slash && character === "/")) {
      return character;
    }
    return [...encoder.encode(character)]
      .map((byte) => `%${byte.toString(16).toUpperCase().padStart(2, "0")}`)
      .join("");
  }).join("");
}

async function digest(value: string): Promise<string> {
  const bytes = await crypto.subtle.digest("SHA-256", encoder.encode(value));
  return hex(new Uint8Array(bytes));
}

async function hmac(key: Uint8Array, value: string): Promise<Uint8Array> {
  const imported = await crypto.subtle.importKey(
    "raw",
    key as BufferSource,
    { name: "HMAC", hash: "SHA-256" },
    false,
    ["sign"],
  );
  const signed = await crypto.subtle.sign(
    "HMAC",
    imported,
    encoder.encode(value),
  );
  return new Uint8Array(signed);
}

function hex(bytes: Uint8Array): string {
  return [...bytes].map((byte) => byte.toString(16).padStart(2, "0")).join("");
}
