import { StrictMode } from "react";
import { createRoot } from "react-dom/client";

const root = document.getElementById("root");
if (!root) {
  throw new Error("root element is missing");
}

const application = createRoot(root);
const overlapReviewRoute = window.location.pathname.replace(/\/+$/, "").endsWith("/review/overlap");
const qualityReviewRoute = window.location.pathname.replace(/\/+$/, "").endsWith("/review/quality");
const reviewRoute = window.location.pathname.replace(/\/+$/, "").endsWith("/review");

if (qualityReviewRoute) {
  void import("./QualityReviewApp").then(({ QualityReviewApp }) =>
    application.render(
      <StrictMode>
        <QualityReviewApp />
      </StrictMode>,
    ),
  );
} else if (overlapReviewRoute) {
  void import("./OverlapReviewApp").then(({ OverlapReviewApp }) =>
    application.render(
      <StrictMode>
        <OverlapReviewApp />
      </StrictMode>,
    ),
  );
} else if (reviewRoute) {
  void import("./ReviewApp").then(({ ReviewApp }) =>
    application.render(
      <StrictMode>
        <ReviewApp />
      </StrictMode>,
    ),
  );
} else {
  void Promise.all([import("./App"), import("./styles.css")]).then(([{ App }]) =>
    application.render(
      <StrictMode>
        <App />
      </StrictMode>,
    ),
  );
}
