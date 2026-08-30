import { spawnSync } from "node:child_process";

export interface ProcessMemoryPeak {
  chromiumBytes: number;
  chromiumGpuBytes: number;
  chromiumRendererBytes: number;
  descendantBytes: number;
  nodeBytes: number;
  otherDescendantBytes: number;
  treeBytes: number;
}

export interface ProcessMemoryReport {
  intervalMilliseconds: number;
  peak: ProcessMemoryPeak;
  samples: number;
  stages: Record<string, ProcessMemoryPeak>;
}

interface ProcessRow {
  command: string;
  pid: number;
  ppid: number;
  rssBytes: number;
}

const EMPTY_PEAK: ProcessMemoryPeak = {
  chromiumBytes: 0,
  chromiumGpuBytes: 0,
  chromiumRendererBytes: 0,
  descendantBytes: 0,
  nodeBytes: 0,
  otherDescendantBytes: 0,
  treeBytes: 0,
};

function maximum(left: ProcessMemoryPeak, right: ProcessMemoryPeak): ProcessMemoryPeak {
  return {
    chromiumBytes: Math.max(left.chromiumBytes, right.chromiumBytes),
    chromiumGpuBytes: Math.max(left.chromiumGpuBytes, right.chromiumGpuBytes),
    chromiumRendererBytes: Math.max(left.chromiumRendererBytes, right.chromiumRendererBytes),
    descendantBytes: Math.max(left.descendantBytes, right.descendantBytes),
    nodeBytes: Math.max(left.nodeBytes, right.nodeBytes),
    otherDescendantBytes: Math.max(left.otherDescendantBytes, right.otherDescendantBytes),
    treeBytes: Math.max(left.treeBytes, right.treeBytes),
  };
}

export function parseProcessTable(output: string): ProcessRow[] {
  const rows: ProcessRow[] = [];
  for (const line of output.split("\n")) {
    const match = /^\s*(\d+)\s+(\d+)\s+(\d+)\s+(.*)$/.exec(line);
    if (match === null) {
      continue;
    }
    const pid = Number(match[1]);
    const ppid = Number(match[2]);
    const rssKilobytes = Number(match[3]);
    const command = match[4] ?? "";
    if (
      !Number.isSafeInteger(pid) ||
      !Number.isSafeInteger(ppid) ||
      !Number.isSafeInteger(rssKilobytes)
    ) {
      continue;
    }
    rows.push({ command, pid, ppid, rssBytes: rssKilobytes * 1_024 });
  }
  return rows;
}

function isSamplerProcess(command: string): boolean {
  return /(?:^|\/)ps\s+-axo\s+pid=,ppid=,rss=,command=/.test(command);
}

function isChromiumProcess(command: string): boolean {
  return /(?:chromium|chrome|headless_shell)/i.test(command);
}

export function summarizeProcessTree(rows: ProcessRow[], rootPid: number): ProcessMemoryPeak {
  const byParent = new Map<number, ProcessRow[]>();
  for (const row of rows) {
    const siblings = byParent.get(row.ppid) ?? [];
    siblings.push(row);
    byParent.set(row.ppid, siblings);
  }

  const descendants: ProcessRow[] = [];
  const pending = [rootPid];
  const seen = new Set<number>(pending);
  while (pending.length > 0) {
    const parent = pending.pop();
    if (parent === undefined) {
      break;
    }
    for (const child of byParent.get(parent) ?? []) {
      if (seen.has(child.pid)) {
        continue;
      }
      seen.add(child.pid);
      pending.push(child.pid);
      if (!isSamplerProcess(child.command)) {
        descendants.push(child);
      }
    }
  }

  const nodeBytes = rows.find((row) => row.pid === rootPid)?.rssBytes ?? 0;
  let chromiumBytes = 0;
  let chromiumGpuBytes = 0;
  let chromiumRendererBytes = 0;
  let otherDescendantBytes = 0;
  for (const row of descendants) {
    if (isChromiumProcess(row.command)) {
      chromiumBytes += row.rssBytes;
      if (row.command.includes("--type=gpu-process")) {
        chromiumGpuBytes += row.rssBytes;
      }
      if (row.command.includes("--type=renderer")) {
        chromiumRendererBytes += row.rssBytes;
      }
    } else {
      otherDescendantBytes += row.rssBytes;
    }
  }
  const descendantBytes = chromiumBytes + otherDescendantBytes;
  return {
    chromiumBytes,
    chromiumGpuBytes,
    chromiumRendererBytes,
    descendantBytes,
    nodeBytes,
    otherDescendantBytes,
    treeBytes: nodeBytes + descendantBytes,
  };
}

function readProcessTable(): ProcessRow[] {
  const result = spawnSync("ps", ["-axo", "pid=,ppid=,rss=,command="], {
    encoding: "utf8",
    maxBuffer: 16 * 1_024 * 1_024,
  });
  if (result.status !== 0) {
    throw new Error(`process memory sampler failed with status ${result.status ?? "unknown"}`);
  }
  return parseProcessTable(result.stdout);
}

export class ProcessMemorySampler {
  readonly #intervalMilliseconds: number;
  #interval: NodeJS.Timeout | undefined;
  #peak = { ...EMPTY_PEAK };
  #samples = 0;
  #stage = "startup";
  readonly #stages = new Map<string, ProcessMemoryPeak>();

  public constructor(intervalMilliseconds = 250) {
    if (!Number.isSafeInteger(intervalMilliseconds) || intervalMilliseconds < 50) {
      throw new Error("process memory sampling interval must be at least 50 milliseconds");
    }
    this.#intervalMilliseconds = intervalMilliseconds;
  }

  public start(): void {
    if (this.#interval !== undefined) {
      throw new Error("process memory sampler is already running");
    }
    this.sample();
    this.#interval = setInterval(() => this.sample(), this.#intervalMilliseconds);
    this.#interval.unref();
  }

  public setStage(stage: string): void {
    if (!/^[a-z0-9-]{1,64}$/.test(stage)) {
      throw new Error("process memory stage must be a safe identifier");
    }
    this.#stage = stage;
    this.sample();
  }

  public sample(): void {
    const current = summarizeProcessTree(readProcessTable(), process.pid);
    this.#peak = maximum(this.#peak, current);
    this.#stages.set(this.#stage, maximum(this.#stages.get(this.#stage) ?? EMPTY_PEAK, current));
    this.#samples += 1;
  }

  public stop(): ProcessMemoryReport {
    if (this.#interval !== undefined) {
      clearInterval(this.#interval);
      this.#interval = undefined;
    }
    this.sample();
    return {
      intervalMilliseconds: this.#intervalMilliseconds,
      peak: this.#peak,
      samples: this.#samples,
      stages: Object.fromEntries([...this.#stages.entries()].sort(([left], [right]) => left.localeCompare(right))),
    };
  }
}
