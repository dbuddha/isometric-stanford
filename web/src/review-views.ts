import type { ReleaseMetadata } from "./release-metadata";

export type ReviewViewId = "campus" | "hoover-tower" | "memorial-church" | "main-quad";

export interface ReviewView {
  id: ReviewViewId;
  label: string;
}

export interface ReviewRectangle {
  x: number;
  y: number;
  width: number;
  height: number;
}

interface PixelRectangle {
  x: number;
  y: number;
  width: number;
  height: number;
}

const WORLD_SHA256 = "0f20877ff045b4180612c2b4f656aefe72ebe92390e1252ac604d0eaa06ccbcd";
const STYLE_ID = "stanford_v1.candidate_c.1";
const STYLE_SHA256 = "761cbedd340b6cd9dc4b5be899c9cadf9eb7056def1844ac96e6ef7fd964ddc2";
const WIDTH = 7_623;
const HEIGHT = 3_325;

export const REVIEW_VIEWS: readonly ReviewView[] = [
  { id: "campus", label: "Whole campus" },
  { id: "hoover-tower", label: "Hoover Tower" },
  { id: "memorial-church", label: "Memorial Church" },
  { id: "main-quad", label: "Main Quad" },
];

const PIXEL_RECTANGLES: Readonly<Record<Exclude<ReviewViewId, "campus">, PixelRectangle>> = {
  "hoover-tower": { x: 3_690, y: 1_654, width: 512, height: 512 },
  "memorial-church": { x: 2_642, y: 887, width: 768, height: 640 },
  "main-quad": { x: 2_049, y: 887, width: 1_600, height: 1_000 },
};

export function supportsLandmarkReview(release: ReleaseMetadata): boolean {
  return (
    release.worldSha256 === WORLD_SHA256 &&
    release.styleId === STYLE_ID &&
    release.styleSha256 === STYLE_SHA256 &&
    release.width === WIDTH &&
    release.height === HEIGHT
  );
}

export function reviewViewFromHash(hash: string): ReviewViewId {
  const value = new URLSearchParams(hash.replace(/^#/, "")).get("view");
  return REVIEW_VIEWS.some((view) => view.id === value) ? (value as ReviewViewId) : "campus";
}

export function reviewHash(view: ReviewViewId): string {
  return `#view=${view}`;
}

export function reviewRectangle(
  view: ReviewViewId,
  release: ReleaseMetadata,
): ReviewRectangle | null {
  if (view === "campus") {
    return null;
  }
  if (!supportsLandmarkReview(release)) {
    throw new Error("landmark review coordinates do not match the release artifact");
  }
  const pixels = PIXEL_RECTANGLES[view];
  return {
    x: pixels.x / WIDTH,
    y: pixels.y / WIDTH,
    width: pixels.width / WIDTH,
    height: pixels.height / WIDTH,
  };
}
