import { describe, expect, it } from "vitest";
import { parseProcessTable, summarizeProcessTree } from "../src/node/process-memory.js";

describe("process tree memory telemetry", () => {
  it("separates Node, Chromium, GPU, renderer, and other descendants", () => {
    const rows = parseProcessTable(`
  100     1 100000 node dist-node/src/node/probe-cli.js
  110   100  20000 /Applications/Chromium.app/Contents/MacOS/Chromium --headless
  111   110  30000 Chromium Helper --type=renderer --secret=must-not-escape
  112   110  40000 Chromium Helper --type=gpu-process
  120   100   5000 cargo run --quiet
  121   100   2000 ps -axo pid=,ppid=,rss=,command=
  200     1 999999 unrelated
`);
    const peak = summarizeProcessTree(rows, 100);
    expect(peak).toEqual({
      chromiumBytes: 90_000 * 1_024,
      chromiumGpuBytes: 40_000 * 1_024,
      chromiumRendererBytes: 30_000 * 1_024,
      descendantBytes: 95_000 * 1_024,
      nodeBytes: 100_000 * 1_024,
      otherDescendantBytes: 5_000 * 1_024,
      treeBytes: 195_000 * 1_024,
    });
    expect(JSON.stringify(peak)).not.toContain("must-not-escape");
  });

  it("ignores malformed rows and missing roots", () => {
    expect(parseProcessTable("junk\n 1 2 nope command")).toEqual([]);
    expect(summarizeProcessTree([], 999)).toEqual({
      chromiumBytes: 0,
      chromiumGpuBytes: 0,
      chromiumRendererBytes: 0,
      descendantBytes: 0,
      nodeBytes: 0,
      otherDescendantBytes: 0,
      treeBytes: 0,
    });
  });
});
