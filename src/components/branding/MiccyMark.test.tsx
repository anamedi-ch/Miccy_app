import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";
import { MiccyMark } from "./MiccyMark";

describe("MiccyMark", () => {
  it("renders default product name with wordmark class", () => {
    const { container } = render(<MiccyMark />);
    expect(container.querySelector(".font-miccy")).not.toBeNull();
    expect(screen.getByText("Miccy")).toBeInTheDocument();
  });

  it("renders children inside the wordmark span", () => {
    render(<MiccyMark>{"Miccy's"}</MiccyMark>);
    expect(screen.getByText("Miccy's")).toBeInTheDocument();
  });
});
