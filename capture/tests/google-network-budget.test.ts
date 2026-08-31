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
      rootTilesetRequests: 1,
      blocked: 1,
      completed: 2,
      failed: 0,
      formats: { glb: 1, json: 1 },
      requestLimit: 2,
      responseBodyBytes: 8,
      rootTilesetSha256: "df3f619804a92fdb4057192dc43dd748ea778adc52bc498ce80524c014b81119",
      statuses: { "200": 2 },
    });
    expect(JSON.stringify(budget.snapshot())).not.toContain("secret");
  });

  it("rejects a changed root response within one browser session", async () => {
    let call = 0;
    const fakeFetch: typeof fetch = async () => {
      call += 1;
      return new Response(new Uint8Array([call]), { status: 200 });
    };
    const budget = new BrowserGoogleRequestBudget(2, fakeFetch);
    const root = "https://tile.googleapis.com/v1/3dtiles/root.json?key=secret";
    await budget.fetch(root);
    await expect(budget.fetch(root)).rejects.toThrow("changed within one browser capture session");
    expect(budget.snapshot()).toMatchObject({
      attempted: 2,
      rootTilesetRequests: 2,
      completed: 2,
      failed: 1,
    });
    expect(JSON.stringify(budget.snapshot())).not.toContain("secret");
  });

  it("accepts the bounded atlas ceiling and rejects larger captures", () => {
    expect(() => new BrowserGoogleRequestBudget(4_000, fetch)).not.toThrow();
    expect(() => new BrowserGoogleRequestBudget(4_001, fetch)).toThrow(/request limit/);
  });
});
