import type { BrowserContext } from "@playwright/test";

interface RequestSizes {
  responseBodySize: number;
}

export interface GoogleNetworkTelemetry {
  attempted: number;
  billableRootRequests: number;
  blocked: number;
  completed: number;
  failed: number;
  formats: Record<string, number>;
  requestLimit: number;
  responseBodyBytes: number;
  statuses: Record<string, number>;
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
  #billableRootRequests = 0;
  #blocked = 0;
  #completed = 0;
  #failed = 0;
  #responseBodyBytes = 0;

  public constructor(limit: number) {
    if (!Number.isSafeInteger(limit) || limit < 1 || limit > 1_000) {
      throw new Error("Google request limit must be an integer from 1 through 1000");
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
      this.#billableRootRequests += 1;
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
      billableRootRequests: this.#billableRootRequests,
      blocked: this.#blocked,
      completed: this.#completed,
      failed: this.#failed,
      formats: Object.fromEntries([...this.#formats.entries()].sort()),
      requestLimit: this.#limit,
      responseBodyBytes: this.#responseBodyBytes,
      statuses: Object.fromEntries(
        [...this.#statuses.entries()].sort(([left], [right]) => left - right).map(([key, value]) => [String(key), value]),
      ),
    };
  }
}

export async function installGoogleRequestBudget(
  context: BrowserContext,
  budget: GoogleRequestBudget,
): Promise<Promise<unknown>[]> {
  const observations: Promise<unknown>[] = [];
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
    observations.push(
      request
        .sizes()
        .then((sizes) => budget.recordFinished(request.url(), sizes))
        .catch(() => budget.recordFailure(request.url())),
    );
  });
  return observations;
}
