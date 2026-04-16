import { describe, expect, it } from "vitest";
import en from "./locales/en/translation.json";

describe("English locale (Miccy)", () => {
  it("exposes brand.logoAlt for images and accessibility", () => {
    expect(en.brand).toBeDefined();
    expect(en.brand.logoAlt).toBe("Miccy");
  });

  it("uses miccy tag in onboarding permission copy for styled wordmark", () => {
    expect(en.onboarding.permissions.description).toContain("<miccy>");
    expect(en.onboarding.permissions.description).toContain("</miccy>");
  });
});
