-- Folders (feature: file-manager style organization). Profiles gain an
-- optional folder_id; the UI shows only folders at the root and reveals a
-- folder's profiles when it is opened. Folders nest via parent_id; a folder
-- deletion re-parents its contents (subfolders + profiles) to the deleted
-- folder's parent, so deleting never destroys profile data.
CREATE TABLE folders (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    parent_id TEXT REFERENCES folders(id) ON DELETE SET NULL,
    position INTEGER NOT NULL DEFAULT 0,
    created_at INTEGER NOT NULL
);
CREATE INDEX idx_folders_parent ON folders(parent_id);

ALTER TABLE profiles ADD COLUMN folder_id TEXT REFERENCES folders(id) ON DELETE SET NULL;
CREATE INDEX idx_profiles_folder ON profiles(folder_id);
