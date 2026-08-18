import { StrictMode } from "react";
import { createRoot } from "react-dom/client";

const root = document.getElementById("root");
if (!root) {
  throw new Error("root element is missing");
}

const application = createRoot(root);
const reviewRoute = window.location.pathname.replace(/\/+$/, "").endsWith("/review");

if (reviewRoute) {
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
