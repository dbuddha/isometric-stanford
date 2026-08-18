import { createReadStream } from "node:fs";
import { stat } from "node:fs/promises";
import { resolve } from "node:path";
import react from "@vitejs/plugin-react";
import type { Plugin } from "vite";
import { defineConfig } from "vitest/config";

const BASE = "/isometric-stanford/";
const LOCAL_REFERENCE_ROUTE = `${BASE}__reference__/`;
const REFERENCE_FILES: Record<string, string> = {
  "color.png": "image/png",
  "coverage.png": "image/png",
  "depth.bin": "application/octet-stream",
  "fixed-shadow.png": "image/png",
  "normal.png": "image/png",
  "reference.manifest.json": "application/json; charset=utf-8",
  "whitebox.png": "image/png",
};

function localReferencePlugin(directory: string | undefined): Plugin {
  return {
    name: "local-registered-reference",
    apply: "serve",
    configureServer(server) {
      if (!directory) {
        return;
      }
      const root = resolve(directory);
      server.middlewares.use((request, response, next) => {
        const pathname = new URL(request.url ?? "/", "http://127.0.0.1").pathname;
        if (!pathname.startsWith(LOCAL_REFERENCE_ROUTE)) {
          next();
          return;
        }
        const filename = pathname.slice(LOCAL_REFERENCE_ROUTE.length);
        const contentType = REFERENCE_FILES[filename];
        if (!contentType || (request.method !== "GET" && request.method !== "HEAD")) {
          response.statusCode = contentType ? 405 : 404;
          response.end();
          return;
        }
        const path = resolve(root, filename);
        void stat(path)
          .then((metadata) => {
            if (!metadata.isFile()) {
              response.statusCode = 404;
              response.end();
              return;
            }
            response.statusCode = 200;
            response.setHeader("cache-control", "no-store");
            response.setHeader("content-length", String(metadata.size));
            response.setHeader("content-type", contentType);
            response.setHeader("cross-origin-resource-policy", "same-origin");
            if (request.method === "HEAD") {
              response.end();
              return;
            }
            const stream = createReadStream(path);
            stream.on("error", () => {
              if (!response.headersSent) {
                response.statusCode = 500;
              }
              response.end();
            });
            stream.pipe(response);
          })
          .catch(() => {
            response.statusCode = 404;
            response.end();
          });
      });
    },
  };
}

export default defineConfig(() => {
  const referenceDirectory = process.env.REFERENCE_BUNDLE_DIRECTORY;
  return {
    plugins: [react(), localReferencePlugin(referenceDirectory)],
    base: BASE,
    define: referenceDirectory
      ? {
          "import.meta.env.VITE_REFERENCE_URL": JSON.stringify(
            `${LOCAL_REFERENCE_ROUTE}reference.manifest.json`,
          ),
        }
      : undefined,
    build: {
      target: "es2022",
      cssCodeSplit: true,
      sourcemap: true,
    },
    test: {
      environment: "jsdom",
      include: ["src/**/*.test.{ts,tsx}"],
      setupFiles: "./src/test-setup.ts",
    },
  };
});
