import { randomBytes } from "node:crypto";
import { createServer } from "node:http";
import type { IncomingMessage, Server, ServerResponse } from "node:http";
import { once } from "node:events";
import { REQUIRED_LAYER_NAMES } from "../contracts.js";
import type { LayerName } from "../contracts.js";
import type { PixelFormat } from "./bundle-writer.js";

const MAX_UPLOAD_BYTES = 80 * 1024 * 1024;
const ALLOWED_FORMATS = new Set<PixelFormat>(["gray8", "rgba8", "u32le-millimeters"]);

export interface UploadServer {
  close(): Promise<void>;
  token: string;
  url: string;
}

export interface LayerSink {
  accept(
    name: LayerName,
    pixels: Uint8Array,
    width: number,
    height: number,
    pixelFormat: PixelFormat,
  ): Promise<void>;
}

function respond(response: ServerResponse, status: number, message: string): void {
  response.writeHead(status, {
    "access-control-allow-origin": "*",
    "content-type": "text/plain; charset=utf-8",
  });
  response.end(message);
}

async function readBody(request: IncomingMessage): Promise<Uint8Array> {
  const chunks: Buffer[] = [];
  let length = 0;
  for await (const chunk of request) {
    const buffer = Buffer.isBuffer(chunk) ? chunk : Buffer.from(chunk);
    length += buffer.length;
    if (length > MAX_UPLOAD_BYTES) {
      throw new Error("capture upload exceeds the bounded layer size");
    }
    chunks.push(buffer);
  }
  return Buffer.concat(chunks, length);
}

function parseIntegerHeader(request: IncomingMessage, name: string): number {
  const value = request.headers[name];
  const parsed = typeof value === "string" ? Number.parseInt(value, 10) : Number.NaN;
  if (!Number.isSafeInteger(parsed) || parsed <= 0 || parsed > 4_096) {
    throw new Error(`capture upload ${name} header is invalid`);
  }
  return parsed;
}

function isLayerName(value: string): value is LayerName {
  return (REQUIRED_LAYER_NAMES as readonly string[]).includes(value);
}

export async function startUploadServer(writer: LayerSink): Promise<UploadServer> {
  const token = randomBytes(32).toString("hex");
  let closed = false;
  let chain = Promise.resolve();
  const server: Server = createServer((request, response) => {
    if (request.method === "OPTIONS") {
      response.writeHead(204, {
        "access-control-allow-headers":
          "content-type,x-capture-height,x-capture-pixel-format,x-capture-token,x-capture-width",
        "access-control-allow-methods": "POST,OPTIONS",
        "access-control-allow-origin": "*",
      });
      response.end();
      return;
    }
    chain = chain.then(async () => {
      try {
        if (request.method !== "POST" || request.headers["x-capture-token"] !== token) {
          respond(response, 403, "capture upload rejected");
          return;
        }
        const match = /^\/layer\/([a-z-]+)$/.exec(request.url ?? "");
        const name = match?.[1] ?? "";
        const pixelFormat = request.headers["x-capture-pixel-format"];
        if (!isLayerName(name) || typeof pixelFormat !== "string" || !ALLOWED_FORMATS.has(pixelFormat as PixelFormat)) {
          respond(response, 400, "capture upload contract is invalid");
          return;
        }
        const width = parseIntegerHeader(request, "x-capture-width");
        const height = parseIntegerHeader(request, "x-capture-height");
        const bytes = await readBody(request);
        await writer.accept(name, bytes, width, height, pixelFormat as PixelFormat);
        respond(response, 204, "");
      } catch {
        respond(response, 400, "capture upload could not be accepted");
      }
    });
  });
  server.listen(0, "127.0.0.1");
  await once(server, "listening");
  const address = server.address();
  if (address === null || typeof address === "string") {
    server.close();
    throw new Error("capture upload server did not bind a TCP port");
  }
  return {
    async close(): Promise<void> {
      if (closed) {
        return;
      }
      closed = true;
      await chain;
      await new Promise<void>((resolve, reject) => {
        server.close((error) => {
          if (error === undefined) {
            resolve();
          } else {
            reject(error);
          }
        });
      });
    },
    token,
    url: `http://127.0.0.1:${address.port}`,
  };
}
