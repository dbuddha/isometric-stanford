export interface ReadinessSnapshot {
  complete: boolean;
  errors: number;
  loading: boolean;
  rootLoaded: boolean;
  stableFrames: number;
  visibleTiles: number;
}

export interface ReadinessRequirements {
  minimumVisibleTiles: number;
  stableDurationMs: number;
  stableFrames: number;
}

export interface WaitForReadinessOptions {
  nextFrame(): Promise<void>;
  now(): number;
  sample(nowMs: number): ReadinessSnapshot;
  timeoutMs: number;
}

export async function waitForStableReadiness(
  options: WaitForReadinessOptions,
): Promise<{ elapsedMs: number; snapshot: ReadinessSnapshot }> {
  const startedAt = options.now();
  for (;;) {
    const now = options.now();
    const snapshot = options.sample(now);
    if (snapshot.errors > 0) {
      throw new Error("tile loading failed before a complete stable frame");
    }
    if (snapshot.complete) {
      return { elapsedMs: Math.round(now - startedAt), snapshot };
    }
    if (now - startedAt > options.timeoutMs) {
      throw new Error("tile readiness timed out before a complete stable frame");
    }
    await options.nextFrame();
  }
}

export class TileReadiness {
  readonly #requirements: ReadinessRequirements;
  #errors = 0;
  #lastSignature = "";
  #loading = true;
  #rootLoaded = false;
  #stableFrames = 0;
  #stableSinceMs = 0;
  #visibleTiles = 0;

  public constructor(requirements: ReadinessRequirements) {
    this.#requirements = requirements;
  }

  public rootLoaded(): void {
    this.#rootLoaded = true;
  }

  public loadStarted(): void {
    this.#loading = true;
    this.#stableFrames = 0;
  }

  public loadEnded(): void {
    this.#loading = false;
  }

  public loadFailed(): void {
    this.#errors += 1;
    this.#loading = false;
  }

  public observe(signature: string, visibleTiles: number, nowMs: number): ReadinessSnapshot {
    this.#visibleTiles = visibleTiles;
    const eligible =
      this.#rootLoaded &&
      !this.#loading &&
      this.#errors === 0 &&
      visibleTiles >= this.#requirements.minimumVisibleTiles &&
      signature.length > 0;
    if (!eligible) {
      this.#lastSignature = signature;
      this.#stableFrames = 0;
      this.#stableSinceMs = nowMs;
      return this.snapshot(nowMs);
    }
    if (signature !== this.#lastSignature) {
      this.#lastSignature = signature;
      this.#stableFrames = 1;
      this.#stableSinceMs = nowMs;
    } else {
      this.#stableFrames += 1;
    }
    return this.snapshot(nowMs);
  }

  public snapshot(nowMs: number): ReadinessSnapshot {
    const complete =
      this.#rootLoaded &&
      !this.#loading &&
      this.#errors === 0 &&
      this.#visibleTiles >= this.#requirements.minimumVisibleTiles &&
      this.#stableFrames >= this.#requirements.stableFrames &&
      nowMs - this.#stableSinceMs >= this.#requirements.stableDurationMs;
    return {
      complete,
      errors: this.#errors,
      loading: this.#loading,
      rootLoaded: this.#rootLoaded,
      stableFrames: this.#stableFrames,
      visibleTiles: this.#visibleTiles,
    };
  }
}
