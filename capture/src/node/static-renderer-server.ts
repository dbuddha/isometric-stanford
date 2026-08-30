import { createReadStream } from "node:fs";
import { stat } from "node:fs/promises";
import { createServer } from "node:http";
import type { Server, ServerResponse } from "node:http";
import { once } from "node:events";
import { extname, resolve, sep } from "node:path";

const CONTENT_TYPES: Readonly<Record<string, string>> = {
  ".html": "text/html; charset=utf-8",
  ".js": "text/javascript; charset=utf-8",
  ".wasm": "application/wasm",
};

export interface StaticRendererServer {
  close(): Promise<void>;
  url: string;
}

function fail(response: ServerResponse, status: number, message: string): void {
  response.writeHead(status, {
    "cache-control": "no-store",
    "content-type": "text/plain; charset=utf-8",
    "x-content-type-options": "nosniff",
  });
  response.end(message);
}

export async function startStaticRendererServer(
  distributionDirectory: string,
): Promise<StaticRendererServer> {
  const root = resolve(distributionDirectory);
  const index = resolve(root, "index.html");
  if (!(await stat(index)).isFile()) {
    throw new Error("capture renderer distribution is missing; run the capture build first");
  }
  let closed = false;
  const server: Server = createServer(async (request, response) => {
    try {
      if (request.method !== "GET" && request.method !== "HEAD") {
        fail(response, 405, "method not allowed");
        return;
      }
      const url = new URL(request.url ?? "/", "http://127.0.0.1");
      let pathname: string;
      try {
        pathname = decodeURIComponent(url.pathname);
      } catch {
        fail(response, 400, "invalid renderer path");
        return;
      }
      const relativePath = pathname === "/" ? "index.html" : pathname.replace(/^\/+/, "");
      const path = resolve(root, relativePath);
      if (path !== index && !path.startsWith(`${root}${sep}assets${sep}`)) {
        fail(response, 404, "renderer artifact not found");
        return;
      }
      const record = await stat(path).catch(() => undefined);
      if (record === undefined || !record.isFile()) {
        fail(response, 404, "renderer artifact not found");
        return;
      }
      response.writeHead(200, {
        "cache-control": "no-store",
        "content-length": record.size,
        "content-type": CONTENT_TYPES[extname(path)] ?? "application/octet-stream",
        "cross-origin-resource-policy": "same-origin",
        "x-content-type-options": "nosniff",
      });
      if (request.method === "HEAD") {
        response.end();
        return;
      }
      createReadStream(path).pipe(response);
    } catch {
      fail(response, 500, "renderer artifact could not be served");
    }
  });
  server.listen(0, "127.0.0.1");
  await once(server, "listening");
  const address = server.address();
  if (address === null || typeof address === "string") {
    server.close();
    throw new Error("capture renderer server did not bind a TCP port");
  }
  return {
    async close(): Promise<void> {
      if (closed) {
        return;
      }
      closed = true;
      await new Promise<void>((resolveClose, reject) => {
        server.close((error) => {
          if (error === undefined) {
            resolveClose();
          } else {
            reject(error);
          }
        });
      });
    },
    url: `http://127.0.0.1:${address.port}`,
  };
}
