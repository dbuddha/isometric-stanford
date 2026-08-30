import { mkdir, mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { resolve } from "node:path";
import { afterEach, describe, expect, it } from "vitest";
import { startStaticRendererServer } from "../src/node/static-renderer-server.js";

const roots: string[] = [];

afterEach(async () => {
  await Promise.all(roots.splice(0).map(async (root) => rm(root, { force: true, recursive: true })));
});

describe("static capture renderer server", () => {
  it("serves only the built index and allowlisted assets without caching", async () => {
    const root = await mkdtemp(resolve(tmpdir(), "isometric-capture-dist-"));
    roots.push(root);
    await mkdir(resolve(root, "assets"));
    await writeFile(resolve(root, "index.html"), "<!doctype html><title>capture</title>");
    await writeFile(resolve(root, "assets", "runtime.js"), "export const ready = true;");
    await writeFile(resolve(root, "secret.txt"), "not public");
    const server = await startStaticRendererServer(root);
    try {
      const index = await fetch(server.url);
      expect(index.status).toBe(200);
      expect(index.headers.get("cache-control")).toBe("no-store");
      expect(await index.text()).toContain("capture");
      const asset = await fetch(`${server.url}/assets/runtime.js`);
      expect(asset.status).toBe(200);
      expect(asset.headers.get("content-type")).toContain("text/javascript");
      expect((await fetch(`${server.url}/secret.txt`)).status).toBe(404);
      expect((await fetch(`${server.url}/../secret.txt`)).status).toBe(404);
      expect((await fetch(`${server.url}/assets/missing.js`)).status).toBe(404);
    } finally {
      await server.close();
    }
  });
});
