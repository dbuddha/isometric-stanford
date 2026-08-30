import { describe, expect, it } from "vitest";
import { BrowserGoogleRequestBudget } from "../src/browser/google-network-budget.js";

describe("browser Google request budget", () => {
  it("caps Google fetches before dispatch and records credential-free telemetry", async () => {
    const urls: string[] = [];
    const fakeFetch: typeof fetch = async (input) => {
      const url = typeof input === "string" ? input : input instanceof URL ? input.toString() : input.url;
      urls.push(url);
      return new Response(new Uint8Array(4), {
        headers: { "content-length": "4" },
        status: 200,
      });
    };
    const budget = new BrowserGoogleRequestBudget(2, fakeFetch);
    const root = "https://tile.googleapis.com/v1/3dtiles/root.json?key=secret";
    const child = "https://tile.googleapis.com/v1/3dtiles/example.glb?session=secret";
    await budget.fetch(root);
    await budget.fetch(child);
    await expect(budget.fetch(`${child}&third=true`)).rejects.toThrow(/bounded browser budget/);
    expect(urls).toHaveLength(2);
    expect(budget.snapshot()).toEqual({
      attempted: 2,
      billableRootRequests: 1,
      blocked: 1,
      completed: 2,
      failed: 0,
      formats: { glb: 1, json: 1 },
      requestLimit: 2,
      responseBodyBytes: 8,
      statuses: { "200": 2 },
    });
    expect(JSON.stringify(budget.snapshot())).not.toContain("secret");
  });
});
