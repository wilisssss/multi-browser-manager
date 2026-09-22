import { useCallback, useEffect, useState } from "react";
import { createGroup, getGroups } from "../lib/tauri-api";
import type { Group } from "../types";

export function useGroups() {
  const [groups, setGroups] = useState<Group[]>([]);

  const refresh = useCallback(async () => {
    try {
      setGroups(await getGroups());
    } catch (e) {
      console.error("Failed to load groups:", e);
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const create = useCallback(
    async (name: string): Promise<Group> => {
      const group = await createGroup(name);
      setGroups((prev) =>
        [...prev, group].sort((a, b) => a.name.localeCompare(b.name)),
      );
      return group;
    },
    [],
  );

  return { groups, refresh, create };
}
