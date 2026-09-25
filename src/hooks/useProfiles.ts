import { useCallback, useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import type { Profile } from "../types";
import {
  createProfile,
  deleteProfile,
  duplicateProfile,
  getProfiles,
  launchProfile,
  stopProfile,
  updateProfile,
} from "../lib/tauri-api";

export function useProfiles() {
  const [profiles, setProfiles] = useState<Profile[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    try {
      const list = await getProfiles();
      setProfiles(list);
      setError(null);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    refresh();

    // Backend emits profile-stopped when a browser process exits (manually or killed).
    const unlistenPromise = listen<string>("profile-stopped", (event) => {
      setProfiles((prev) =>
        prev.map((p) =>
          p.id === event.payload ? { ...p, status: "stopped" as const } : p,
        ),
      );
    });

    const failedPromise = listen<{ profileId: string; reason: string }>(
      "profile-launch-failed",
      (event) => {
        setError(`Launch failed for ${event.payload.profileId}: ${event.payload.reason}`);
      },
    );

    return () => {
      unlistenPromise.then((fn) => fn());
      failedPromise.then((fn) => fn());
    };
  }, [refresh]);

  const launch = useCallback(
    async (id: string) => {
      await launchProfile(id);
      setProfiles((prev) =>
        prev.map((p) =>
          p.id === id
            ? { ...p, status: "running" as const, lastUsedAt: Math.floor(Date.now() / 1000) }
            : p,
        ),
      );
    },
    [],
  );

  const stop = useCallback(async (id: string) => {
    await stopProfile(id);
    // Status update also arrives via the profile-stopped event; this is instant feedback.
    setProfiles((prev) =>
      prev.map((p) => (p.id === id ? { ...p, status: "stopped" as const } : p)),
    );
  }, []);

  const create = useCallback(
    async (input: Parameters<typeof createProfile>[0]) => {
      const created = await createProfile(input);
      setProfiles((prev) => [...prev, created]);
      return created;
    },
    [],
  );

  const update = useCallback(
    async (id: string, input: Parameters<typeof updateProfile>[1]) => {
      const updated = await updateProfile(id, input);
      setProfiles((prev) => prev.map((p) => (p.id === id ? updated : p)));
      return updated;
    },
    [],
  );

  const remove = useCallback(async (id: string) => {
    // Returns undo info (F6): the profile goes to the trash, not the void.
    const info = await deleteProfile(id);
    setProfiles((prev) => prev.filter((p) => p.id !== id));
    return info;
  }, []);

  const duplicate = useCallback(async (id: string) => {
    const copy = await duplicateProfile(id);
    setProfiles((prev) => [...prev, copy]);
    return copy;
  }, []);

  return {
    profiles,
    loading,
    error,
    refresh,
    launch,
    stop,
    create,
    update,
    remove,
    duplicate,
    setError,
  };
}
