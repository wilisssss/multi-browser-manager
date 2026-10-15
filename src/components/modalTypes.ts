import type { Folder, Profile } from "../types";

export type Modal =
  | { kind: "profileForm"; editing: Profile | null; defaultFolderId: string | null }
  | { kind: "proxyManager" }
  | { kind: "history" }
  | { kind: "help" }
  | { kind: "settings" }
  | { kind: "credentials"; profile: Profile }
  | { kind: "resources" }
  | { kind: "palette" }
  | { kind: "exportPassphrase"; fileName: string }
  | { kind: "importPassphrase"; fileName: string }
  | { kind: "folderName"; editing: Folder | null; parentId: string | null }
  | {
      kind: "folderPicker";
      folder: Folder | null;
      profileIds: string[] | null;
      currentParentId: string | null;
    }
  | null;
