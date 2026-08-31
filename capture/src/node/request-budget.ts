import type { BrowserContext } from "@playwright/test";
import {
  MAX_GOOGLE_REQUESTS_PER_CAPTURE,
  type GoogleNetworkTelemetry,
} from "../contracts.js";

interface RequestSizes {
  responseBodySize: number;
}

export type { GoogleNetworkTelemetry } from "../contracts.js";

export interface GoogleRequestObservations {
  drain(): Promise<void>;
  pending(): number;
}

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

function format(pathname: string): string {
  if (pathname.endsWith(".glb")) {
    return "glb";
  }
  if (pathname.endsWith(".json")) {
    return "json";
  }
  return "other";
}

export class GoogleRequestBudget {
  readonly #formats = new Map<string, number>();
  readonly #limit: number;
  readonly #statuses = new Map<number, number>();
  #attempted = 0;
  #rootTilesetRequests = 0;
  #blocked = 0;
  #completed = 0;
  #failed = 0;
  #responseBodyBytes = 0;

  public constructor(limit: number) {
    if (
      !Number.isSafeInteger(limit) ||
      limit < 1 ||
      limit > MAX_GOOGLE_REQUESTS_PER_CAPTURE
    ) {
      throw new Error(
        `Google request limit must be an integer from 1 through ${MAX_GOOGLE_REQUESTS_PER_CAPTURE}`,
      );
    }
    this.#limit = limit;
  }

  public authorize(url: string): boolean {
    const parsed = googleUrl(url);
    if (parsed === undefined) {
      return true;
    }
    if (this.#attempted >= this.#limit) {
      this.#blocked += 1;
      return false;
    }
    this.#attempted += 1;
    if (parsed.pathname === "/v1/3dtiles/root.json") {
      this.#rootTilesetRequests += 1;
    }
    const kind = format(parsed.pathname);
    this.#formats.set(kind, (this.#formats.get(kind) ?? 0) + 1);
    return true;
  }

  public recordFailure(url: string): void {
    if (googleUrl(url) !== undefined) {
      this.#failed += 1;
    }
  }

  public recordFinished(url: string, sizes: RequestSizes): void {
    if (googleUrl(url) !== undefined) {
      this.#completed += 1;
      this.#responseBodyBytes += Math.max(0, sizes.responseBodySize);
    }
  }

  public recordStatus(url: string, status: number): void {
    if (googleUrl(url) !== undefined) {
      this.#statuses.set(status, (this.#statuses.get(status) ?? 0) + 1);
    }
  }

  public snapshot(): GoogleNetworkTelemetry {
    return {
      attempted: this.#attempted,
      rootTilesetRequests: this.#rootTilesetRequests,
      blocked: this.#blocked,
      completed: this.#completed,
      failed: this.#failed,
      formats: Object.fromEntries([...this.#formats.entries()].sort()),
      requestLimit: this.#limit,
      responseBodyBytes: this.#responseBodyBytes,
      rootTilesetSha256: null,
      statuses: Object.fromEntries(
        [...this.#statuses.entries()].sort(([left], [right]) => left - right).map(([key, value]) => [String(key), value]),
      ),
    };
  }
}

export async function installGoogleRequestBudget(
  context: BrowserContext,
  budget: GoogleRequestBudget,
): Promise<GoogleRequestObservations> {
  const observations = new Set<Promise<void>>();
  await context.route("https://tile.googleapis.com/**", async (route) => {
    if (!budget.authorize(route.request().url())) {
      await route.abort("blockedbyclient");
      return;
    }
    await route.continue();
  });
  context.on("response", (response) => {
    budget.recordStatus(response.url(), response.status());
  });
  context.on("requestfailed", (request) => {
    budget.recordFailure(request.url());
  });
  context.on("requestfinished", (request) => {
    const observation = request
      .sizes()
      .then((sizes) => budget.recordFinished(request.url(), sizes))
      .catch(() => budget.recordFailure(request.url()));
    observations.add(observation);
    void observation.then(() => observations.delete(observation));
  });
  return {
    async drain(): Promise<void> {
      while (observations.size > 0) {
        await Promise.all([...observations]);
      }
    },
    pending(): number {
      return observations.size;
    },
  };
}
