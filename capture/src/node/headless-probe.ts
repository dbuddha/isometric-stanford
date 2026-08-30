import { spawn } from "node:child_process";
import { existsSync, readFileSync } from "node:fs";
import { mkdtemp, rm } from "node:fs/promises";
import { createRequire } from "node:module";
import { homedir, tmpdir } from "node:os";
import { dirname, isAbsolute, resolve } from "node:path";
import type { ProbeExecutionResult } from "../contracts.js";
import type { ProbeCoordinator } from "./probe-coordinator.js";

interface HeadlessBrowserExecutable {
  executable: string;
  headlessFlag: "--headless" | "--headless=new";
}

function headlessBrowserExecutable(): HeadlessBrowserExecutable {
  const require = createRequire(import.meta.url);
  const packageRoot = dirname(require.resolve("playwright-core/package.json"));
  const manifest = JSON.parse(readFileSync(resolve(packageRoot, "browsers.json"), "utf8")) as {
    browsers?: Array<{ name?: string; revision?: string }>;
  };
  const revision = manifest.browsers?.find(
    (browser) => browser.name === "chromium-headless-shell",
  )?.revision;
  if (revision === undefined || !/^\d+$/.test(revision)) {
    throw new Error("pinned Chromium manifest has no valid headless-shell revision");
  }
  const configuredCache = process.env.PLAYWRIGHT_BROWSERS_PATH;
  const defaultCache =
    process.platform === "darwin"
      ? resolve(homedir(), "Library", "Caches", "ms-playwright")
      : process.platform === "win32"
        ? resolve(
            process.env.LOCALAPPDATA ?? resolve(homedir(), "AppData", "Local"),
            "ms-playwright",
          )
        : resolve(process.env.XDG_CACHE_HOME ?? resolve(homedir(), ".cache"), "ms-playwright");
  const configuredRoot =
    configuredCache === "0"
      ? resolve(packageRoot, ".local-browsers")
      : configuredCache === undefined
        ? defaultCache
        : configuredCache;
  const cacheRoot = isAbsolute(configuredRoot)
    ? configuredRoot
    : resolve(process.env.INIT_CWD ?? process.cwd(), configuredRoot);
  const platformDirectory =
    process.platform === "darwin"
      ? `chrome-headless-shell-mac-${process.arch === "arm64" ? "arm64" : "x64"}`
      : process.platform === "win32"
        ? "chrome-headless-shell-win64"
        : "chrome-headless-shell-linux64";
  const executable = resolve(
    cacheRoot,
    `chromium_headless_shell-${revision}`,
    platformDirectory,
    process.platform === "win32" ? "chrome-headless-shell.exe" : "chrome-headless-shell",
  );
  if (existsSync(executable)) {
    return { executable, headlessFlag: "--headless" };
  }
  if (process.env.CI !== undefined) {
    const hostedChrome = ["/usr/bin/google-chrome", "/usr/bin/google-chrome-stable"].find(
      (path) => existsSync(path),
    );
    if (hostedChrome !== undefined) {
      return { executable: hostedChrome, headlessFlag: "--headless=new" };
    }
  }
  throw new Error("pinned Chromium headless shell is not installed");
}

function timeout(milliseconds: number): Promise<never> {
  return new Promise((_, reject) => {
    const timer = setTimeout(
      () => reject(new Error("direct Chromium probe timed out before returning evidence")),
      milliseconds,
    );
    timer.unref();
  });
}

function createBlankTarget(debuggerUrl: string): Promise<string> {
  return new Promise((resolveTarget, rejectTarget) => {
    const socket = new WebSocket(debuggerUrl);
    const timer = setTimeout(() => {
      socket.close();
      rejectTarget(new Error("direct Chromium target creation timed out"));
    }, 10_000);
    timer.unref();
    socket.addEventListener("open", () => {
      socket.send(
        JSON.stringify({ id: 1, method: "Target.createTarget", params: { url: "about:blank" } }),
      );
    });
    socket.addEventListener("message", (event) => {
      try {
        const response = JSON.parse(String(event.data)) as {
          error?: { message?: string };
          id?: number;
          result?: { sessionId?: string; targetId?: string };
        };
        if (response.id === 1 && typeof response.result?.targetId === "string") {
          clearTimeout(timer);
          socket.close();
          resolveTarget(response.result.targetId);
          return;
        }
        if (response.id === 1) {
          clearTimeout(timer);
          socket.close();
          rejectTarget(
            new Error(response.error?.message ?? "direct Chromium target creation was rejected"),
          );
        }
      } catch {
        clearTimeout(timer);
        socket.close();
        rejectTarget(new Error("direct Chromium returned malformed target evidence"));
      }
    });
    socket.addEventListener("error", () => {
      clearTimeout(timer);
      rejectTarget(new Error("direct Chromium target WebSocket failed"));
    });
  });
}

function navigateTarget(
  debuggerUrl: string,
  targetId: string,
  targetUrl: string,
): Promise<WebSocket> {
  return new Promise((resolveTarget, rejectTarget) => {
    let phase = "connecting";
    const observed: string[] = [];
    const endpoint = new URL(debuggerUrl);
    endpoint.pathname = `/devtools/page/${encodeURIComponent(targetId)}`;
    const socket = new WebSocket(endpoint);
    const timer = setTimeout(() => {
      socket.close();
      rejectTarget(
        new Error(
          `direct Chromium navigation timed out during ${phase}; observed ${observed.join(",")}`,
        ),
      );
    }, 10_000);
    timer.unref();
    socket.addEventListener("open", () => {
      phase = "enable-page";
      socket.send(JSON.stringify({ id: 1, method: "Page.enable" }));
    });
    socket.addEventListener("message", (event) => {
      try {
        const response = JSON.parse(String(event.data)) as {
          error?: { message?: string };
          id?: number;
          method?: string;
        };
        if (observed.length < 32) {
          observed.push(
            response.id === undefined
              ? `event:${response.method ?? "unknown"}`
              : `id:${response.id}`,
          );
        }
        if (response.id === 1 && response.error === undefined) {
          phase = "navigate";
          socket.send(
            JSON.stringify({ id: 2, method: "Page.navigate", params: { url: targetUrl } }),
          );
          return;
        }
        if (response.id !== 1 && response.id !== 2) {
          return;
        }
        clearTimeout(timer);
        if (response.error !== undefined) {
          socket.close();
          rejectTarget(new Error(response.error.message ?? "direct Chromium navigation failed"));
        } else {
          resolveTarget(socket);
        }
      } catch {
        clearTimeout(timer);
        socket.close();
        rejectTarget(new Error("direct Chromium returned malformed navigation evidence"));
      }
    });
    socket.addEventListener("error", () => {
      clearTimeout(timer);
      rejectTarget(new Error("direct Chromium page WebSocket failed"));
    });
    socket.addEventListener("close", () => {
      clearTimeout(timer);
      rejectTarget(new Error(`direct Chromium page WebSocket closed during ${phase}`));
    });
  });
}

async function createTarget(debuggerUrl: string, targetUrl: string): Promise<WebSocket> {
  const targetId = await createBlankTarget(debuggerUrl);
  return navigateTarget(debuggerUrl, targetId, targetUrl);
}

export async function runDirectChromiumProbe(
  rendererUrl: string,
  coordinator: ProbeCoordinator,
  timeoutMilliseconds: number,
): Promise<ProbeExecutionResult> {
  const profile = await mkdtemp(resolve(tmpdir(), "isometric-chromium-profile-"));
  const launchUrl = new URL(rendererUrl);
  launchUrl.hash = new URLSearchParams({ probe: coordinator.url, token: coordinator.token }).toString();
  const environment = { ...process.env };
  delete environment.GOOGLE_MAP_TILES_API_KEY;
  const graphicsArguments =
    process.platform === "darwin"
      ? ["--use-angle=metal"]
      : ["--use-angle=swiftshader", "--enable-unsafe-swiftshader"];
  const browserExecutable = headlessBrowserExecutable();
  const browser = spawn(
    browserExecutable.executable,
    [
      browserExecutable.headlessFlag,
      "--no-sandbox",
      "--disable-dev-shm-usage",
      "--disable-background-networking",
      "--disable-extensions",
      "--disable-features=HttpsUpgrades",
      "--enable-webgl",
      "--ignore-gpu-blocklist",
      "--use-gl=angle",
      ...graphicsArguments,
      "--disable-component-update",
      "--force-color-profile=srgb",
      "--no-default-browser-check",
      "--no-first-run",
      "--no-proxy-server",
      "--password-store=basic",
      "--use-mock-keychain",
      "--remote-debugging-port=0",
      "--remote-allow-origins=*",
      `--user-data-dir=${profile}`,
    ],
    { env: environment, stdio: ["ignore", "ignore", "pipe"] },
  );
  let diagnostics = "";
  let resolveDebuggerUrl: (url: string) => void = () => undefined;
  const debuggerUrl = new Promise<string>((resolveUrl) => {
    resolveDebuggerUrl = resolveUrl;
  });
  browser.stderr?.on("data", (chunk: Buffer) => {
    diagnostics = `${diagnostics}${chunk.toString()}`.slice(-16_384);
    const match = /DevTools listening on (ws:\/\/127\.0\.0\.1:\d+\/\S+)/.exec(diagnostics);
    if (match?.[1] !== undefined) {
      resolveDebuggerUrl(match[1]);
    }
  });
  const exited = new Promise<never>((_, reject) => {
    browser.once("error", reject);
    browser.once("exit", (code, signal) => {
      reject(
        new Error(
          `direct Chromium exited before returning evidence: ${code ?? signal ?? "unknown"}`,
        ),
      );
    });
  });
  let control: WebSocket | undefined;
  try {
    try {
      const endpoint = await Promise.race([
        debuggerUrl,
        exited,
        timeout(Math.min(timeoutMilliseconds, 10_000)),
      ]);
      control = await createTarget(endpoint, launchUrl.toString());
      return await Promise.race([
        coordinator.result,
        exited,
        timeout(timeoutMilliseconds),
      ]);
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      const safeDiagnostics = diagnostics.replaceAll(coordinator.token, "[REDACTED]").trim();
      throw new Error(
        `${message}${safeDiagnostics.length > 0 ? `; Chromium diagnostics: ${safeDiagnostics}` : ""}`,
      );
    }
  } finally {
    control?.close();
    if (browser.exitCode === null && browser.signalCode === null) {
      browser.kill("SIGTERM");
    }
    await new Promise<void>((resolveExit) => {
      if (browser.exitCode !== null || browser.signalCode !== null) {
        resolveExit();
        return;
      }
      browser.once("exit", () => resolveExit());
      const timer = setTimeout(() => {
        browser.kill("SIGKILL");
        resolveExit();
      }, 5_000);
      timer.unref();
    });
    await rm(profile, {
      force: true,
      maxRetries: 5,
      recursive: true,
      retryDelay: 100,
    });
  }
}
