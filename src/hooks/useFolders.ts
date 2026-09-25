import { useCallback, useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { getFolders } from "../lib/tauri-api";
import type { Folder } from "../types";

/**
 * Loads every folder once and refreshes whenever the backend signals changes
 * (`profiles-changed` covers folder CRUD too — folder mutations emit it).
 */
export function useFolders() {
  const [folders, setFolders] = useState<Folder[]>([]);
  const [loading, setLoading] = useState(true);

  const refresh = useCallback(async () => {
    try {
      setFolders(await getFolders());
    } catch (e) {
      console.error("Failed to load folders:", e);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void refresh();
    const unlistenPromise = listen("profiles-changed", () => {
      void refresh();
    });
    return () => {
      unlistenPromise.then((fn) => fn());
    };
  }, [refresh]);

  return { folders, loading, refresh };
}
