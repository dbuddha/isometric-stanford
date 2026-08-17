import { describe, expect, it } from "vitest";

import { parseReleaseMetadata } from "./release-metadata";

function manifest() {
  return {
    schema: "isometric-release/v1",
    status: "artifact-candidate",
    qualified: false,
    style_id: "stanford_v1.candidate_c.1",
    style_sha256: "b".repeat(64),
    world_sha256: "a".repeat(64),
    dzi: {
      descriptor: "hero.dzi",
      width: 7_623,
      height: 3_325,
      tile_size: 512,
      overlap: 0,
      format: "webp",
      tile_count: 157,
      encoded_bytes: 4_324_252,
      tile_set_sha256: "c".repeat(64),
    },
  };
}

describe("release metadata", () => {
  it("accepts an explicit unqualified candidate", () => {
    expect(parseReleaseMetadata(manifest())).toEqual({
      status: "artifact-candidate",
      qualified: false,
      styleId: "stanford_v1.candidate_c.1",
      styleSha256: "b".repeat(64),
      worldSha256: "a".repeat(64),
      width: 7_623,
      height: 3_325,
      tileCount: 157,
      encodedBytes: 4_324_252,
      tileSetSha256: "c".repeat(64),
    });
  });

  it("rejects qualified, malformed, or non-WebP inputs", () => {
    const qualified = manifest();
    qualified.qualified = true;
    expect(() => parseReleaseMetadata(qualified)).toThrow(/claims qualification/);

    const malformed = manifest();
    malformed.world_sha256 = "not-a-hash";
    expect(() => parseReleaseMetadata(malformed)).toThrow(/invalid/);

    const wrongFormat = manifest();
    wrongFormat.dzi.format = "jpeg";
    expect(() => parseReleaseMetadata(wrongFormat)).toThrow(/invalid/);
  });
});
