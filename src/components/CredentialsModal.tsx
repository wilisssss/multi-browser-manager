import { useCallback, useEffect, useState } from "react";
import { ask } from "@tauri-apps/plugin-dialog";
import {
  ArrowLeft,
  Check,
  Copy,
  Eye,
  EyeOff,
  KeyRound,
  Loader2,
  Plus,
  Sprout,
  Trash2,
  X,
} from "lucide-react";
import {
  createCredential,
  deleteCredential,
  getCredentials,
  updateCredential,
} from "../lib/tauri-api";
import { findPlatform, PLATFORMS } from "../lib/platforms";
import type { Credential, Profile } from "../types";

interface Props {
  profile: Profile;
  onClose: () => void;
}

type View = "list" | "pick" | "form";

/** Copy-to-clipboard button with a brief "copied" confirmation. */
function CopyButton({ text, title }: { text: string; title: string }) {
  const [copied, setCopied] = useState(false);
  return (
    <button
      type="button"
      disabled={!text}
      onClick={async () => {
        await navigator.clipboard.writeText(text);
        setCopied(true);
        setTimeout(() => setCopied(false), 1500);
      }}
      title={title}
      className="shrink-0 rounded-md border border-neutral-300 p-1.5 text-neutral-500 hover:text-neutral-800 disabled:opacity-40 dark:border-neutral-700 dark:text-neutral-400 dark:hover:text-neutral-200"
    >
      {copied ? <Check className="h-3.5 w-3.5 text-green-600" /> : <Copy className="h-3.5 w-3.5" />}
    </button>
  );
}

export function CredentialsModal({ profile, onClose }: Props) {
  const [view, setView] = useState<View>("list");
  const [creds, setCreds] = useState<Credential[] | null>(null);
  const [selected, setSelected] = useState<Credential | null>(null);
  const [newPlatform, setNewPlatform] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  // Form fields. On edit the secrets are pre-filled — emptying the field
  // clears the stored value on save.
  const [label, setLabel] = useState("");
  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");
  const [showPassword, setShowPassword] = useState(false);
  const [seedPhrase, setSeedPhrase] = useState("");
  const [evmAddress, setEvmAddress] = useState("");
  const [notes, setNotes] = useState("");
  const [saving, setSaving] = useState(false);

  const load = useCallback(async () => {
    try {
      setCreds(await getCredentials(profile.id));
    } catch (e) {
      setError(String(e));
      setCreds([]);
    }
  }, [profile.id]);

  useEffect(() => {
    load();
  }, [load]);

  const resetForm = () => {
    setLabel("");
    setUsername("");
    setPassword("");
    setShowPassword(false);
    setSeedPhrase("");
    setEvmAddress("");
    setNotes("");
  };

  const openForm = (cred: Credential | null, platformId: string | null) => {
    setSelected(cred);
    setNewPlatform(platformId);
    resetForm();
    if (cred) {
      const t = findPlatform(cred.platform);
      setLabel(cred.label === t.name ? "" : cred.label);
      setUsername(cred.username ?? "");
      setPassword(cred.password ?? "");
      setSeedPhrase(cred.seedPhrase ?? "");
      setEvmAddress(cred.evmAddress ?? "");
      setNotes(cred.notes ?? "");
    }
    setView("form");
  };

  const handleSave = async () => {
    setSaving(true);
    setError(null);
    try {
      if (selected) {
        await updateCredential(selected.id, {
          label: label.trim() || undefined,
          username: username.trim() || null,
          password: password || null,
          seedPhrase: seedPhrase.trim() || null,
          evmAddress: evmAddress.trim() || null,
          notes: notes.trim() || null,
        });
      } else {
        await createCredential(profile.id, {
          platform: newPlatform ?? "custom",
          label: label.trim(),
          username: username.trim() || null,
          password: password || null,
          seedPhrase: seedPhrase.trim() || null,
          evmAddress: evmAddress.trim() || null,
          notes: notes.trim() || null,
        });
      }
      await load();
      setView("list");
    } catch (e) {
      setError(String(e));
    } finally {
      setSaving(false);
    }
  };

  const handleDelete = async (cred: Credential) => {
    const ok = await ask(
      `Delete the ${findPlatform(cred.platform).name} account "${cred.label}"?`,
      { title: "Delete credential", kind: "warning", okLabel: "Delete", cancelLabel: "Cancel" },
    );
    if (!ok) return;
    try {
      await deleteCredential(cred.id);
      await load();
      setView("list");
    } catch (e) {
      setError(String(e));
    }
  };

  const platformOf = selected
    ? findPlatform(selected.platform)
    : findPlatform(newPlatform ?? "custom");
  const PlatformIcon = platformOf.icon;

  const inputClass =
    "w-full rounded-lg border border-neutral-300 bg-white px-3 py-2 text-sm outline-none focus:border-blue-600 dark:border-neutral-700 dark:bg-neutral-900";

  return (
    <div className="fixed inset-0 z-[60] flex items-center justify-center bg-black/70 p-4">
      <div className="flex max-h-[85vh] w-full max-w-md flex-col rounded-xl border border-neutral-200 bg-white p-6 shadow-xl dark:border-neutral-800 dark:bg-neutral-900">
        {/* Header */}
        <div className="mb-4 flex items-center justify-between">
          <h2 className="flex items-center gap-2 text-base font-semibold">
            {view === "list" ? (
              <>
                <KeyRound className="h-4 w-4" /> Credentials — {profile.name}
              </>
            ) : (
              <button
                onClick={() => setView(creds && creds.length > 0 ? "list" : "pick")}
                className="flex items-center gap-1.5 text-sm text-neutral-500 hover:text-neutral-800 dark:hover:text-neutral-200"
              >
                <ArrowLeft className="h-4 w-4" /> Back
              </button>
            )}
          </h2>
          <button onClick={onClose} className="text-neutral-500 hover:text-neutral-700 dark:hover:text-neutral-300">
            <X className="h-5 w-5" />
          </button>
        </div>

        {error && <p className="mb-3 text-sm text-red-600 dark:text-red-400">{error}</p>}

        {/* LIST */}
        {view === "list" && (
          <div className="flex flex-col gap-2 overflow-y-auto">
            {creds === null ? (
              <div className="flex justify-center py-10 text-neutral-400">
                <Loader2 className="h-5 w-5 animate-spin" />
              </div>
            ) : creds.length === 0 ? (
              <div className="rounded-lg border border-dashed border-neutral-300 py-8 text-center text-sm text-neutral-400 dark:border-neutral-800">
                No accounts saved for this profile yet.
              </div>
            ) : (
              creds.map((cred) => {
                const t = findPlatform(cred.platform);
                const Icon = t.icon;
                return (
                  <div
                    key={cred.id}
                    className="flex items-center gap-3 rounded-lg border border-neutral-200 px-3 py-2.5 hover:bg-neutral-100 dark:border-neutral-800 dark:hover:bg-neutral-800"
                  >
                    <button
                      onClick={() => openForm(cred, null)}
                      className="flex min-w-0 flex-1 items-center gap-3 text-left"
                    >
                      <Icon className="h-5 w-5 shrink-0" />
                      <span className="min-w-0 flex-1">
                        <span className="block truncate text-sm font-medium">{cred.label}</span>
                        <span className="block truncate text-xs text-neutral-400">
                          {cred.username ?? t.name}
                        </span>
                      </span>
                    </button>
                    <span className="flex shrink-0 items-center gap-1 text-neutral-300 dark:text-neutral-600">
                      {cred.password && <CopyButton text={cred.password} title="Copy password" />}
                      {cred.seedPhrase && (
                        <CopyButton text={cred.seedPhrase} title="Copy seed phrase" />
                      )}
                    </span>
                    <button
                      onClick={() => openForm(cred, null)}
                      className="shrink-0 rounded-md p-1.5 text-neutral-400 hover:text-neutral-700 dark:hover:text-neutral-200"
                      title="Edit"
                    >
                      <Eye className="h-4 w-4" />
                    </button>
                  </div>
                );
              })
            )}
            <button
              onClick={() => setView("pick")}
              className="mt-2 flex items-center justify-center gap-1.5 rounded-lg border border-dashed border-neutral-300 py-2.5 text-sm text-neutral-500 hover:border-blue-600 hover:text-blue-600 dark:border-neutral-700"
            >
              <Plus className="h-4 w-4" /> Add account
            </button>
          </div>
        )}

        {/* PICK PLATFORM */}
        {view === "pick" && (
          <div className="grid grid-cols-3 gap-2 overflow-y-auto">
            {PLATFORMS.map((t) => {
              const Icon = t.icon;
              return (
                <button
                  key={t.id}
                  onClick={() => openForm(null, t.id)}
                  className="flex flex-col items-center gap-2 rounded-lg border border-neutral-200 px-2 py-3 text-xs hover:border-blue-600 hover:bg-blue-50 dark:border-neutral-800 dark:hover:border-blue-600 dark:hover:bg-blue-950/30"
                >
                  <Icon className="h-6 w-6" />
                  {t.name}
                </button>
              );
            })}
          </div>
        )}

        {/* FORM */}
        {view === "form" && (
          <div className="flex flex-col gap-3 overflow-y-auto pr-1">
            <div className="flex items-center gap-2.5 rounded-lg bg-neutral-100 px-3 py-2.5 dark:bg-neutral-800/60">
              <PlatformIcon className="h-5 w-5" />
              <span className="text-sm font-medium">{platformOf.name}</span>
              {selected && (
                <button
                  onClick={() => handleDelete(selected)}
                  className="ml-auto rounded-md p-1.5 text-neutral-400 hover:bg-red-50 hover:text-red-600 dark:hover:bg-red-950 dark:hover:text-red-400"
                  title="Delete this account"
                >
                  <Trash2 className="h-4 w-4" />
                </button>
              )}
            </div>

            <div>
              <label className="mb-1 block text-sm font-medium">Label</label>
              <input
                value={label}
                onChange={(e) => setLabel(e.target.value)}
                placeholder={`e.g. ${platformOf.name} main`}
                className={inputClass}
              />
            </div>

            <div>
              <label className="mb-1 block text-sm font-medium">{platformOf.usernameLabel}</label>
              <input
                value={username}
                onChange={(e) => setUsername(e.target.value)}
                placeholder={platformOf.usernamePlaceholder}
                className={inputClass}
              />
            </div>

            {platformOf.hasPassword && (
              <div>
                <label className="mb-1 flex items-center justify-between text-sm font-medium">
                  <span className="flex items-center gap-1.5">
                    <KeyRound className="h-3.5 w-3.5 text-neutral-400" /> Password
                  </span>
                  <span className="flex items-center gap-1.5">
                    <CopyButton text={password} title="Copy password" />
                    <button
                      type="button"
                      onClick={() => setShowPassword((v) => !v)}
                      title={showPassword ? "Hide" : "Show"}
                      className="shrink-0 rounded-md border border-neutral-300 p-1.5 text-neutral-500 hover:text-neutral-800 dark:border-neutral-700 dark:text-neutral-400 dark:hover:text-neutral-200"
                    >
                      {showPassword ? (
                        <EyeOff className="h-3.5 w-3.5" />
                      ) : (
                        <Eye className="h-3.5 w-3.5" />
                      )}
                    </button>
                  </span>
                </label>
                <input
                  type={showPassword ? "text" : "password"}
                  value={password}
                  onChange={(e) => setPassword(e.target.value)}
                  placeholder="password"
                  className={inputClass}
                />
              </div>
            )}

            {platformOf.hasSeedPhrase && (
              <div>
                <label className="mb-1 flex items-center justify-between text-sm font-medium">
                  <span className="flex items-center gap-1.5">
                    <Sprout className="h-3.5 w-3.5 text-neutral-400" /> Seed phrase
                  </span>
                  <CopyButton text={seedPhrase} title="Copy seed phrase" />
                </label>
                <textarea
                  value={seedPhrase}
                  onChange={(e) => setSeedPhrase(e.target.value)}
                  rows={3}
                  placeholder="twelve / twenty-four words"
                  className="w-full resize-none rounded-lg border border-neutral-300 bg-white px-3 py-2 font-mono text-sm outline-none focus:border-blue-600 dark:border-neutral-700 dark:bg-neutral-900"
                />
              </div>
            )}

            {platformOf.hasEvmAddress && (
              <div>
                <label className="mb-1 flex items-center justify-between text-sm font-medium">
                  <span>EVM address</span>
                  <CopyButton text={evmAddress} title="Copy address" />
                </label>
                <input
                  value={evmAddress}
                  onChange={(e) => setEvmAddress(e.target.value)}
                  placeholder="0x…"
                  className="w-full rounded-lg border border-neutral-300 bg-white px-3 py-2 font-mono text-sm outline-none focus:border-blue-600 dark:border-neutral-700 dark:bg-neutral-900"
                />
              </div>
            )}

            <div>
              <label className="mb-1 block text-sm font-medium">Notes</label>
              <textarea
                value={notes}
                onChange={(e) => setNotes(e.target.value)}
                rows={2}
                placeholder="2FA backup codes, recovery email, …"
                className="w-full resize-none rounded-lg border border-neutral-300 bg-white px-3 py-2 text-sm outline-none focus:border-blue-600 dark:border-neutral-700 dark:bg-neutral-900"
              />
            </div>

            <div className="flex justify-end gap-2">
              <button
                onClick={() => setView(creds && creds.length > 0 ? "list" : "pick")}
                className="rounded-lg px-3 py-2 text-sm text-neutral-600 hover:bg-neutral-100 dark:text-neutral-300 dark:hover:bg-neutral-800"
              >
                Cancel
              </button>
              <button
                onClick={handleSave}
                disabled={saving}
                className="flex items-center gap-1.5 rounded-lg bg-blue-600 px-4 py-2 text-sm font-medium text-white hover:bg-blue-500 disabled:opacity-50"
              >
                {saving && <Loader2 className="h-4 w-4 animate-spin" />} Save
              </button>
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
