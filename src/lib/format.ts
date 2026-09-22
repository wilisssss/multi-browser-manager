/**
 * Human-readable "last used" label for a unix-seconds timestamp.
 *
 * `now` is injectable so tests are deterministic; production callers omit it.
 */
export function formatLastUsed(ts: number | null, now: number = Date.now()): string {
  if (!ts) return "never";
  const diff = Math.max(0, now - ts * 1000);
  const minutes = Math.floor(diff / 60_000);
  if (minutes < 1) return "just now";
  if (minutes < 60) return `${minutes}m ago`;
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return `${hours}h ago`;
  const days = Math.floor(hours / 24);
  if (days < 30) return `${days}d ago`;
  return new Date(ts * 1000).toLocaleDateString();
}
