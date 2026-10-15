import { useEffect, useState } from "react";
import { Loader2, X } from "lucide-react";
import { detectBrowsers } from "../lib/tauri-api";
import { PHONE_DEVICES, phoneByUserAgent } from "../lib/userAgents";
import { Select } from "./Select";
import type { BrowserInfo, CreateProfileInput, Fingerprint, Profile, Proxy } from "../types";

interface Props {
  profile: Profile | null;
  proxies: Proxy[];
  /** All folders, for the "which folder does this live in" select. */
  folders: import("../types").Folder[];
  /** Preselected folder for new profiles (the folder being browsed). */
  defaultFolderId?: string | null;
  /** Preselected browser type for new profiles (from settings). */
  defaultBrowserType?: string;
  onClose: () => void;
  onSubmit: (input: CreateProfileInput) => Promise<void>;
}

export function ProfileForm({ profile, proxies, folders, defaultFolderId, defaultBrowserType, onClose, onSubmit }: Props) {
  const [browsers, setBrowsers] = useState<BrowserInfo[]>([]);
  const [name, setName] = useState(profile?.name ?? "");
  const [browserType, setBrowserType] = useState(profile?.browserType ?? defaultBrowserType ?? "");
  const [proxyId, setProxyId] = useState<string>(profile?.proxyId ?? "");
  const [notes, setNotes] = useState(profile?.notes ?? "");
  const [startupUrl, setStartupUrl] = useState(profile?.startupUrl ?? "");
  const [restartOnCrash, setRestartOnCrash] = useState(profile?.restartOnCrash ?? false);
  const [folderId, setFolderId] = useState<string>(profile?.folderId ?? defaultFolderId ?? "");
  const fp = profile?.fingerprint ?? null;
  const [language, setLanguage] = useState(fp?.language ?? "");
  /** UA dropdown value: "" = default, a device id, or "custom". */
  const initialPhone = phoneByUserAgent(fp?.userAgent);
  const [uaMode, setUaMode] = useState<string>(
    initialPhone ? initialPhone.id : fp?.userAgent ? "custom" : "",
  );
  const [customUserAgent, setCustomUserAgent] = useState(
    initialPhone ? "" : (fp?.userAgent ?? ""),
  );
  const [screenText, setScreenText] = useState(
    fp?.screen ? `${fp.screen.width}x${fp.screen.height}` : "",
  );
  const [timezone, setTimezone] = useState(fp?.timezone ?? "");
  const [platform, setPlatform] = useState(fp?.platform ?? "");
  const [geoLat, setGeoLat] = useState<number | "">(fp?.geolocation?.latitude ?? "");
  const [geoLon, setGeoLon] = useState<number | "">(fp?.geolocation?.longitude ?? "");
  const [fontsText, setFontsText] = useState((fp?.fonts ?? []).join(", "));
  const [disableWebRTC, setDisableWebRTC] = useState(fp?.disableWebRTC ?? false);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    detectBrowsers()
      .then((list) => {
        setBrowsers(list);
        if (!profile && list.length > 0) {
          setBrowserType((current) => current || list[0].browserType);
        }
      })
      .catch((e) => setError(String(e)));
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  /** Picks a UA option. A phone device also fills the portrait screen size. */
  const chooseUaMode = (mode: string) => {
    setUaMode(mode);
    const device = PHONE_DEVICES.find((d) => d.id === mode);
    if (device) {
      setScreenText(`${device.screen.width}x${device.screen.height}`);
    }
  };

  const userAgentValue = (): string | null => {
    if (uaMode === "custom") return customUserAgent.trim() || null;
    const device = PHONE_DEVICES.find((d) => d.id === uaMode);
    return device ? device.userAgent : null;
  };

  /** Parses "WIDTHxHEIGHT" (x or ×). Empty = no override. */
  const parseScreen = (): { width: number; height: number } | null | "invalid" => {
    const raw = screenText.trim();
    if (!raw) return null;
    const m = raw.match(/^(\d+)\s*[x×]\s*(\d+)$/i);
    if (!m) return "invalid";
    return { width: Number(m[1]), height: Number(m[2]) };
  };

  /** Builds the fingerprint from the form; empty fields are left unset. */
  const buildFingerprint = (): Fingerprint | null => {
    const screen = parseScreen();
    const fonts = fontsText
      .split(",")
      .map((f) => f.trim())
      .filter(Boolean);
    const hasGeo = geoLat !== "" && geoLon !== "";
    const fp: Fingerprint = {
      language: language.trim() || null,
      userAgent: userAgentValue(),
      timezone: timezone.trim() || null,
      platform: platform.trim() || null,
      geolocation: hasGeo ? { latitude: Number(geoLat), longitude: Number(geoLon) } : null,
      fonts: fonts.length > 0 ? fonts : null,
      screen: screen === "invalid" ? null : screen,
      // Only sent when on: the backend field is a plain bool, not an Option.
      ...(disableWebRTC ? { disableWebRTC: true } : {}),
    };
    const empty = Object.values(fp).every((v) => v === null);
    return empty ? null : fp;
  };

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!name.trim() || !browserType) {
      setError("Name and browser are required.");
      return;
    }
    if ((geoLat === "") !== (geoLon === "")) {
      setError("Geolocation needs both latitude and longitude.");
      return;
    }
    if (parseScreen() === "invalid") {
      setError("Screen size must look like 412x915.");
      return;
    }
    setSaving(true);
    setError(null);
    try {
      await onSubmit({
        name: name.trim(),
        browserType,
        proxyId: proxyId || null,
        notes: notes.trim() || null,
        restartOnCrash,
        folderId: folderId || null,
        fingerprint: buildFingerprint(),
        startupUrl: startupUrl.trim() || null,
      });
    } catch (err) {
      setError(String(err));
    } finally {
      setSaving(false);
    }
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/70 p-4">
      <div className="flex max-h-[90vh] w-full max-w-md flex-col rounded-xl border border-neutral-200 bg-white p-6 shadow-xl dark:border-neutral-800 dark:bg-neutral-900">
        <div className="mb-4 flex items-center justify-between">
          <h2 className="text-base font-semibold">
            {profile ? "Edit Profile" : "New Profile"}
          </h2>
          <button onClick={onClose} className="text-neutral-500 hover:text-neutral-700 dark:hover:text-neutral-300">
            <X className="h-5 w-5" />
          </button>
        </div>

        {error && (
          <div className="mb-3 rounded-lg border border-red-300 bg-red-50 px-3 py-2 text-sm text-red-700 dark:border-red-900 dark:bg-red-950/50 dark:text-red-200">
            {error}
          </div>
        )}

        <form onSubmit={handleSubmit} className="min-h-0 flex-1 space-y-4 overflow-y-auto pr-1">
          <div>
            <label className="mb-1 block text-xs font-medium text-neutral-500 dark:text-neutral-400">Name</label>
            <input
              value={name}
              onChange={(e) => setName(e.target.value)}
              placeholder="e.g. Client A – Marketing"
              autoFocus
              className="w-full rounded-lg border border-neutral-300 bg-white px-3 py-2 text-sm outline-none focus:border-blue-600 dark:border-neutral-800 dark:bg-neutral-950"
            />
            {profile && name.trim() && name.trim().toLowerCase() !== profile.name.toLowerCase() && (
              <p className="mt-1.5 rounded-md border border-amber-300/60 bg-amber-50 px-2.5 py-1.5 text-xs text-amber-800 dark:border-amber-900/60 dark:bg-amber-950/40 dark:text-amber-200">
                CLI/WM keybinds that use <code className="font-mono">mbm --launch {profile.name}</code>{" "}
                stop working after this rename — update them to{" "}
                <code className="font-mono">mbm --launch {name.trim()}</code>.
              </p>
            )}
          </div>

          <div>
            <label className="mb-1 block text-xs font-medium text-neutral-500 dark:text-neutral-400">Browser</label>
            {browsers.length === 0 ? (
              <p className="text-xs text-neutral-500">
                No Chromium browsers detected on this machine.
              </p>
            ) : (
              <Select
                value={browserType}
                onChange={setBrowserType}
                options={browsers.map((b) => ({
                  value: b.browserType,
                  label: b.name + (b.version ? ` — ${b.version}` : ""),
                }))}
                placeholder="Choose a browser"
              />
            )}
          </div>

          <div>
            <label className="mb-1 block text-xs font-medium text-neutral-500 dark:text-neutral-400">
              Proxy (optional)
            </label>
            <Select
              value={proxyId}
              onChange={setProxyId}
              options={[
                { value: "", label: "No proxy" },
                ...proxies.map((p) => ({
                  value: p.id,
                  label: `${p.label} (${p.protocol}://${p.host}:${p.port})`,
                })),
              ]}
            />
          </div>

          <div>
            <label className="mb-1 block text-xs font-medium text-neutral-500 dark:text-neutral-400">
              Folder
            </label>
            <Select
              value={folderId}
              onChange={setFolderId}
              options={[
                { value: "", label: "Unfiled" },
                ...folders.map((f) => {
                  // Indent by depth so nesting is visible in the dropdown.
                  let depth = 0;
                  let cur = f.parentId;
                  const byId = new Map(folders.map((x) => [x.id, x]));
                  while (cur) {
                    depth++;
                    cur = byId.get(cur)?.parentId ?? null;
                  }
                  return { value: f.id, label: `${"  ".repeat(depth)}${f.name}` };
                }),
              ]}
            />
          </div>

          <label className="flex items-center gap-2 text-xs font-medium text-neutral-600 dark:text-neutral-300">
            <input
              type="checkbox"
              checked={restartOnCrash}
              onChange={(e) => setRestartOnCrash(e.target.checked)}
              className="h-4 w-4 accent-blue-600"
            />
            Auto-restart on crash
          </label>

          <details className="rounded-lg border border-neutral-200 p-3 dark:border-neutral-800">
            <summary className="cursor-pointer text-xs font-medium text-neutral-600 dark:text-neutral-300">
              Fingerprint (optional)
            </summary>
            <p className="mb-3 mt-2 text-xs text-neutral-400 dark:text-neutral-500">
              Empty fields keep the browser's own values. Overrides are
              best-effort; sites may still detect the browser.
            </p>
            <div className="grid grid-cols-2 gap-3">
              <FpField label="Language" placeholder="en-US" value={language} onChange={setLanguage} />
              <FpField label="Platform" placeholder="Win32" value={platform} onChange={setPlatform} />
              <FpField label="Timezone" placeholder="Europe/Berlin" value={timezone} onChange={setTimezone} className="col-span-2" />
              <div className="col-span-2">
                <label className="mb-1 block text-xs font-medium text-neutral-500 dark:text-neutral-400">User agent</label>
                <Select
                  value={uaMode}
                  onChange={chooseUaMode}
                  options={[
                    { value: "", label: "Default (browser)" },
                    ...PHONE_DEVICES.map((d) => ({ value: d.id, label: `Phone · ${d.name}` })),
                    { value: "custom", label: "Custom" },
                  ]}
                />
              </div>
              {uaMode === "custom" && (
                <div className="col-span-2">
                  <label className="mb-1 block text-xs font-medium text-neutral-500 dark:text-neutral-400">Custom user agent</label>
                  <textarea
                    value={customUserAgent}
                    onChange={(e) => setCustomUserAgent(e.target.value)}
                    rows={3}
                    placeholder="Paste the full User-Agent string"
                    className="w-full resize-none rounded-lg border border-neutral-300 bg-white px-3 py-2 font-mono text-xs outline-none focus:border-blue-600 dark:border-neutral-800 dark:bg-neutral-950"
                  />
                </div>
              )}
              <FpField
                label="Screen (WIDTHxHEIGHT)"
                placeholder="412x915"
                value={screenText}
                onChange={setScreenText}
                className="col-span-2"
              />
              <FpNumberField label="Latitude" placeholder="52.52" value={geoLat} onChange={setGeoLat} />
              <FpNumberField label="Longitude" placeholder="13.405" value={geoLon} onChange={setGeoLon} />
              <FpField
                label="System fonts (comma-separated)"
                placeholder="Arial, Calibri, Segoe UI"
                value={fontsText}
                onChange={setFontsText}
                className="col-span-2"
              />
              <label className="col-span-2 flex items-start gap-2 text-xs font-medium text-neutral-600 dark:text-neutral-300">
                <input
                  type="checkbox"
                  checked={disableWebRTC}
                  onChange={(e) => setDisableWebRTC(e.target.checked)}
                  className="mt-0.5 h-4 w-4 accent-blue-600"
                />
                <span>
                  Disable WebRTC
                  <span className="mt-0.5 block font-normal text-neutral-400 dark:text-neutral-500">
                    Stops sites from learning your real IP through WebRTC (common
                    proxy leak). Video and voice calls will not work in this profile.
                  </span>
                </span>
              </label>
            </div>
          </details>

          <div>
            <label className="mb-1 block text-xs font-medium text-neutral-500 dark:text-neutral-400">
              Startup page (optional)
            </label>
            <input
              value={startupUrl}
              onChange={(e) => setStartupUrl(e.target.value)}
              placeholder="https://example.com"
              className="w-full rounded-lg border border-neutral-300 bg-white px-3 py-2 text-sm outline-none focus:border-blue-600 dark:border-neutral-800 dark:bg-neutral-950"
            />
            <p className="mt-1 text-xs text-neutral-400 dark:text-neutral-500">
              Opened when the browser starts. Only http:// and https:// addresses.
            </p>
          </div>

          <div>
            <label className="mb-1 block text-xs font-medium text-neutral-500 dark:text-neutral-400">
              Notes (optional)
            </label>
            <textarea
              value={notes}
              onChange={(e) => setNotes(e.target.value)}
              rows={3}
              className="w-full resize-none rounded-lg border border-neutral-300 bg-white px-3 py-2 text-sm outline-none focus:border-blue-600 dark:border-neutral-800 dark:bg-neutral-950"
            />
          </div>

          <div className="flex justify-end gap-2 pt-2">
            <button
              type="button"
              onClick={onClose}
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
              {profile ? "Save changes" : "Create profile"}
            </button>
          </div>
        </form>
      </div>
    </div>
  );
}

function FpField({
  label,
  value,
  onChange,
  placeholder,
  className = "",
}: {
  label: string;
  value: string;
  onChange: (v: string) => void;
  placeholder?: string;
  className?: string;
}) {
  return (
    <div className={className}>
      <label className="mb-1 block text-xs font-medium text-neutral-500 dark:text-neutral-400">{label}</label>
      <input
        value={value}
        placeholder={placeholder}
        onChange={(e) => onChange(e.target.value)}
        className="w-full rounded-lg border border-neutral-300 bg-white px-3 py-2 text-sm outline-none focus:border-blue-600 dark:border-neutral-800 dark:bg-neutral-950"
      />
    </div>
  );
}

function FpNumberField({
  label,
  value,
  onChange,
  placeholder,
}: {
  label: string;
  value: number | "";
  onChange: (v: number | "") => void;
  placeholder?: string;
}) {
  return (
    <div>
      <label className="mb-1 block text-xs font-medium text-neutral-500 dark:text-neutral-400">{label}</label>
      <input
        type="number"
        step="any"
        value={value}
        placeholder={placeholder}
        onChange={(e) => onChange(e.target.value === "" ? "" : Number(e.target.value))}
        className="w-full rounded-lg border border-neutral-300 bg-white px-3 py-2 text-sm outline-none focus:border-blue-600 dark:border-neutral-800 dark:bg-neutral-950"
      />
    </div>
  );
}
