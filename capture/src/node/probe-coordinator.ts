import { randomBytes } from "node:crypto";
import { createServer } from "node:http";
import type { IncomingMessage, Server, ServerResponse } from "node:http";
import { once } from "node:events";
import type { ProbeCandidate, ProbeExecutionResult } from "../contracts.js";

const MAX_RESULT_BYTES = 2 * 1_024 * 1_024;

interface ProbeBootstrap {
  apiKey: string;
  candidates: ProbeCandidate[];
  requestLimit: number;
}

interface BrowserFailure {
  error: string;
}

export interface ProbeCoordinator {
  close(): Promise<void>;
  result: Promise<ProbeExecutionResult>;
  token: string;
  url: string;
}

function respond(response: ServerResponse, status: number, body: string): void {
  response.writeHead(status, {
    "access-control-allow-headers": "content-type,x-probe-token",
    "access-control-allow-methods": "GET,POST,OPTIONS",
    "access-control-allow-origin": "*",
    "cache-control": "no-store",
    "content-type": "application/json; charset=utf-8",
  });
  response.end(body);
}

async function readBody(request: IncomingMessage): Promise<string> {
  const chunks: Buffer[] = [];
  let length = 0;
  for await (const chunk of request) {
    const bytes = Buffer.isBuffer(chunk) ? chunk : Buffer.from(chunk);
    length += bytes.length;
    if (length > MAX_RESULT_BYTES) {
      throw new Error("probe coordinator result exceeds its bounded contract");
    }
    chunks.push(bytes);
  }
  return Buffer.concat(chunks, length).toString("utf8");
}

function isFailure(value: unknown): value is BrowserFailure {
  return (
    typeof value === "object" &&
    value !== null &&
    typeof (value as Partial<BrowserFailure>).error === "string"
  );
}

export async function startProbeCoordinator(bootstrap: ProbeBootstrap): Promise<ProbeCoordinator> {
  const token = randomBytes(32).toString("hex");
  let closed = false;
  let settled = false;
  let resolveResult: (result: ProbeExecutionResult) => void = () => undefined;
  let rejectResult: (error: Error) => void = () => undefined;
  const result = new Promise<ProbeExecutionResult>((resolve, reject) => {
    resolveResult = resolve;
    rejectResult = reject;
  });
  const server: Server = createServer((request, response) => {
    if (request.method === "OPTIONS") {
      respond(response, 204, "");
      return;
    }
    void (async () => {
      try {
        if (request.headers["x-probe-token"] !== token) {
          respond(response, 403, JSON.stringify({ error: "probe coordinator rejected request" }));
          return;
        }
        if (request.method === "GET" && request.url === "/bootstrap") {
          respond(response, 200, JSON.stringify(bootstrap));
          return;
        }
        if (request.method !== "POST" || request.url !== "/result" || settled) {
          respond(response, 400, JSON.stringify({ error: "probe coordinator request is invalid" }));
          return;
        }
        const parsed: unknown = JSON.parse(await readBody(request));
        settled = true;
        if (isFailure(parsed)) {
          rejectResult(new Error(parsed.error));
        } else {
          resolveResult(parsed as ProbeExecutionResult);
        }
        respond(response, 204, "");
      } catch (error) {
        if (!settled) {
          settled = true;
          rejectResult(error instanceof Error ? error : new Error("probe coordinator failed"));
        }
        respond(response, 400, JSON.stringify({ error: "probe coordinator could not accept result" }));
      }
    })();
  });
  server.listen(0, "127.0.0.1");
  await once(server, "listening");
  const address = server.address();
  if (address === null || typeof address === "string") {
    server.close();
    throw new Error("probe coordinator did not bind a loopback port");
  }
  return {
    async close(): Promise<void> {
      if (closed) {
        return;
      }
      closed = true;
      await new Promise<void>((resolve, reject) => {
        server.close((error) => (error === undefined ? resolve() : reject(error)));
      });
    },
    result,
    token,
    url: `http://127.0.0.1:${address.port}`,
  };
}
