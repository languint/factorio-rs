export type TreeNode = {
  name: string;
  /** Full path for files; undefined for folders. */
  path?: string;
  children?: TreeNode[];
};

function ensureFolder(
  children: Map<string, DirEntry>,
  name: string,
): Extract<DirEntry, { kind: "dir" }> {
  const existing = children.get(name);
  if (existing?.kind === "dir") {
    return existing;
  }
  const entry: Extract<DirEntry, { kind: "dir" }> = {
    kind: "dir",
    name,
    children: new Map(),
  };
  children.set(name, entry);
  return entry;
}

type DirEntry =
  | { kind: "dir"; name: string; children: Map<string, DirEntry> }
  | { kind: "file"; name: string; path: string };

function toNodes(entries: Map<string, DirEntry>): TreeNode[] {
  const dirs: TreeNode[] = [];
  const files: TreeNode[] = [];
  for (const entry of entries.values()) {
    if (entry.kind === "dir") {
      dirs.push({
        name: entry.name,
        children: toNodes(entry.children),
      });
    } else {
      files.push({ name: entry.name, path: entry.path });
    }
  }
  dirs.sort((a, b) => a.name.localeCompare(b.name));
  files.sort((a, b) => a.name.localeCompare(b.name));
  return [...dirs, ...files];
}

/** Build a sorted file tree from flat slash-separated paths. */
export function buildFileTree(paths: string[]): TreeNode[] {
  const root = new Map<string, DirEntry>();
  for (const path of paths) {
    const parts = path.split("/").filter(Boolean);
    if (parts.length === 0) {
      continue;
    }
    let current = root;
    for (let index = 0; index < parts.length; index += 1) {
      const part = parts[index];
      const isFile = index === parts.length - 1;
      if (isFile) {
        current.set(part, { kind: "file", name: part, path });
      } else {
        current = ensureFolder(current, part).children;
      }
    }
  }
  return toNodes(root);
}

/** Ancestor folder keys for a path (`a/b/c.rs` -> `a`, `a/b`). */
export function folderKeysForPath(path: string): string[] {
  const parts = path.split("/").filter(Boolean);
  if (parts.length <= 1) {
    return [];
  }
  const keys: string[] = [];
  for (let index = 1; index < parts.length; index += 1) {
    keys.push(parts.slice(0, index).join("/"));
  }
  return keys;
}
