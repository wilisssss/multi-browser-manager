import { useEffect, useMemo, useRef, useState } from "react";
import type { Profile } from "../types";

/** Sort orders for the profile grid. Pinned profiles always stay on top. */
export type SortKey =
  | "name-asc"
  | "name-desc"
  | "last-used-desc"
  | "last-used-asc"
  | "created-desc"
  | "created-asc"
  | "usage-desc"
  | "status";

export const SORT_OPTIONS: { value: SortKey; label: string }[] = [
  { value: "name-asc", label: "Sort: Name A–Z" },
  { value: "name-desc", label: "Sort: Name Z–A" },
  { value: "last-used-desc", label: "Sort: Last used (newest)" },
  { value: "last-used-asc", label: "Sort: Last used (oldest)" },
  { value: "created-desc", label: "Sort: Created (newest)" },
  { value: "created-asc", label: "Sort: Created (oldest)" },
  { value: "usage-desc", label: "Sort: Most used" },
  { value: "status", label: "Sort: Status" },
];

const SORT_STORAGE_KEY = "mbm.sort";

function loadSort(): SortKey {
  const stored = localStorage.getItem(SORT_STORAGE_KEY);
  return SORT_OPTIONS.some((o) => o.value === stored) ? (stored as SortKey) : "name-asc";
}

interface Filters {
  profiles: Profile[];
  /** Valid tag ids, for the stale-filter fallback (L10). */
  groups?: { id: string }[];
  /** Total browser seconds per profile id (from launch-history stats). */
  usageSeconds?: Record<string, number>;
}

/**
 * Search (debounced), status/browser/tag filters and sorting for the profile
 * grid, plus the debounced search state. Extracted from Dashboard so the
 * filtering rules are one cohesive, testable unit.
 */
export function useProfileFilters({ profiles, groups, usageSeconds }: Filters) {
  const [search, setSearch] = useState("");
  // Debounced mirror of `search`: typing filters after a short pause instead of
  // re-filtering (and re-rendering every card) on each keystroke.
  const [debouncedSearch, setDebouncedSearch] = useState("");
  const [statusFilter, setStatusFilter] = useState<"all" | "running" | "stopped">("all");
  const [browserFilter, setBrowserFilter] = useState<string>("all");
  const [tagFilter, setTagFilter] = useState<string>("all");
  const [sortBy, setSortBy] = useState<SortKey>(loadSort);
  const searchRef = useRef<HTMLInputElement>(null);

  // Remember the chosen sort across sessions.
  useEffect(() => {
    localStorage.setItem(SORT_STORAGE_KEY, sortBy);
  }, [sortBy]);

  // 150 ms debounce on the search box.
  useEffect(() => {
    const t = setTimeout(() => setDebouncedSearch(search), 150);
    return () => clearTimeout(t);
  }, [search]);

  const browserTypes = useMemo(
    () => Array.from(new Set(profiles.map((p) => p.browserType))).sort(),
    [profiles],
  );

  const filtered = useMemo(() => {
    const q = debouncedSearch.trim().toLowerCase();
    const matched = profiles.filter((p) => {
      if (q && !p.name.toLowerCase().includes(q) && !(p.notes ?? "").toLowerCase().includes(q)) {
        return false;
      }
      if (statusFilter !== "all" && p.status !== statusFilter) return false;
      if (browserFilter !== "all" && p.browserType !== browserFilter) return false;
      if (tagFilter !== "all" && !p.groups.some((g) => g.id === tagFilter)) return false;
      return true;
    });

    // Pinned profiles always float to the top, whatever the sort order.
    const byName = (a: Profile, b: Profile) =>
      a.name.localeCompare(b.name, undefined, { sensitivity: "base" });
    const byTime = (key: "lastUsedAt" | "createdAt", desc: boolean) => (a: Profile, b: Profile) => {
      const av = a[key];
      const bv = b[key];
      // Never-used profiles sink to the bottom regardless of direction.
      if (av == null && bv == null) return byName(a, b);
      if (av == null) return 1;
      if (bv == null) return -1;
      return desc ? bv - av : av - bv;
    };

    const comparators: Record<SortKey, (a: Profile, b: Profile) => number> = {
      "name-asc": byName,
      "name-desc": (a, b) => byName(b, a),
      "last-used-desc": byTime("lastUsedAt", true),
      "last-used-asc": byTime("lastUsedAt", false),
      "created-desc": byTime("createdAt", true),
      "created-asc": byTime("createdAt", false),
      status: (a, b) => {
        // Running first, then alphabetical.
        if (a.status !== b.status) return a.status === "running" ? -1 : 1;
        return byName(a, b);
      },
      "usage-desc": (a, b) => {
        const au = usageSeconds?.[a.id] ?? 0;
        const bu = usageSeconds?.[b.id] ?? 0;
        if (au !== bu) return bu - au; // most used first
        return byName(a, b);
      },
    };
    const cmp = comparators[sortBy];
    return [...matched].sort((a, b) => {
      if (a.pinned !== b.pinned) return a.pinned ? -1 : 1;
      return cmp(a, b);
    });
  }, [profiles, debouncedSearch, statusFilter, browserFilter, tagFilter, sortBy, usageSeconds]);

  // L10: a stored filter value can become stale (browser uninstalled, tag
  // deleted) — fall back to "all" instead of showing a raw value with no
  // matching option and an empty grid.
  useEffect(() => {
    if (browserFilter !== "all" && !browserTypes.includes(browserFilter)) {
      setBrowserFilter("all");
    }
  }, [browserTypes, browserFilter]);

  useEffect(() => {
    if (tagFilter !== "all" && groups && !groups.some((g) => g.id === tagFilter)) {
      setTagFilter("all");
    }
  }, [groups, tagFilter]);

  return {
    search,
    setSearch,
    statusFilter,
    setStatusFilter,
    browserFilter,
    setBrowserFilter,
    tagFilter,
    setTagFilter,
    sortBy,
    setSortBy,
    browserTypes,
    filtered,
    searchRef,
  };
}
