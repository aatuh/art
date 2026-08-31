import assert from "node:assert/strict";
import { mkdir, mkdtemp, rm, symlink, writeFile } from "node:fs/promises";
import { request } from "node:http";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";

import {
  contentTypeForPath,
  createGalleryServer,
  parseWatchMode,
  resolveRequestPath,
  shouldRebuildForPath
} from "./serve.mjs";

test("watch arguments accept only the documented optional flag", () => {
  assert.equal(parseWatchMode([]), false);
  assert.equal(parseWatchMode(["--watch"]), true);
  assert.throws(() => parseWatchMode(["--watch", "extra"]), /Usage:/);
  assert.throws(() => parseWatchMode(["--port", "9000"]), /Usage:/);
});

test("watch selection includes source and static artwork assets", () => {
  for (const path of [
    "crates/artwork-world-in-light/shaders/planet.vert.glsl",
    "assets/earth/earth-surface-4096.webp",
    "Cargo.lock",
    "Cargo.toml",
    "bootstrap.js",
    "index.html",
    "styles.css"
  ]) {
    assert.equal(shouldRebuildForPath(path), true, path);
  }
  for (const path of [
    "dist/index.html",
    "target/release/gallery.wasm",
    "crates/artwork-world-in-light/target/debug/library.rlib",
    ".git/index",
    "README.md",
    "assets-backup/image.png"
  ]) {
    assert.equal(shouldRebuildForPath(path), false, path);
  }
});

test("request path resolution remains inside the configured serving root", () => {
  const servingRoot = join(tmpdir(), "gallery-serving-root");
  assert.equal(resolveRequestPath("/", servingRoot), join(servingRoot, "index.html"));
  assert.equal(
    resolveRequestPath("/assets/earth.png?cache=1", servingRoot),
    join(servingRoot, "assets", "earth.png")
  );
  assert.equal(resolveRequestPath("/%2e%2e%2foutside.txt", servingRoot), null);
  assert.equal(resolveRequestPath("/%00", servingRoot), null);
  assert.throws(() => resolveRequestPath("/%", servingRoot), URIError);
});

test("MIME selection retains browser-critical image and Wasm mappings", () => {
  assert.equal(contentTypeForPath("texture.png"), "image/png");
  assert.equal(contentTypeForPath("texture.webp"), "image/webp");
  assert.equal(contentTypeForPath("gallery.wasm"), "application/wasm");
  assert.equal(contentTypeForPath("unknown.bin"), "application/octet-stream");
});

test("server bounds paths and implements GET, HEAD, and method rejection", async (context) => {
  const fixtureRoot = await mkdtemp(join(tmpdir(), "gallery-server-test-"));
  const servingRoot = join(fixtureRoot, "dist");
  await mkdir(join(servingRoot, "assets"), { recursive: true });
  await writeFile(join(servingRoot, "index.html"), "<!doctype html><body>gallery</body>");
  await writeFile(join(servingRoot, "assets", "pixel.png"), Buffer.from([0x89, 0x50, 0x4e, 0x47]));
  await writeFile(join(fixtureRoot, "outside.txt"), "not public");
  await symlink(join(fixtureRoot, "outside.txt"), join(servingRoot, "escape.txt"));

  const server = createGalleryServer({ servingRoot });
  await new Promise((resolve, reject) => {
    server.once("error", reject);
    server.listen(0, "127.0.0.1", resolve);
  });
  context.after(async () => {
    await new Promise((resolve, reject) => {
      server.close((error) => (error ? reject(error) : resolve()));
    });
    await rm(fixtureRoot, { recursive: true, force: true });
  });

  const get = await sendRequest(server, "GET", "/");
  assert.equal(get.statusCode, 200);
  assert.match(get.body.toString("utf8"), /gallery/);

  const head = await sendRequest(server, "HEAD", "/");
  assert.equal(head.statusCode, 200);
  assert.equal(head.body.length, 0);
  assert.equal(head.headers["content-length"], get.headers["content-length"]);

  const png = await sendRequest(server, "GET", "/assets/pixel.png");
  assert.equal(png.statusCode, 200);
  assert.equal(png.headers["content-type"], "image/png");

  const traversal = await sendRequest(server, "GET", "/%2e%2e%2foutside.txt");
  assert.equal(traversal.statusCode, 403);
  const symlinkEscape = await sendRequest(server, "GET", "/escape.txt");
  assert.equal(symlinkEscape.statusCode, 403);
  const malformed = await sendRequest(server, "GET", "/%");
  assert.equal(malformed.statusCode, 400);

  const post = await sendRequest(server, "POST", "/");
  assert.equal(post.statusCode, 405);
  assert.equal(post.headers.allow, "GET, HEAD");
});

function sendRequest(server, method, path) {
  const address = server.address();
  if (!address || typeof address === "string") throw new Error("server did not bind to TCP");
  return new Promise((resolve, reject) => {
    const outgoing = request(
      { host: "127.0.0.1", port: address.port, method, path },
      (response) => {
        const chunks = [];
        response.on("data", (chunk) => chunks.push(chunk));
        response.on("end", () => {
          resolve({
            statusCode: response.statusCode,
            headers: response.headers,
            body: Buffer.concat(chunks)
          });
        });
      }
    );
    outgoing.once("error", reject);
    outgoing.end();
  });
}
