import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { App } from "./App";

describe("App", () => {
  it("reports an unqualified release without pretending imagery exists", () => {
    render(<App />);
    expect(screen.getByRole("status")).toHaveTextContent("Qualification in progress");
    expect(screen.getByText(/map is being built from the world up/i)).toBeVisible();
    expect(screen.getByTestId("release-evidence")).toHaveAttribute("aria-hidden", "true");
  });

  it("exposes accessible map controls", () => {
    render(<App />);
    expect(screen.getByRole("button", { name: "Zoom in" })).toBeEnabled();
    expect(screen.getByRole("button", { name: "Zoom out" })).toBeEnabled();
    expect(screen.getByRole("button", { name: "Reset map view" })).toBeEnabled();
  });
});
