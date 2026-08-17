import { describe, expect, it } from "vitest";

import type { ReleaseMetadata } from "./release-metadata";
import {
  reviewHash,
  reviewRectangle,
  reviewViewFromHash,
  supportsLandmarkReview,
} from "./review-views";

function release(): ReleaseMetadata {
  return {
    status: "artifact-candidate",
    qualified: false,
    styleId: "stanford_v1.candidate_c.1",
    styleSha256: "761cbedd340b6cd9dc4b5be899c9cadf9eb7056def1844ac96e6ef7fd964ddc2",
    worldSha256: "0f20877ff045b4180612c2b4f656aefe72ebe92390e1252ac604d0eaa06ccbcd",
    width: 7_623,
    height: 3_325,
    tileCount: 157,
    encodedBytes: 4_324_252,
    tileSetSha256: "1f0261eb5141a4a37bc43f072aa29e839bd5c35724766b4a71b15a5d5752cd41",
  };
}

describe("landmark review views", () => {
  it("parses only stable named fragments", () => {
    expect(reviewViewFromHash("#view=hoover-tower")).toBe("hoover-tower");
    expect(reviewViewFromHash("#view=main-quad")).toBe("main-quad");
    expect(reviewViewFromHash("#view=unknown")).toBe("campus");
    expect(reviewViewFromHash("")).toBe("campus");
    expect(reviewHash("memorial-church")).toBe("#view=memorial-church");
  });

  it("normalizes pixel crops in OpenSeadragon image coordinates", () => {
    expect(reviewRectangle("campus", release())).toBeNull();
    expect(reviewRectangle("hoover-tower", release())).toEqual({
      x: 3_690 / 7_623,
      y: 1_654 / 7_623,
      width: 512 / 7_623,
      height: 512 / 7_623,
    });
  });

  it("fails closed when a release changes beneath pinned review coordinates", () => {
    expect(supportsLandmarkReview(release())).toBe(true);
    const changed = release();
    changed.worldSha256 = "a".repeat(64);
    expect(supportsLandmarkReview(changed)).toBe(false);
    expect(() => reviewRectangle("main-quad", changed)).toThrow(/do not match/);

    const renamedStyle = release();
    renamedStyle.styleId = "stanford_v1.candidate_c.2";
    expect(supportsLandmarkReview(renamedStyle)).toBe(false);
  });
});
