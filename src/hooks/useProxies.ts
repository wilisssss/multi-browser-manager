import { useCallback, useEffect, useState } from "react";
import type {
  ImportProxiesInput,
  Proxy,
  ProxyExitTest,
  ProxyImportResult,
  ProxyTestResult,
} from "../types";
import {
  createProxy,
  deleteProxy,
  getProxies,
  getProxyTests,
  importProxiesFromUrl,
  resetProxyHistory,
  testProxy,
  updateProxy,
} from "../lib/tauri-api";

export function useProxies() {
  const [proxies, setProxies] = useState<Proxy[]>([]);
  const [testsByProxy, setTestsByProxy] = useState<Record<string, ProxyExitTest[]>>({});
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const refreshTests = useCallback(async (list: Proxy[]) => {
    const entries = await Promise.all(
      list.map(async (proxy) => {
        try {
          return [proxy.id, await getProxyTests(proxy.id)] as const;
        } catch {
          return [proxy.id, []] as const;
        }
      }),
    );
    setTestsByProxy(Object.fromEntries(entries));
  }, []);

  const refresh = useCallback(async () => {
    try {
      const list = await getProxies();
      setProxies(list);
      await refreshTests(list);
      setError(null);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }, [refreshTests]);


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
    setTestsByProxy((prev) => {
      const next = { ...prev };
      delete next[id];
      return next;
    });
  }, []);

  const resetHistory = useCallback(async (id: string) => {
    await resetProxyHistory({ proxyId: id });
    setTestsByProxy((prev) => ({ ...prev, [id]: [] }));
  }, []);

  const test = useCallback(
    async (id: string): Promise<ProxyTestResult> => {
      const result = await testProxy(id);
      // The backend has now recorded the run; re-read this proxy's history
      // so the fresh/used strip matches the persisted truth.
      try {
        const mine = await getProxyTests(id);
        setTestsByProxy((prev) => ({ ...prev, [id]: mine }));
      } catch {
        /* A failed re-read must not mask the test result itself. */
      }
      return result;
    },
    [],
  );

  const importFromEndpoint = useCallback(async (input: ImportProxiesInput) => {
    const result: ProxyImportResult = await importProxiesFromUrl(input);
    await refresh();
    return result;
  }, [refresh]);

  return { proxies, loading, error, testsByProxy, refresh, create, update, remove, resetHistory, test, importFromEndpoint, setError };
}
