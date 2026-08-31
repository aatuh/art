import { spawn } from "node:child_process";
import { lstatSync, readdirSync, watch } from "node:fs";
import { readFile, realpath, stat } from "node:fs/promises";
import { createServer } from "node:http";
import { extname, resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";

const projectRoot = resolve(fileURLToPath(new URL("..", import.meta.url)));
const defaultServingRoot = resolve(projectRoot, "dist");
const host = "127.0.0.1";
const port = 8080;
const relevantTopLevelPaths = new Set([
  "Cargo.lock",
  "Cargo.toml",
  "bootstrap.js",
  "index.html",
  "styles.css"
]);
const mimeTypes = Object.freeze({
  ".css": "text/css; charset=utf-8",
  ".html": "text/html; charset=utf-8",
  ".js": "text/javascript; charset=utf-8",
  ".mjs": "text/javascript; charset=utf-8",
  ".png": "image/png",
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

export function parseWatchMode(argumentsFromCommandLine) {
  if (argumentsFromCommandLine.length === 0) return false;
  if (argumentsFromCommandLine.length === 1 && argumentsFromCommandLine[0] === "--watch") {
    return true;
  }
  throw new Error("Usage: node scripts/serve.mjs [--watch]");
}

export function contentTypeForPath(filePath) {
  return mimeTypes[extname(filePath)] ?? "application/octet-stream";
}

export function resolveRequestPath(requestUrl, servingRoot = defaultServingRoot) {
  const normalizedRoot = resolve(servingRoot);
  const pathname = new URL(requestUrl, `http://${host}`).pathname;
  const decodedPath = decodeURIComponent(pathname);
  if (decodedPath.includes("\0")) return null;
  const requestedPath = decodedPath === "/" ? "/index.html" : decodedPath;
  const filePath = resolve(normalizedRoot, `.${requestedPath}`);
  return isInside(normalizedRoot, filePath) ? filePath : null;
}

export function shouldRebuildForPath(fileName) {
  const relativePath = String(fileName ?? "")
    .replaceAll("\\", "/")
    .replace(/^\.\/+/, "");
  if (
    relativePath === "" ||
    relativePath === "dist" ||
    relativePath.startsWith("dist/") ||
    relativePath === "target" ||
    relativePath.startsWith("target/") ||
    relativePath.endsWith("/target") ||
    relativePath.includes("/target/") ||
    relativePath === ".git" ||
    relativePath.startsWith(".git/")
  ) {
    return false;
  }
  return (
    relativePath === "src" ||
    relativePath.startsWith("src/") ||
    relativePath === "crates" ||
    relativePath.startsWith("crates/") ||
    relativePath === "assets" ||
    relativePath.startsWith("assets/") ||
    relevantTopLevelPaths.has(relativePath)
  );
}

export function createGalleryServer({
  servingRoot = defaultServingRoot,
  watchMode = false,
  getReloadVersion = () => 0
} = {}) {
  const normalizedRoot = resolve(servingRoot);
  const indexPath = resolve(normalizedRoot, "index.html");
  return createServer(async (request, response) => {
    if (request.method !== "GET" && request.method !== "HEAD") {
      response.writeHead(405, {
        Allow: "GET, HEAD",
        "X-Content-Type-Options": "nosniff"
      });
      response.end();
      return;
    }

    const method = request.method;
    const requestUrl = request.url ?? "/";
    let requestPath;
    try {
      requestPath = new URL(requestUrl, `http://${host}`).pathname;
    } catch {
      sendEmpty(response, 400);
      return;
    }
    if (watchMode && requestPath === "/_reload-version") {
      sendBuffer(response, method, Buffer.from(String(getReloadVersion())), {
        "Content-Type": "text/plain; charset=utf-8",
        "Cache-Control": "no-store"
      });
      return;
    }
    if (watchMode && requestPath === "/_reload-client.js") {
      sendBuffer(response, method, Buffer.from(reloadClient), {
        "Content-Type": "text/javascript; charset=utf-8",
        "Cache-Control": "no-store"
      });
      return;
    }

    let lexicalPath;
    try {
      lexicalPath = resolveRequestPath(requestUrl, normalizedRoot);
    } catch {
      sendEmpty(response, 400);
      return;
    }
    if (!lexicalPath) {
      sendEmpty(response, 403);
      return;
    }

    try {
      const [canonicalRoot, canonicalPath] = await Promise.all([
        realpath(normalizedRoot),
        realpath(lexicalPath)
      ]);
      if (!isInside(canonicalRoot, canonicalPath)) {
        sendEmpty(response, 403);
        return;
      }
      if (!(await stat(canonicalPath)).isFile()) throw new Error("Not a file");
      let content = await readFile(canonicalPath);
      if (watchMode && lexicalPath === indexPath) {
        content = Buffer.from(
          content
            .toString("utf8")
            .replace(
              "</body>",
              '    <script type="module" src="/_reload-client.js"></script>\n  </body>'
            )
        );
      }
      sendBuffer(response, method, content, {
        "Content-Type": contentTypeForPath(canonicalPath),
        "Cache-Control": "no-store"
      });
    } catch {
      sendEmpty(response, 404);
    }
  });
}

function isInside(rootPath, candidatePath) {
  return candidatePath === rootPath || candidatePath.startsWith(`${rootPath}${sep}`);
}

function sendBuffer(response, method, content, headers) {
  response.writeHead(200, {
    ...headers,
    "Content-Length": content.byteLength,
    "X-Content-Type-Options": "nosniff"
  });
  response.end(method === "HEAD" ? undefined : content);
}

function sendEmpty(response, statusCode) {
  response.writeHead(statusCode, {
    "Content-Length": 0,
    "X-Content-Type-Options": "nosniff"
  });
  response.end();
}

function watchProject(scheduleBuild) {
  const onChange = (_, fileName) => {
    if (shouldRebuildForPath(fileName)) scheduleBuild();
  };

  try {
    watch(projectRoot, { recursive: true }, onChange);
  } catch {
    watch(projectRoot, onChange);
    watchDirectoryTree(resolve(projectRoot, "src"), scheduleBuild);
    watchDirectoryTree(resolve(projectRoot, "crates"), scheduleBuild);
    watchDirectoryTree(resolve(projectRoot, "assets"), scheduleBuild);
  }
}

function watchDirectoryTree(treeRoot, scheduleBuild) {
  const watchedDirectories = new Set();
  const addDirectory = (directory) => {
    if (watchedDirectories.has(directory)) return;
    const metadata = lstatSync(directory);
    if (!metadata.isDirectory() || metadata.isSymbolicLink()) return;
    watchedDirectories.add(directory);
    const watcher = watch(directory, (eventType, fileName) => {
      scheduleBuild();
      if (eventType !== "rename" || fileName === null) return;
      const candidate = resolve(directory, String(fileName));
      try {
        if (lstatSync(candidate).isDirectory()) addDirectoryTree(candidate);
      } catch {
        // A removed entry needs no new watcher.
      }
    });
    watcher.on("error", () => watchedDirectories.delete(directory));
  };
  const addDirectoryTree = (directory) => {
    addDirectory(directory);
    for (const entry of readdirSync(directory, { withFileTypes: true })) {
      if (
        entry.isDirectory() &&
        !entry.isSymbolicLink() &&
        !["target", ".git", "dist"].includes(entry.name)
      ) {
        addDirectoryTree(resolve(directory, entry.name));
      }
    }
  };
  addDirectoryTree(treeRoot);
}

function createBuildScheduler(onSuccessfulBuild) {
  let debounceTimer;
  let buildInProgress = false;
  let rebuildQueued = false;

  const scheduleBuild = () => {
    clearTimeout(debounceTimer);
    debounceTimer = setTimeout(runBuild, 120);
  };

  const runBuild = () => {
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
    let finished = false;
    const finishOnce = (succeeded) => {
      if (finished) return;
      finished = true;
      buildInProgress = false;
      if (succeeded) {
        onSuccessfulBuild();
        console.log("Build complete; refreshing local browser tabs.");
      } else {
        console.error("Build failed; keeping the last successful site available.");
      }
      if (rebuildQueued) {
        rebuildQueued = false;
        scheduleBuild();
      }
    };
    build.once("error", () => finishOnce(false));
    build.once("exit", (code) => finishOnce(code === 0));
  };

  return scheduleBuild;
}

function main() {
  const watchMode = parseWatchMode(process.argv.slice(2));
  let reloadVersion = 0;
  const server = createGalleryServer({
    watchMode,
    getReloadVersion: () => reloadVersion
  });
  server.listen(port, host, () => {
    console.log(
      `Gallery available at http://${host}:${port}${watchMode ? " (watching for changes)" : ""}`
    );
  });

  if (watchMode) {
    const scheduleBuild = createBuildScheduler(() => {
      reloadVersion += 1;
    });
    watchProject(scheduleBuild);
  }
}

const invokedPath = process.argv[1] === undefined ? "" : resolve(process.argv[1]);
if (invokedPath === fileURLToPath(import.meta.url)) main();
