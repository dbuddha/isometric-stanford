import { describe, expect, it } from "vitest";
import { GoogleRequestBudget } from "../src/node/request-budget.js";

const SIZES = {
  requestBodySize: 0,
  requestHeadersSize: 120,
  responseBodySize: 4_096,
  responseHeadersSize: 240,
};

describe("Google request budget", () => {
  it("counts one billable root separately and aborts before exceeding its limit", () => {
    const budget = new GoogleRequestBudget(2);
    expect(budget.authorize("http://127.0.0.1/local")).toBe(true);
    const root = "https://tile.googleapis.com/v1/3dtiles/root.json?key=secret";
    const child = "https://tile.googleapis.com/v1/3dtiles/datasets/example.glb?session=secret";
    expect(budget.authorize(root)).toBe(true);
    expect(budget.authorize(child)).toBe(true);
    expect(budget.authorize("https://tile.googleapis.com/v1/3dtiles/extra.glb?session=secret")).toBe(
      false,
    );
    budget.recordStatus(root, 200);
    budget.recordStatus(child, 200);
    budget.recordFinished(root, SIZES);
    budget.recordFinished(child, SIZES);
    expect(budget.snapshot()).toEqual({
      attempted: 2,
      billableRootRequests: 1,
      blocked: 1,
      completed: 2,
      failed: 0,
      formats: { glb: 1, json: 1 },
      requestLimit: 2,
      responseBodyBytes: 8_192,
      rootTilesetSha256: null,
      statuses: { "200": 2 },
    });
    expect(JSON.stringify(budget.snapshot())).not.toContain("secret");
  });

  it("rejects unbounded or empty limits", () => {
    expect(() => new GoogleRequestBudget(0)).toThrow(/request limit/);
    expect(() => new GoogleRequestBudget(1_001)).toThrow(/request limit/);
  });
});
