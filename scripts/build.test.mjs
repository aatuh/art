import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import {
  access,
  chmod,
  copyFile,
  mkdir,
  mkdtemp,
  readFile,
  readdir,
  rm,
  symlink,
  writeFile
} from "node:fs/promises";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

const repositoryRoot = dirname(fileURLToPath(new URL("../Makefile", import.meta.url)));

test("Make build stages completely before publishing and preserves the last good site", async (context) => {
  const fixtureRoot = await mkdtemp(join(tmpdir(), "gallery-build-test-"));
  context.after(() => rm(fixtureRoot, { recursive: true, force: true }));
  await createFixture(fixtureRoot);

  const readyPath = join(fixtureRoot, "build-ready");
  const releasePath = join(fixtureRoot, "build-release");
  const runningBuild = runMake(fixtureRoot, {
    FAKE_BUILD_READY: readyPath,
    FAKE_BUILD_RELEASE: releasePath
  });
  await waitForPath(readyPath);
  assert.equal(await readFile(join(fixtureRoot, "dist", "last-good.txt"), "utf8"), "stable");
  await assert.rejects(access(join(fixtureRoot, "dist", "pkg", "partial.txt")));

  await writeFile(releasePath, "continue");
  const completed = await runningBuild;
  assert.equal(completed.code, 0, completed.output);
  assert.equal(await readFile(join(fixtureRoot, "dist", "index.html"), "utf8"), "new index");
  assert.equal(await readFile(join(fixtureRoot, "dist", "pkg", "gallery.js"), "utf8"), "javascript");
  assert.equal(await readFile(join(fixtureRoot, "dist", "assets", "nested", "art.webp"), "utf8"), "asset");
  await assert.rejects(access(join(fixtureRoot, "dist", "last-good.txt")));
  await assert.rejects(access(join(fixtureRoot, "dist", "pkg", "partial.txt")));
  await assertNoPublishDirectories(fixtureRoot);

  await writeFile(join(fixtureRoot, "dist", "last-good.txt"), "new stable");
  const failed = await runMake(fixtureRoot, { FAKE_BUILD_FAIL: "1" });
  assert.notEqual(failed.code, 0);
  assert.equal(await readFile(join(fixtureRoot, "dist", "last-good.txt"), "utf8"), "new stable");
  assert.equal(await readFile(join(fixtureRoot, "dist", "index.html"), "utf8"), "new index");
  await assertNoPublishDirectories(fixtureRoot);
});

test("Make build refuses a staging directory that escapes through a symlink", async (context) => {
  const fixtureRoot = await mkdtemp(join(tmpdir(), "gallery-build-path-test-"));
  const outsideRoot = await mkdtemp(join(tmpdir(), "gallery-build-outside-"));
  context.after(async () => {
    await rm(fixtureRoot, { recursive: true, force: true });
    await rm(outsideRoot, { recursive: true, force: true });
  });
  await createFixture(fixtureRoot);
  await symlink(outsideRoot, join(fixtureRoot, "target"), "dir");

  const failed = await runMake(fixtureRoot);
  assert.notEqual(failed.code, 0);
  assert.match(failed.output, /Refusing symlinked build staging directory/);
  assert.equal(await readFile(join(fixtureRoot, "dist", "last-good.txt"), "utf8"), "stable");
  assert.deepEqual(await readdir(outsideRoot), []);
});

test("Make build refuses to replace a symlinked publication directory", async (context) => {
  const fixtureRoot = await mkdtemp(join(tmpdir(), "gallery-build-dist-test-"));
  const outsideRoot = await mkdtemp(join(tmpdir(), "gallery-build-dist-outside-"));
  context.after(async () => {
    await rm(fixtureRoot, { recursive: true, force: true });
    await rm(outsideRoot, { recursive: true, force: true });
  });
  await createFixture(fixtureRoot);
  await rm(join(fixtureRoot, "dist"), { recursive: true });
  await writeFile(join(outsideRoot, "do-not-replace.txt"), "outside");
  await symlink(outsideRoot, join(fixtureRoot, "dist"), "dir");

  const failed = await runMake(fixtureRoot);
  assert.notEqual(failed.code, 0);
  assert.match(failed.output, /Refusing to replace a non-directory or symlinked dist path/);
  assert.equal(await readFile(join(outsideRoot, "do-not-replace.txt"), "utf8"), "outside");
  assert.deepEqual(await readdir(outsideRoot), ["do-not-replace.txt"]);
  await assertNoPublishDirectories(fixtureRoot);
});

async function createFixture(fixtureRoot) {
  await copyFile(join(repositoryRoot, "Makefile"), join(fixtureRoot, "Makefile"));
  await mkdir(join(fixtureRoot, "assets", "nested"), { recursive: true });
  await mkdir(join(fixtureRoot, "dist"));
  await mkdir(join(fixtureRoot, "fake-bin"));
  await writeFile(join(fixtureRoot, "index.html"), "new index");
  await writeFile(join(fixtureRoot, "styles.css"), "new styles");
  await writeFile(join(fixtureRoot, "bootstrap.js"), "new bootstrap");
  await writeFile(join(fixtureRoot, "assets", "nested", "art.webp"), "asset");
  await writeFile(join(fixtureRoot, "dist", "last-good.txt"), "stable");

  const fakeWasmPack = join(fixtureRoot, "fake-bin", "wasm-pack");
  await writeFile(
    fakeWasmPack,
    `#!/usr/bin/env node
import { existsSync, mkdirSync, rmSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";

const argumentsFromCommandLine = process.argv.slice(2);
const outputIndex = argumentsFromCommandLine.indexOf("--out-dir");
if (outputIndex < 0 || !argumentsFromCommandLine[outputIndex + 1]) process.exit(64);
const outputRoot = resolve(process.cwd(), argumentsFromCommandLine[outputIndex + 1]);
mkdirSync(outputRoot, { recursive: true });
writeFileSync(resolve(outputRoot, "partial.txt"), "partial");

if (process.env.FAKE_BUILD_READY && process.env.FAKE_BUILD_RELEASE) {
  writeFileSync(process.env.FAKE_BUILD_READY, "ready");
  const deadline = Date.now() + 5000;
  const waitState = new Int32Array(new SharedArrayBuffer(4));
  while (!existsSync(process.env.FAKE_BUILD_RELEASE)) {
    if (Date.now() > deadline) process.exit(70);
    Atomics.wait(waitState, 0, 0, 10);
  }
}
if (process.env.FAKE_BUILD_FAIL === "1") process.exit(9);
rmSync(resolve(outputRoot, "partial.txt"));
writeFileSync(resolve(outputRoot, "gallery.js"), "javascript");
writeFileSync(resolve(outputRoot, "gallery_bg.wasm"), "wasm");
`
  );
  await chmod(fakeWasmPack, 0o755);
}

function runMake(fixtureRoot, extraEnvironment = {}) {
  return new Promise((resolve, reject) => {
    const child = spawn("make", ["build"], {
      cwd: fixtureRoot,
      env: {
        ...process.env,
        ...extraEnvironment,
        PATH: `${join(fixtureRoot, "fake-bin")}:${process.env.PATH ?? ""}`
      },
      stdio: ["ignore", "pipe", "pipe"]
    });
    const chunks = [];
    child.stdout.on("data", (chunk) => chunks.push(chunk));
    child.stderr.on("data", (chunk) => chunks.push(chunk));
    child.once("error", reject);
    child.once("close", (code) => resolve({ code, output: Buffer.concat(chunks).toString("utf8") }));
  });
}

async function waitForPath(path) {
  const deadline = Date.now() + 5000;
  while (Date.now() <= deadline) {
    try {
      await access(path);
      return;
    } catch {
      await new Promise((resolve) => setTimeout(resolve, 10));
    }
  }
  throw new Error(`timed out waiting for ${path}`);
}

async function assertNoPublishDirectories(fixtureRoot) {
  const targetEntries = await readdir(join(fixtureRoot, "target"));
  assert.deepEqual(
    targetEntries.filter((entry) => entry.startsWith("gallery-publish.")),
    []
  );
}
