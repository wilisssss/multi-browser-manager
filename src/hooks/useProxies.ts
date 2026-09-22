import { useCallback, useEffect, useState } from "react";
import type { Proxy, ProxyTestResult } from "../types";
import {
  createProxy,
  deleteProxy,
  getProxies,
  testProxy,
  updateProxy,
} from "../lib/tauri-api";

export function useProxies() {
  const [proxies, setProxies] = useState<Proxy[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    try {
      const list = await getProxies();
      setProxies(list);
      setError(null);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    refresh();
  }, [refresh]);

  const create = useCallback(
    async (input: Parameters<typeof createProxy>[0]) => {
      const created = await createProxy(input);
      setProxies((prev) => [...prev, created]);
      return created;
    },
    [],
  );

  const update = useCallback(
    async (id: string, input: Parameters<typeof updateProxy>[1]) => {
      const updated = await updateProxy(id, input);
      setProxies((prev) => prev.map((p) => (p.id === id ? updated : p)));
      return updated;
    },
    [],
  );

  const remove = useCallback(async (id: string) => {
    await deleteProxy(id);
    setProxies((prev) => prev.filter((p) => p.id !== id));
  }, []);

  const test = useCallback(
    async (id: string): Promise<ProxyTestResult> => testProxy(id),
    [],
  );

  return { proxies, loading, error, refresh, create, update, remove, test, setError };
}
