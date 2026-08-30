import type { GoogleNetworkTelemetry } from "../contracts.js";

function googleUrl(value: string): URL | undefined {
  try {
    const parsed = new URL(value);
    return parsed.protocol === "https:" && parsed.hostname === "tile.googleapis.com"
      ? parsed
      : undefined;
  } catch {
    return undefined;
  }
}

function requestUrl(input: RequestInfo | URL): string {
  if (typeof input === "string") {
    return input;
  }
  return input instanceof URL ? input.toString() : input.url;
}

function format(pathname: string): string {
  if (pathname.endsWith(".glb")) {
    return "glb";
  }
  if (pathname.endsWith(".json")) {
    return "json";
  }
  return "other";
}

export class BrowserGoogleRequestBudget {
  readonly #formats = new Map<string, number>();
  readonly #limit: number;
  readonly #originalFetch: typeof fetch;
  readonly #statuses = new Map<number, number>();
  #attempted = 0;
  #billableRootRequests = 0;
  #blocked = 0;
  #completed = 0;
  #failed = 0;
  #responseBodyBytes = 0;

  public constructor(limit: number, originalFetch: typeof fetch = window.fetch.bind(window)) {
    if (!Number.isSafeInteger(limit) || limit < 1 || limit > 1_000) {
      throw new Error("browser Google request limit must be an integer from 1 through 1000");
    }
    this.#limit = limit;
    this.#originalFetch = originalFetch;
  }

  public install(): () => void {
    const replacement = async (input: RequestInfo | URL, init?: RequestInit): Promise<Response> =>
      await this.fetch(input, init);
    window.fetch = replacement;
    return () => {
      if (window.fetch === replacement) {
        window.fetch = this.#originalFetch;
      }
    };
  }

  public async fetch(input: RequestInfo | URL, init?: RequestInit): Promise<Response> {
    const url = requestUrl(input);
    const parsed = googleUrl(url);
    if (parsed === undefined) {
      return await this.#originalFetch(input, init);
    }
    if (this.#attempted >= this.#limit) {
      this.#blocked += 1;
      throw new Error("Google tile request was blocked by the bounded browser budget");
    }
    this.#attempted += 1;
    if (parsed.pathname === "/v1/3dtiles/root.json") {
      this.#billableRootRequests += 1;
    }
    const kind = format(parsed.pathname);
    this.#formats.set(kind, (this.#formats.get(kind) ?? 0) + 1);
    try {
      const response = await this.#originalFetch(input, init);
      this.#completed += 1;
      this.#statuses.set(response.status, (this.#statuses.get(response.status) ?? 0) + 1);
      const contentLength = Number.parseInt(response.headers.get("content-length") ?? "0", 10);
      if (Number.isSafeInteger(contentLength) && contentLength > 0) {
        this.#responseBodyBytes += contentLength;
      }
      return response;
    } catch (error) {
      this.#failed += 1;
      throw error;
    }
  }

  public snapshot(): GoogleNetworkTelemetry {
    return {
      attempted: this.#attempted,
      billableRootRequests: this.#billableRootRequests,
      blocked: this.#blocked,
      completed: this.#completed,
      failed: this.#failed,
      formats: Object.fromEntries([...this.#formats.entries()].sort()),
      requestLimit: this.#limit,
      responseBodyBytes: this.#responseBodyBytes,
      statuses: Object.fromEntries(
        [...this.#statuses.entries()]
          .sort(([left], [right]) => left - right)
          .map(([status, count]) => [String(status), count]),
      ),
    };
  }
}
