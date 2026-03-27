/**
 * PredictX SDK — Formatting utilities for display strings.
 */

/**
 * Truncates a Stellar address for display (e.g. "GABCD…WXYZ").
 *
 * @param address - Full Stellar account or contract ID.
 * @param start   - Characters to keep at the start (default 6).
 * @param end     - Characters to keep at the end (default 4).
 */
export function truncateAddress(
  address: string,
  start = 6,
  end = 4,
): string {
  if (address.length <= start + end) return address;
  return `${address.slice(0, start)}…${address.slice(-end)}`;
}

/**
 * Formats a `Date` as a human-readable local datetime string.
 *
 * @param date   - Date to format.
 * @param locale - BCP-47 locale string (default "en-US").
 */
export function formatDate(date: Date, locale = "en-US"): string {
  return date.toLocaleString(locale, {
    year: "numeric",
    month: "short",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  });
}

/**
 * Returns a countdown string like "2h 15m" or "Expired" for a future date.
 *
 * @param target - Target date to count down to.
 */
export function formatCountdown(target: Date): string {
  const diffMs = target.getTime() - Date.now();
  if (diffMs <= 0) return "Expired";

  const totalSecs = Math.floor(diffMs / 1000);
  const days = Math.floor(totalSecs / 86_400);
  const hours = Math.floor((totalSecs % 86_400) / 3_600);
  const minutes = Math.floor((totalSecs % 3_600) / 60);

  if (days > 0) return `${days}d ${hours}h`;
  if (hours > 0) return `${hours}h ${minutes}m`;
  return `${minutes}m`;
}

/**
 * Formats a percentage (e.g. from `roi`) to two decimal places.
 *
 * @param value - Percentage value as a plain number.
 */
export function formatPercent(value: number): string {
  return `${value >= 0 ? "+" : ""}${value.toFixed(2)}%`;
}
