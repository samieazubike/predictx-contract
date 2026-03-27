import {
  truncateAddress,
  formatDate,
  formatCountdown,
  formatPercent,
} from "../src/utils/formatting";

describe("truncateAddress", () => {
  const addr = "GABCDEFGHIJKLMNOPQRSTUVWXYZABCDEFGHIJKLMNOPQRSTUVWXYZ";

  it("truncates long addresses", () => {
    const result = truncateAddress(addr);
    expect(result).toContain("…");
    expect(result.startsWith("GABCDE")).toBe(true);
  });

  it("returns address unchanged if short", () => {
    expect(truncateAddress("GABCD")).toBe("GABCD");
  });

  it("respects custom start/end lengths", () => {
    const result = truncateAddress(addr, 4, 4);
    expect(result).toMatch(/^GABC…/);
  });
});

describe("formatDate", () => {
  it("returns a non-empty string for a valid date", () => {
    const result = formatDate(new Date("2025-06-15T12:00:00Z"));
    expect(typeof result).toBe("string");
    expect(result.length).toBeGreaterThan(0);
  });
});

describe("formatCountdown", () => {
  it("returns 'Expired' for past dates", () => {
    const past = new Date(Date.now() - 1000);
    expect(formatCountdown(past)).toBe("Expired");
  });

  it("shows minutes for near-future dates", () => {
    const future = new Date(Date.now() + 5 * 60 * 1000); // 5 minutes
    const result = formatCountdown(future);
    expect(result).toMatch(/\dm/);
  });

  it("shows hours for medium-future dates", () => {
    const future = new Date(Date.now() + 3 * 3600 * 1000); // 3 hours
    const result = formatCountdown(future);
    expect(result).toMatch(/h/);
  });

  it("shows days for far-future dates", () => {
    const future = new Date(Date.now() + 3 * 86_400 * 1000); // 3 days
    const result = formatCountdown(future);
    expect(result).toMatch(/d/);
  });
});

describe("formatPercent", () => {
  it("shows + for positive ROI", () => {
    expect(formatPercent(12.5)).toBe("+12.50%");
  });

  it("shows - for negative ROI", () => {
    expect(formatPercent(-5.2)).toBe("-5.20%");
  });

  it("formats zero", () => {
    expect(formatPercent(0)).toBe("+0.00%");
  });
});
