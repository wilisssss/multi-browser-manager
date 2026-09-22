import { useState } from "react";
import { Loader2, Pencil, Plug, Plus, Trash2, X, Zap } from "lucide-react";
import type { useProxies } from "../hooks/useProxies";
import { Select } from "./Select";
import type { CreateProxyInput, Proxy, ProxyTestResult } from "../types";

interface Props {
  proxiesState: ReturnType<typeof useProxies>;
  onClose: () => void;
}

const EMPTY_FORM = {
  label: "",
  protocol: "http",
  host: "",
  port: 8080,
  username: "",
  password: "",
};

export function ProxyManager({ proxiesState, onClose }: Props) {
  const { proxies, create, update, remove, test } = proxiesState;

  const [form, setForm] = useState<typeof EMPTY_FORM & { id: string | null }>({
    ...EMPTY_FORM,
    id: null,
  });
  const [showForm, setShowForm] = useState(false);
  const [saving, setSaving] = useState(false);
  const [testing, setTesting] = useState<string | null>(null);
  const [testResults, setTestResults] = useState<Record<string, ProxyTestResult>>({});
  const [error, setError] = useState<string | null>(null);

  const openCreate = () => {
    setForm({ ...EMPTY_FORM, id: null });
    setShowForm(true);
  };

  const openEdit = (proxy: Proxy) => {
    setForm({
      id: proxy.id,
      label: proxy.label,
      protocol: proxy.protocol,
      host: proxy.host,
      port: proxy.port,
      username: proxy.username ?? "",
      // Password is write-only: leave blank to keep the stored one.
      password: "",
    });
    setShowForm(true);
  };

  const handleSave = async (e: React.FormEvent) => {
    e.preventDefault();
    setError(null);
    setSaving(true);
    try {
      if (form.id) {
        await update(form.id, {
          label: form.label,
          protocol: form.protocol,
          host: form.host,
          port: form.port,
          username: form.username || null,
          ...(form.password
            ? { password: form.password }
            : // Username cleared → proxy becomes no-auth; drop the stale password too.
              form.username
              ? {}
              : { password: null }),
        });
      } else {
        const input: CreateProxyInput = {
          label: form.label,
          protocol: form.protocol,
          host: form.host,
          port: form.port,
          username: form.username || null,
          password: form.password || null,
        };
        await create(input);
      }
      setShowForm(false);
    } catch (err) {
      setError(String(err));
    } finally {
      setSaving(false);
    }
  };

  const handleTest = async (id: string) => {
    setTesting(id);
    try {
      const result = await test(id);
      setTestResults((prev) => ({ ...prev, [id]: result }));
    } catch (err) {
      setTestResults((prev) => ({
        ...prev,
        [id]: { success: false, message: String(err), latencyMs: null, ip: null },
      }));
    } finally {
      setTesting(null);
    }
  };

  const handleDelete = async (id: string) => {
    setError(null);
    try {
      await remove(id);
    } catch (err) {
      setError(String(err));
    }
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/70 p-4">
      <div className="flex max-h-[85vh] w-full max-w-2xl flex-col rounded-xl border border-neutral-200 bg-white dark:border-neutral-800 dark:bg-neutral-900">
        <div className="flex items-center justify-between border-b border-neutral-200 px-6 py-4 dark:border-neutral-800">
          <h2 className="text-base font-semibold">Proxy Manager</h2>
          <div className="flex items-center gap-2">
            <button
              onClick={openCreate}
              className="flex items-center gap-1.5 rounded-lg bg-blue-600 px-3 py-1.5 text-xs font-medium text-white hover:bg-blue-500"
            >
              <Plus className="h-3.5 w-3.5" /> Add proxy
            </button>
            <button onClick={onClose} className="text-neutral-500 hover:text-neutral-300">
              <X className="h-5 w-5" />
            </button>
          </div>
        </div>

        <div className="flex-1 overflow-y-auto px-6 py-4">
          {error && (
            <div className="mb-3 rounded-lg border border-red-900 bg-red-950/50 px-3 py-2 text-sm text-red-200">
              {error}
            </div>
          )}

          {showForm && (
            <form
              onSubmit={handleSave}
              className="mb-4 space-y-3 rounded-lg border border-neutral-800 bg-neutral-950 p-4"
            >
              <div className="grid grid-cols-2 gap-3">
                <div className="col-span-2">
                  <label className="mb-1 block text-xs text-neutral-500 dark:text-neutral-400">Label</label>
                  <input
                    value={form.label}
                    onChange={(e) => setForm({ ...form, label: e.target.value })}
                    placeholder="e.g. Datacenter SG-01"
                    className="w-full rounded-lg border border-neutral-300 bg-white px-3 py-2 text-sm outline-none focus:border-blue-600 dark:border-neutral-800 dark:bg-neutral-900"
                  />
                </div>
                <div>
                  <label className="mb-1 block text-xs text-neutral-500 dark:text-neutral-400">Protocol</label>
                  <Select
                    value={form.protocol}
                    onChange={(v) => setForm({ ...form, protocol: v })}
                    options={[
                      { value: "http", label: "HTTP" },
                      { value: "socks5", label: "SOCKS5" },
                    ]}
                  />
                </div>
                <div>
                  <label className="mb-1 block text-xs text-neutral-500 dark:text-neutral-400">Port</label>
                  <input
                    type="number"
                    min={1}
                    max={65535}
                    value={form.port}
                    onChange={(e) => setForm({ ...form, port: Number(e.target.value) })}
                    className="w-full rounded-lg border border-neutral-300 bg-white px-3 py-2 text-sm outline-none focus:border-blue-600 dark:border-neutral-800 dark:bg-neutral-900"
                  />
                </div>
                <div className="col-span-2">
                  <label className="mb-1 block text-xs text-neutral-500 dark:text-neutral-400">Host</label>
                  <input
                    value={form.host}
                    onChange={(e) => setForm({ ...form, host: e.target.value })}
                    placeholder="proxy.example.com"
                    className="w-full rounded-lg border border-neutral-300 bg-white px-3 py-2 text-sm outline-none focus:border-blue-600 dark:border-neutral-800 dark:bg-neutral-900"
                  />
                </div>
                <div>
                  <label className="mb-1 block text-xs text-neutral-500 dark:text-neutral-400">
                    Username (optional)
                  </label>
                  <input
                    value={form.username}
                    onChange={(e) => setForm({ ...form, username: e.target.value })}
                    className="w-full rounded-lg border border-neutral-300 bg-white px-3 py-2 text-sm outline-none focus:border-blue-600 dark:border-neutral-800 dark:bg-neutral-900"
                  />
                </div>
                <div>
                  <label className="mb-1 block text-xs text-neutral-500 dark:text-neutral-400">
                    {form.id ? "New password (blank = keep)" : "Password (optional)"}
                  </label>
                  <input
                    type="password"
                    value={form.password}
                    onChange={(e) => setForm({ ...form, password: e.target.value })}
                    className="w-full rounded-lg border border-neutral-300 bg-white px-3 py-2 text-sm outline-none focus:border-blue-600 dark:border-neutral-800 dark:bg-neutral-900"
                  />
                </div>
              </div>

              <div className="flex justify-end gap-2 pt-1">
                <button
                  type="button"
                  onClick={() => setShowForm(false)}
                  className="rounded-lg px-4 py-2 text-sm text-neutral-600 hover:bg-neutral-100 dark:text-neutral-300 dark:hover:bg-neutral-800"
                >
                  Cancel
                </button>
                <button
                  type="submit"
                  disabled={saving}
                  className="flex items-center gap-1.5 rounded-lg bg-blue-600 px-4 py-2 text-sm font-medium text-white hover:bg-blue-500 disabled:opacity-50"
                >
                  {saving && <Loader2 className="h-4 w-4 animate-spin" />}
                  {form.id ? "Save" : "Add"}
                </button>
              </div>
            </form>
          )}

          {proxies.length === 0 && !showForm ? (
            <div className="flex flex-col items-center gap-2 py-12 text-center">
              <Plug className="h-8 w-8 text-neutral-700" />
              <p className="text-sm text-neutral-400">
                No proxies configured. Add one to route a profile through it.
              </p>
            </div>
          ) : (
            <ul className="space-y-2">
              {proxies.map((proxy) => {
                const result = testResults[proxy.id];
                return (
                  <li
                    key={proxy.id}
                    className="flex items-center justify-between gap-3 rounded-lg border border-neutral-200 bg-neutral-50 px-4 py-3 dark:border-neutral-800 dark:bg-neutral-950"
                  >
                    <div className="min-w-0">
                      <p className="truncate text-sm font-medium">{proxy.label}</p>
                      <p className="truncate text-xs text-neutral-500">
                        {proxy.protocol}://{proxy.host}:{proxy.port}
                        {proxy.username ? ` · ${proxy.username}` : ""}
                      </p>
                      {result && (
                        <p
                          className={`mt-1 text-xs ${
                            result.success ? "text-green-400" : "text-red-400"
                          }`}
                        >
                          {result.message}
                          {result.ip ? ` · exit IP: ${result.ip}` : ""}
                        </p>
                      )}
                    </div>
                    <div className="flex shrink-0 items-center gap-1">
                      <button
                        onClick={() => handleTest(proxy.id)}
                        title="Test connection"
                        className="flex items-center gap-1 rounded-lg px-2.5 py-1.5 text-xs text-neutral-600 hover:bg-neutral-100 dark:text-neutral-300 dark:hover:bg-neutral-800"
                      >
                        {testing === proxy.id ? (
                          <Loader2 className="h-3.5 w-3.5 animate-spin" />
                        ) : (
                          <Zap className="h-3.5 w-3.5" />
                        )}
                        Test
                      </button>
                      <button
                        onClick={() => openEdit(proxy)}
                        title="Edit"
                        className="rounded-lg p-1.5 text-neutral-400 hover:bg-neutral-100 hover:text-neutral-700 dark:hover:bg-neutral-800 dark:hover:text-neutral-200"
                      >
                        <Pencil className="h-4 w-4" />
                      </button>
                      <button
                        onClick={() => handleDelete(proxy.id)}
                        title="Delete"
                        className="rounded-lg p-1.5 text-neutral-400 hover:bg-red-50 hover:text-red-600 dark:hover:bg-red-950 dark:hover:text-red-400"
                      >
                        <Trash2 className="h-4 w-4" />
                      </button>
                    </div>
                  </li>
                );
              })}
            </ul>
          )}
        </div>
      </div>
    </div>
  );
}
