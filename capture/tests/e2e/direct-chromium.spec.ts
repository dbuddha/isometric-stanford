import { once } from "node:events";
import { createServer } from "node:http";
import { expect, test } from "@playwright/test";
import { runDirectChromiumProbe } from "../../src/node/headless-probe.js";
import { startProbeCoordinator } from "../../src/node/probe-coordinator.js";

test("direct Chromium returns credential-free evidence without a Playwright protocol session", async () => {
  const requests: string[] = [];
  const renderer = createServer((request, response) => {
    requests.push(request.url ?? "");
    response.writeHead(200, { "content-type": "text/html; charset=utf-8" });
    response.end(`<!doctype html><link rel="icon" href="data:," /><script type="module">
      const canvas = document.createElement('canvas');
      if (canvas.getContext('webgl2') === null) throw new Error('WebGL2 unavailable');
      const fragment = new URLSearchParams(location.hash.slice(1));
      const coordinator = fragment.get('probe');
      const token = fragment.get('token');
      history.replaceState(null, '', location.pathname);
      const bootstrap = await fetch(coordinator + '/bootstrap', {headers: {'x-probe-token': token}});
      if (!bootstrap.ok) throw new Error('bootstrap rejected');
      await bootstrap.json();
      await fetch(coordinator + '/result', {
        method: 'POST',
        headers: {'content-type': 'application/json', 'x-probe-token': token},
        body: JSON.stringify({
          browserMemory: {jsHeapSizeLimitBytes: 1, jsHeapTotalBytes: 1, jsHeapUsedBytes: 1},
          probe: {
            candidates: [],
            network: {attempted: 0, billableRootRequests: 0, blocked: 0, completed: 0, failed: 0, formats: {}, requestLimit: 1, responseBodyBytes: 0, rootTilesetSha256: null, statuses: {}}
          }
        })
      });
    </script>`);
  });
  renderer.listen(0, "127.0.0.1");
  await once(renderer, "listening");
  const address = renderer.address();
  if (address === null || typeof address === "string") {
    throw new Error("fixture renderer did not bind a port");
  }
  const coordinator = await startProbeCoordinator({
    apiKey: "fixture-key",
    candidates: [],
    requestLimit: 1,
  });
  try {
    const result = await runDirectChromiumProbe(
      `http://127.0.0.1:${address.port}/`,
      coordinator,
      15_000,
    ).catch((error: unknown) => {
      throw new Error(
        `${error instanceof Error ? error.message : String(error)}; renderer requests: ${requests.join(",")}`,
      );
    });
    expect(result.probe.candidates).toEqual([]);
    expect(result.probe.network.requestLimit).toBe(1);
    expect(requests).toEqual(["/"]);
  } finally {
    await coordinator.close();
    await new Promise<void>((resolve, reject) => {
      renderer.close((error) => (error === undefined ? resolve() : reject(error)));
    });
  }
});
