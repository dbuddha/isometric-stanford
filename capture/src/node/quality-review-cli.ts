import { resolve } from "node:path";
import { writeQualityReviewReport } from "./quality-review.js";

const directory = process.argv[2];
if (!directory) {
  throw new Error("quality review requires an evidence directory");
}

writeQualityReviewReport(resolve(directory))
  .then((path) => process.stdout.write(`wrote quality review evidence to ${path}\n`))
  .catch((error: unknown) => {
    const message = error instanceof Error ? error.message : String(error);
    process.stderr.write(`quality review failed: ${message}\n`);
    process.exitCode = 1;
  });
