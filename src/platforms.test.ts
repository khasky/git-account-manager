import { describe, expect, it } from "vitest";
import { azureKeysPageUrl, azureOrganization, tokenPageUrl } from "./platforms";

describe("azureOrganization", () => {
  it("reads the organization out of whatever was pasted, as the backend does", () => {
    for (const input of [
      "acme",
      " acme ",
      "https://dev.azure.com/acme",
      "https://dev.azure.com/acme/Proj/_git/app",
      "dev.azure.com/acme/",
      "dev.azure.com/acme",
      "https://acme@dev.azure.com/acme/Proj",
      "https://acme.visualstudio.com/DefaultCollection",
    ]) {
      expect(azureOrganization(input), input).toBe("acme");
    }
    for (const input of ["", "   ", "acme corp", "a?b"]) {
      expect(azureOrganization(input), input).toBe("");
    }
  });
});

describe("Azure DevOps settings links", () => {
  it("open the organization's page when the field holds its URL", () => {
    expect(tokenPageUrl("azure", "dev.azure.com/thebestplan")).toBe(
      "https://dev.azure.com/thebestplan/_usersSettings/tokens",
    );
    expect(azureKeysPageUrl("https://dev.azure.com/thebestplan")).toBe(
      "https://dev.azure.com/thebestplan/_usersSettings/keys",
    );
  });

  it("fall back to dev.azure.com while no organization is typed", () => {
    expect(tokenPageUrl("azure", "")).toBe("https://dev.azure.com/");
    expect(tokenPageUrl("azure", "acme corp")).toBe("https://dev.azure.com/");
  });
});
