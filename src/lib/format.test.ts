import { describe, expect, it } from "vitest";
import { formatLastUsed } from "./format";

// Fixed "now": 2026-09-20 12:00:00 UTC, in ms.
const NOW = Date.UTC(2026, 8, 20, 12, 0, 0);

describe("formatLastUsed", () => {
  it("returns never for null and 0", () => {
    expect(formatLastUsed(null, NOW)).toBe("never");
    expect(formatLastUsed(0, NOW)).toBe("never");
  });

  it("returns just now for sub-minute recency", () => {
    const ts = Math.floor((NOW - 30_000) / 1000);
    expect(formatLastUsed(ts, NOW)).toBe("just now");
    const future = Math.floor((NOW + 60_000) / 1000);
    expect(formatLastUsed(future, NOW)).toBe("just now"); // clock skew clamps
  });

  it("formats minutes", () => {
    const ts = Math.floor((NOW - 5 * 60_000) / 1000);
    expect(formatLastUsed(ts, NOW)).toBe("5m ago");
  });

  it("formats hours", () => {
    const ts = Math.floor((NOW - 3 * 3_600_000) / 1000);
    expect(formatLastUsed(ts, NOW)).toBe("3h ago");
  });

  it("formats days", () => {
    const ts = Math.floor((NOW - 7 * 86_400_000) / 1000);
    expect(formatLastUsed(ts, NOW)).toBe("7d ago");
  });

  it("falls back to a date string beyond 30 days", () => {
    const ts = Math.floor((NOW - 45 * 86_400_000) / 1000);
    const out = formatLastUsed(ts, NOW);
    expect(out).not.toMatch(/ago/);
    expect(out).not.toBe("never");
  });
});
