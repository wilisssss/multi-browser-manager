import { useState } from "react";
import { Download, Loader2, Pencil, Plug, Plus, Trash2, X, Zap } from "lucide-react";
import type { useProxies } from "../hooks/useProxies";
import { Select } from "./Select";
import type { CreateProxyInput, Proxy, ProxyExitTest, ProxyImportResult, ProxyTestResult } from "../types";

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
  const { proxies, testsByProxy, create, update, remove, resetHistory, test, importFromEndpoint } = proxiesState;

  const [form, setForm] = useState<typeof EMPTY_FORM & { id: string | null }>({
    ...EMPTY_FORM,
    id: null,
  });
  const [showForm, setShowForm] = useState(false);
  const [showImport, setShowImport] = useState(false);
  const [importForm, setImportForm] = useState({ endpoint: "", defaultProtocol: "http", labelPrefix: "" });
  const [importing, setImporting] = useState(false);
  const [importResult, setImportResult] = useState<ProxyImportResult | null>(null);
  const [importError, setImportError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  const [testing, setTesting] = useState<string | null>(null);
  const [testResults, setTestResults] = useState<Record<string, ProxyTestResult>>({});
  const [resetState, setResetState] = useState<{ id: string; error: string | null } | null>(null);
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

  const handleImport = async (e: React.FormEvent) => {
    e.preventDefault();
    setImportError(null);
    setImportResult(null);
    setImporting(true);
    try {
      const result = await importFromEndpoint({
        endpoint: importForm.endpoint.trim(),
        defaultProtocol: importForm.defaultProtocol,
        labelPrefix: importForm.labelPrefix.trim() || null,
      });
      setImportResult(result);
      if (result.imported > 0) {
        setImportForm({ endpoint: "", defaultProtocol: "http", labelPrefix: "" });
      }
    } catch (err) {
      setImportError(String(err));
    } finally {
      setImporting(false);
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
        [id]: {
          success: false,
          message: String(err),
          latencyMs: null,
          ip: null,
          country: null,
          countryCode: null,
          asn: null,
          asnOrg: null,
          fresh: true,
          previouslySeen: null,
        },
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
              onClick={() => { setShowImport((v) => !v); setImportResult(null); setImportError(null); }}
              className="flex items-center gap-1.5 rounded-lg border border-neutral-300 px-3 py-1.5 text-xs font-medium text-neutral-700 hover:bg-neutral-100 dark:border-neutral-700 dark:text-neutral-300 dark:hover:bg-neutral-800"
              title="Fetch a proxy list from an HTTP(S) endpoint"
            >
              <Download className="h-3.5 w-3.5" /> Import from URL
            </button>
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
                      { value: "https", label: "HTTPS" },
                      { value: "socks4", label: "SOCKS4" },
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

          {showImport && (
            <form onSubmit={handleImport} className="mb-4 space-y-3 rounded-lg border border-blue-900/60 bg-blue-950/20 p-4">
              <div className="flex items-center justify-between">
                <h3 className="text-sm font-semibold">Import proxies from URL</h3>
                <button type="button" onClick={() => setShowImport(false)} className="text-neutral-500 hover:text-neutral-300">
                  <X className="h-4 w-4" />
                </button>
              </div>
              <p className="text-xs text-neutral-500 dark:text-neutral-400">
                Fetches the list from the endpoint and creates one proxy per entry. Passwords are stored in the OS keychain and never shown or logged.
              </p>
              <div className="grid grid-cols-2 gap-3">
                <div className="col-span-2">
                  <label className="mb-1 block text-xs text-neutral-500 dark:text-neutral-400">Endpoint URL</label>
                  <input
                    value={importForm.endpoint}
                    onChange={(e) => setImportForm({ ...importForm, endpoint: e.target.value })}
                    placeholder="https://provider.example.com/proxies?key=…"
                    className="w-full rounded-lg border border-neutral-300 bg-white px-3 py-2 text-sm outline-none focus:border-blue-600 dark:border-neutral-800 dark:bg-neutral-900"
                  />
                </div>
                <div>
                  <label className="mb-1 block text-xs text-neutral-500 dark:text-neutral-400">Default protocol</label>
                  <Select
                    value={importForm.defaultProtocol}
                    onChange={(v) => setImportForm({ ...importForm, defaultProtocol: v })}
                    options={[
                      { value: "http", label: "HTTP" },
                      { value: "https", label: "HTTPS" },
                      { value: "socks4", label: "SOCKS4" },
                      { value: "socks5", label: "SOCKS5" },
                    ]}
                  />
                </div>
                <div>
                  <label className="mb-1 block text-xs text-neutral-500 dark:text-neutral-400">Label prefix (optional)</label>
                  <input
                    value={importForm.labelPrefix}
                    onChange={(e) => setImportForm({ ...importForm, labelPrefix: e.target.value })}
                    placeholder="e.g. ISP — "
                    className="w-full rounded-lg border border-neutral-300 bg-white px-3 py-2 text-sm outline-none focus:border-blue-600 dark:border-neutral-800 dark:bg-neutral-900"
                  />
                </div>
              </div>
              <details className="text-xs text-neutral-500 dark:text-neutral-400">
                <summary className="cursor-pointer select-none">Accepted formats</summary>
                <pre className="mt-2 overflow-x-auto rounded-lg bg-neutral-950 p-2 leading-5 text-neutral-400">{
                  'http://user:pass@host:port\n' +
                  'user:pass@host:port\n' +
                  '1.2.3.4:3128:user:pass\n' +
                  '1.2.3.4:3128\n' +
                  'host:port user:pass\n' +
                  '[{"ip":"1.2.3.4","port":3128,"username":"u","password":"p","protocol":"socks5"}]'
                }</pre>
              </details>
              {importError && (
                <div className="rounded-lg border border-red-900 bg-red-950/50 px-3 py-2 text-xs text-red-200">
                  {importError}
                </div>
              )}
              {importResult && (
                <div className={`rounded-lg px-3 py-2 text-xs ${
                  importResult.imported > 0
                    ? "border border-green-900 bg-green-950/40 text-green-200"
                    : "border border-neutral-800 bg-neutral-950 text-neutral-300"
                }`}>
                  <p>
                    <strong>{importResult.imported}</strong> imported · {importResult.duplicates} duplicates skipped · {importResult.invalid} invalid
                  </p>
                  {importResult.errors.length > 0 && (
                    <ul className="mt-1 max-h-40 space-y-0.5 overflow-y-auto font-mono">
                      {importResult.errors.map((e, i) => (
                        <li key={i}>{e}</li>
                      ))}
                    </ul>
                  )}
                </div>
              )}
              <div className="flex justify-end gap-2 pt-1">
                <button
                  type="button"
                  onClick={() => setShowImport(false)}
                  className="rounded-lg px-4 py-2 text-sm text-neutral-600 hover:bg-neutral-100 dark:text-neutral-300 dark:hover:bg-neutral-800"
                >
                  Cancel
                </button>
                <button
                  type="submit"
                  disabled={importing}
                  className="flex items-center gap-1.5 rounded-lg bg-blue-600 px-4 py-2 text-sm font-medium text-white hover:bg-blue-500 disabled:opacity-50"
                >
                  {importing && <Loader2 className="h-4 w-4 animate-spin" />}
                  Import
                </button>
              </div>
            </form>
          )}

          {proxies.length === 0 && !showForm && !showImport ? (
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
                          {result.country ? ` · ${result.country}${result.countryCode ? ` (${result.countryCode})` : ""}` : ""}
                          {result.asn ? ` · ${result.asn}${result.asnOrg ? ` ${result.asnOrg}` : ""}` : ""}
                        </p>
                      )}
                      {result && result.success && result.ip && (
                        result.fresh ? (
                          <p className="mt-1 text-xs text-emerald-300">
                            <span className="rounded-full border border-emerald-800 bg-emerald-950/40 px-2 py-0.5">fresh</span>
                            <span className="ml-2">This exit IP has never appeared in a previous test.</span>
                          </p>
                        ) : result.previouslySeen && (
                          <p className="mt-1 text-xs text-amber-300">
                            <span className="rounded-full border border-amber-800 bg-amber-950/40 px-2 py-0.5">used before</span>
                            <span className="ml-2">
                              This exit IP was returned by {result.previouslySeen.count} earlier test
                              {result.previouslySeen.count === 1 ? "" : "s"} (last{" "}
                              {new Date(result.previouslySeen.lastSeenAt * 1000).toLocaleString(undefined, {
                                dateStyle: "medium",
                                timeStyle: "short",
                              })}
                              ) — for a rotating proxy, test again to draw a different exit IP.
                            </span>
                          </p>
                        )
                      )}
                      <HistoryStrip
                        proxy={proxy}
                        tests={testsByProxy[proxy.id] ?? []}
                        onReset={async (id: string) => {
                          setResetState({ id, error: null });
                          try {
                            await resetHistory(id);
                            setResetState(null);
                          } catch (err) {
                            setResetState({ id, error: String(err) });
                          }
                        }}
                        resetState={resetState}
                        error={error}
                      />
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

/** The 3 most recent exit IPs, with freshness badges + reset button. */
function HistoryStrip({
  proxy,
  tests,
  onReset,
  resetState,
  error,
}: {
  proxy: Proxy;
  tests: ProxyExitTest[];
  onReset: (id: string) => Promise<void> | void;
  resetState: { id: string; error: string | null } | null;
  error: string | null;
}) {
  if (!tests || tests.length === 0) {
    return (
      <p className="mt-1 text-[11px] text-neutral-500 dark:text-neutral-400">
        No successful test yet — a rotating proxy's new exit IP will be flagged as used before.
      </p>
    );
  }
  const recent = tests.slice(0, 3);
  const countByIp = new Map<string, number>();
  for (const t of tests) {
    countByIp.set(t.ip, (countByIp.get(t.ip) ?? 0) + 1);
  }
  const resetError = resetState?.id === proxy.id ? resetState.error : null;
  return (
    <div className="mt-2">
      <div className="flex flex-wrap items-center gap-1.5">
        {recent.map((t) => {
          const used = (countByIp.get(t.ip) ?? 0) > 1;
          const when = new Date(t.testedAt * 1000).toLocaleString(undefined, {
            dateStyle: "medium",
            timeStyle: "short",
          });
          return (
            <span
              key={t.id || `${t.ip}-${t.testedAt}`}
              className={`inline-flex items-center gap-1 rounded-full border px-2 py-0.5 text-[11px] ${
                used
                  ? "border-amber-800 bg-amber-950/40 text-amber-300"
                  : "border-emerald-800 bg-emerald-950/40 text-emerald-300"
              }`}
              title={`${t.ip}${t.country ? ` · ${t.country}` : ""}${t.asn ? ` · ${t.asn}${t.asnOrg ? ` ${t.asnOrg}` : ""}` : ""} — tested ${when}`}
            >
              <span className={`h-1.5 w-1.5 rounded-full ${used ? "bg-amber-400" : "bg-emerald-400"}`} />
              <span className="font-mono">{t.ip}</span>
              <span className="text-[10px] uppercase tracking-wide">
                {used ? `seen ${countByIp.get(t.ip!)}×` : "fresh"}
              </span>
            </span>
          );
        })}
      </div>
      <div className="mt-1 flex items-center gap-2">
        <button
          onClick={() => onReset(proxy.id)}
          className="text-[11px] text-neutral-500 underline decoration-dotted underline-offset-2 hover:text-neutral-300"
          title="Forget every exit IP this proxy has shown, so the next test starts from an empty set"
        >
          Reset freshness
        </button>
        {(resetError || error) && (
          <span className="text-[11px] text-red-400">{resetError ?? error}</span>
        )}
      </div>
    </div>
  );
}
