import { createServer } from "node:http";
import { watch } from "node:fs";
import { readFile, stat } from "node:fs/promises";
import { extname, resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";
import { spawn } from "node:child_process";

const projectRoot = resolve(fileURLToPath(new URL("..", import.meta.url)));
const root = resolve(projectRoot, "dist");
const host = "127.0.0.1";
const port = 8080;
const argumentsFromCommandLine = process.argv.slice(2);
const watchMode = argumentsFromCommandLine.length === 1 && argumentsFromCommandLine[0] === "--watch";

if (argumentsFromCommandLine.length > 0 && !watchMode) {
  throw new Error("Usage: node scripts/serve.mjs [--watch]");
}

let reloadVersion = 0;
let debounceTimer;
let buildInProgress = false;
let rebuildQueued = false;
const mimeTypes = Object.freeze({
  ".css": "text/css; charset=utf-8",
  ".html": "text/html; charset=utf-8",
  ".js": "text/javascript; charset=utf-8",
  ".mjs": "text/javascript; charset=utf-8",
  ".svg": "image/svg+xml",
  ".wasm": "application/wasm",
  ".webp": "image/webp"
});

const reloadClient = `
let buildVersion;
async function checkForBuild() {
  try {
    const response = await fetch("/_reload-version", { cache: "no-store" });
    const nextVersion = await response.text();
    if (buildVersion !== undefined && nextVersion !== buildVersion) location.reload();
    buildVersion = nextVersion;
  } catch { /* The local server may be restarting. */ }
  setTimeout(checkForBuild, 400);
}
checkForBuild();`;

function resolveRequestPath(requestUrl) {
  const pathname = new URL(requestUrl, `http://${host}`).pathname;
  const decodedPath = decodeURIComponent(pathname);
  if (decodedPath.includes("\0")) return null;
  const requestedPath = decodedPath === "/" ? "/index.html" : decodedPath;
  const filePath = resolve(root, `.${requestedPath}`);
  return filePath === root || filePath.startsWith(`${root}${sep}`) ? filePath : null;
}

const server = createServer(async (request, response) => {
  if (request.method !== "GET" && request.method !== "HEAD") {
    response.writeHead(405, { Allow: "GET, HEAD" });
    response.end();
    return;
  }

  const requestPath = new URL(request.url ?? "/", `http://${host}`).pathname;
  if (watchMode && requestPath === "/_reload-version") {
    response.writeHead(200, {
      "Content-Type": "text/plain; charset=utf-8",
      "Cache-Control": "no-store",
      "X-Content-Type-Options": "nosniff"
    });
    response.end(request.method === "HEAD" ? undefined : String(reloadVersion));
    return;
  }
  if (watchMode && requestPath === "/_reload-client.js") {
    response.writeHead(200, {
      "Content-Type": "text/javascript; charset=utf-8",
      "Cache-Control": "no-store",
      "X-Content-Type-Options": "nosniff"
    });
    response.end(request.method === "HEAD" ? undefined : reloadClient);
    return;
  }

  let filePath;
  try {
    filePath = resolveRequestPath(request.url ?? "/");
  } catch {
    response.writeHead(400);
    response.end();
    return;
  }
  if (!filePath) {
    response.writeHead(403);
    response.end();
    return;
  }

  try {
    if (!(await stat(filePath)).isFile()) throw new Error("Not a file");
    let content = await readFile(filePath);
    if (watchMode && filePath === resolve(root, "index.html")) {
      content = Buffer.from(
        content
          .toString("utf8")
          .replace("</body>", '    <script type="module" src="/_reload-client.js"></script>\n  </body>')
      );
    }
    response.writeHead(200, {
      "Content-Type": mimeTypes[extname(filePath)] ?? "application/octet-stream",
      "X-Content-Type-Options": "nosniff",
      "Cache-Control": "no-store"
    });
    response.end(request.method === "HEAD" ? undefined : content);
  } catch {
    response.writeHead(404);
    response.end();
  }
});

server.listen(port, host, () => {
  console.log(`Gallery available at http://${host}:${port}${watchMode ? " (watching for changes)" : ""}`);
});

if (watchMode) {
  watchProject();
}

function watchProject() {
  const relevantPaths = new Set([
    "Cargo.lock",
    "Cargo.toml",
    "bootstrap.js",
    "index.html",
    "styles.css"
  ]);
  const onChange = (_, fileName) => {
    const relativePath = String(fileName ?? "").replaceAll("\\", "/");
    if (relativePath.startsWith("dist/") || relativePath.startsWith("target/") || relativePath.startsWith(".git/")) {
      return;
    }
    if (relativePath.startsWith("src/") || relevantPaths.has(relativePath)) {
      scheduleBuild();
    }
  };

  try {
    watch(projectRoot, { recursive: true }, onChange);
  } catch {
    watch(resolve(projectRoot, "src"), () => scheduleBuild());
    for (const path of relevantPaths) watch(resolve(projectRoot, path), onChange);
  }
}

function scheduleBuild() {
  clearTimeout(debounceTimer);
  debounceTimer = setTimeout(runBuild, 120);
}

function runBuild() {
  if (buildInProgress) {
    rebuildQueued = true;
    return;
  }
  buildInProgress = true;
  console.log("Source change detected; rebuilding Wasm site…");
  const build = spawn("make", ["build"], {
    cwd: projectRoot,
    shell: false,
    stdio: "inherit"
  });
  build.once("error", () => finishBuild(false));
  build.once("exit", (code) => finishBuild(code === 0));
}

function finishBuild(succeeded) {
  buildInProgress = false;
  if (succeeded) {
    reloadVersion += 1;
    console.log("Build complete; refreshing local browser tabs.");
  } else {
    console.error("Build failed; keeping the last successful site available.");
  }
  if (rebuildQueued) {
    rebuildQueued = false;
    scheduleBuild();
  }
}
