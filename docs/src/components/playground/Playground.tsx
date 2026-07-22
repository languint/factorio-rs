import {
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
  type CSSProperties,
  type MouseEvent,
} from "react";
import { CodeEditor } from "./CodeEditor";
import { ContextMenu, type ContextMenuItem } from "./ContextMenu";
import { FileTree, type TreeContextTarget } from "./FileTree";
import {
  EXAMPLES,
  nextUntitledPath,
  normalizeRustPath,
  parentDirectory,
  pathsInFolder,
  rustPathToLua,
  sortedPaths,
  type PlaygroundFileMap,
} from "./examples";
import "./playground.css";

type TranspileFilesFn = (filesJson: string) => {
  ok: boolean;
  files_json?: string;
  message?: string;
  free: () => void;
};

type MenuState = {
  x: number;
  y: number;
  target: TreeContextTarget | { kind: "editor"; path: string };
};

async function loadTranspileFiles(): Promise<TranspileFilesFn> {
  const base = import.meta.env.BASE_URL.endsWith("/")
    ? import.meta.env.BASE_URL
    : `${import.meta.env.BASE_URL}/`;
  const jsUrl = new URL(
    `${base}playground/factorio_playground.js`,
    window.location.origin,
  ).href;
  const wasmUrl = new URL(
    `${base}playground/factorio_playground_bg.wasm`,
    window.location.origin,
  ).href;

  const mod = await import(/* @vite-ignore */ jsUrl);
  await mod.default({ module_or_path: wasmUrl });
  return mod.transpile_files as TranspileFilesFn;
}

const MIN_SPLIT = 20;
const MAX_SPLIT = 80;
const DEFAULT_SOURCE = 'pub fn on_init() {\n    println!("hello");\n}\n';

function defaultExample() {
  return EXAMPLES.find((example) => example.id === "full-mod") ?? EXAMPLES[0];
}

function menuItemsFor(
  target: MenuState["target"],
  rustPaths: string[],
): ContextMenuItem[] {
  if (target.kind === "folder") {
    const children = pathsInFolder(rustPaths, target.path);
    return [
      { id: "new-file", label: "New File..." },
      { id: "rename", label: "Rename Folder..." },
      {
        id: "delete",
        label: "Delete Folder",
        danger: true,
        disabled: rustPaths.length - children.length < 1,
      },
    ];
  }
  return [
    { id: "new-file", label: "New File..." },
    { id: "rename", label: "Rename..." },
    {
      id: "delete",
      label: "Delete",
      danger: true,
      disabled: rustPaths.length <= 1,
    },
  ];
}

export default function Playground() {
  const initial = defaultExample();
  const [exampleId, setExampleId] = useState(initial.id);
  const [files, setFiles] = useState<PlaygroundFileMap>({ ...initial.files });
  const [activeRust, setActiveRust] = useState(
    initial.activeFile ?? sortedPaths(initial.files)[0] ?? "",
  );
  const [luaFiles, setLuaFiles] = useState<PlaygroundFileMap>({});
  const [activeLua, setActiveLua] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [status, setStatus] = useState<"loading" | "ready" | "failed">(
    "loading",
  );
  const [split, setSplit] = useState(50);
  const [dragging, setDragging] = useState(false);
  const [menu, setMenu] = useState<MenuState | null>(null);
  const panesRef = useRef<HTMLDivElement | null>(null);
  const transpileRef = useRef<TranspileFilesFn | null>(null);

  const rustPaths = useMemo(() => sortedPaths(files), [files]);
  const luaPaths = useMemo(() => sortedPaths(luaFiles), [luaFiles]);

  useEffect(() => {
    let cancelled = false;
    loadTranspileFiles()
      .then((transpileFiles) => {
        if (cancelled) {
          return;
        }
        transpileRef.current = transpileFiles;
        setStatus("ready");
      })
      .catch((err: unknown) => {
        if (cancelled) {
          return;
        }
        setStatus("failed");
        const message =
          err instanceof Error ? err.message : "Failed to load playground WASM";
        setError(
          `${message}. Build locally with ./scripts/build-playground-wasm.sh`,
        );
      });
    return () => {
      cancelled = true;
    };
  }, []);

  const activeRustRef = useRef(activeRust);
  activeRustRef.current = activeRust;

  const run = useCallback((nextFiles: PlaygroundFileMap) => {
    const transpileFiles = transpileRef.current;
    if (!transpileFiles) {
      return;
    }
    const result = transpileFiles(JSON.stringify(nextFiles));
    try {
      if (result.ok && result.files_json) {
        const parsed = JSON.parse(result.files_json) as PlaygroundFileMap;
        setLuaFiles(parsed);
        setError(null);
        setActiveLua((current) => {
          if (current && parsed[current]) {
            return current;
          }
          if (parsed["control.lua"]) {
            return "control.lua";
          }
          const preferred = rustPathToLua(activeRustRef.current);
          if (parsed[preferred]) {
            return preferred;
          }
          const roots = ["data.lua", "settings.lua", "info.json"];
          for (const root of roots) {
            if (parsed[root]) {
              return root;
            }
          }
          return sortedPaths(parsed)[0] ?? "";
        });
      } else {
        setLuaFiles({});
        setActiveLua("");
        setError(result.message ?? "Transpile failed");
      }
    } finally {
      result.free();
    }
  }, []);

  useEffect(() => {
    if (status !== "ready") {
      return;
    }
    const handle = window.setTimeout(() => run(files), 250);
    return () => window.clearTimeout(handle);
  }, [files, status, run]);

  useEffect(() => {
    if (!dragging) {
      return;
    }

    function onPointerMove(event: PointerEvent) {
      const panes = panesRef.current;
      if (!panes) {
        return;
      }
      const rect = panes.getBoundingClientRect();
      const stacked = window.matchMedia("(max-width: 60rem)").matches;
      if (stacked) {
        if (rect.height <= 0) {
          return;
        }
        const next = ((event.clientY - rect.top) / rect.height) * 100;
        setSplit(Math.min(MAX_SPLIT, Math.max(MIN_SPLIT, next)));
        return;
      }
      if (rect.width <= 0) {
        return;
      }
      const next = ((event.clientX - rect.left) / rect.width) * 100;
      setSplit(Math.min(MAX_SPLIT, Math.max(MIN_SPLIT, next)));
    }

    function onPointerUp() {
      setDragging(false);
    }

    window.addEventListener("pointermove", onPointerMove);
    window.addEventListener("pointerup", onPointerUp);
    window.addEventListener("pointercancel", onPointerUp);
    const stacked = window.matchMedia("(max-width: 60rem)").matches;
    document.body.style.cursor = stacked ? "row-resize" : "col-resize";
    document.body.style.userSelect = "none";

    return () => {
      window.removeEventListener("pointermove", onPointerMove);
      window.removeEventListener("pointerup", onPointerUp);
      window.removeEventListener("pointercancel", onPointerUp);
      document.body.style.cursor = "";
      document.body.style.userSelect = "";
    };
  }, [dragging]);

  function onExampleChange(id: string) {
    const example = EXAMPLES.find((item) => item.id === id);
    if (!example) {
      return;
    }
    setExampleId(example.id);
    setFiles({ ...example.files });
    setActiveRust(example.activeFile ?? sortedPaths(example.files)[0] ?? "");
    setMenu(null);
  }

  function updateActiveSource(value: string) {
    if (!activeRust) {
      return;
    }
    setFiles((current) => ({ ...current, [activeRust]: value }));
  }

  function selectRust(path: string) {
    setActiveRust(path);
    const luaPath = rustPathToLua(path);
    if (luaFiles[luaPath]) {
      setActiveLua(luaPath);
    }
  }

  function openMenu(event: MouseEvent, target: MenuState["target"]) {
    event.preventDefault();
    setMenu({ x: event.clientX, y: event.clientY, target });
  }

  function createFile(directory: string) {
    const suggested = nextUntitledPath(files, directory || "control");
    const entered = window.prompt("New Rust file path (under src/)", suggested);
    if (!entered) {
      return;
    }
    const path = normalizeRustPath(entered);
    if (!path.endsWith(".rs")) {
      window.alert("Path must end with .rs");
      return;
    }
    if (files[path]) {
      window.alert("A file with that path already exists");
      return;
    }
    setFiles((current) => ({ ...current, [path]: DEFAULT_SOURCE }));
    setActiveRust(path);
  }

  function renameFile(from: string) {
    const entered = window.prompt("Rename Rust file path", from);
    if (!entered || entered === from) {
      return;
    }
    const path = normalizeRustPath(entered);
    if (!path.endsWith(".rs")) {
      window.alert("Path must end with .rs");
      return;
    }
    if (files[path] && path !== from) {
      window.alert("A file with that path already exists");
      return;
    }
    setFiles((current) => {
      const next = { ...current };
      next[path] = next[from] ?? "";
      delete next[from];
      return next;
    });
    if (activeRust === from) {
      setActiveRust(path);
    }
  }

  function deleteFile(path: string) {
    if (rustPaths.length <= 1) {
      return;
    }
    setFiles((current) => {
      const next = { ...current };
      delete next[path];
      return next;
    });
    if (activeRust === path) {
      const remaining = rustPaths.filter((item) => item !== path);
      setActiveRust(remaining[0] ?? "");
    }
  }

  function renameFolder(folder: string) {
    const entered = window.prompt("Rename folder path", folder);
    if (!entered || entered === folder) {
      return;
    }
    const nextFolder = normalizeRustPath(entered).replace(/\/$/u, "");
    if (!nextFolder) {
      window.alert("Folder path cannot be empty");
      return;
    }
    const children = pathsInFolder(rustPaths, folder);
    const renamed = children.map((path) => ({
      from: path,
      to: `${nextFolder}${path.slice(folder.length)}`,
    }));
    if (renamed.some(({ to }) => files[to] && !to.startsWith(`${folder}/`))) {
      window.alert("A file already exists at the destination");
      return;
    }
    setFiles((current) => {
      const next = { ...current };
      for (const { from, to } of renamed) {
        next[to] = next[from] ?? "";
        delete next[from];
      }
      return next;
    });
    if (activeRust === folder || activeRust.startsWith(`${folder}/`)) {
      setActiveRust(`${nextFolder}${activeRust.slice(folder.length)}`);
    }
  }

  function deleteFolder(folder: string) {
    const children = pathsInFolder(rustPaths, folder);
    if (children.length === 0) {
      return;
    }
    if (rustPaths.length - children.length < 1) {
      window.alert("Cannot delete the last remaining files");
      return;
    }
    setFiles((current) => {
      const next = { ...current };
      for (const path of children) {
        delete next[path];
      }
      return next;
    });
    if (activeRust === folder || activeRust.startsWith(`${folder}/`)) {
      const remaining = rustPaths.filter(
        (path) => !path.startsWith(`${folder}/`),
      );
      setActiveRust(remaining[0] ?? "");
    }
  }

  function onMenuSelect(id: string) {
    if (!menu) {
      return;
    }
    const { target } = menu;
    setMenu(null);

    if (id === "new-file") {
      if (target.kind === "folder") {
        createFile(target.path);
      } else if (target.kind === "file" || target.kind === "editor") {
        createFile(parentDirectory(target.path) || "control");
      } else {
        createFile("control");
      }
      return;
    }

    if (id === "rename") {
      if (target.kind === "folder") {
        renameFolder(target.path);
      } else if (target.kind === "file" || target.kind === "editor") {
        renameFile(target.path);
      }
      return;
    }

    if (id === "delete") {
      if (target.kind === "folder") {
        deleteFolder(target.path);
      } else if (target.kind === "file" || target.kind === "editor") {
        deleteFile(target.path);
      }
    }
  }

  return (
    <div className="fr-playground">
      <div className="fr-playground__toolbar">
        <label className="fr-playground__field">
          <span>Example</span>
          <select
            value={exampleId}
            onChange={(event) => onExampleChange(event.target.value)}
          >
            {EXAMPLES.map((example) => (
              <option key={example.id} value={example.id}>
                {example.label}
              </option>
            ))}
          </select>
        </label>
        <span className="fr-playground__status" data-status={status}>
          {status === "loading" && "Loading WASM..."}
          {status === "ready" && "Live transpile"}
          {status === "failed" && "WASM missing"}
        </span>
      </div>

      <div
        ref={panesRef}
        className={`fr-playground__panes${dragging ? " is-dragging" : ""}`}
        style={{ "--fr-split": `${split}%` } as CSSProperties}
      >
        <div className="fr-playground__label fr-playground__label--rust">
          Rust
        </div>
        <div
          className="fr-playground__splitter"
          role="separator"
          aria-orientation="vertical"
          aria-valuenow={Math.round(split)}
          aria-valuemin={MIN_SPLIT}
          aria-valuemax={MAX_SPLIT}
          aria-label="Resize Rust and mod panes"
          onPointerDown={(event) => {
            event.preventDefault();
            setDragging(true);
          }}
        />
        <div className="fr-playground__label fr-playground__label--lua">
          Mod
        </div>

        <div className="fr-playground__body fr-playground__body--rust">
          <div className="fr-playground__workspace">
            <FileTree
              paths={rustPaths}
              activePath={activeRust}
              onSelect={selectRust}
              label="Rust files"
              onContextMenu={openMenu}
            />
            <div
              className="fr-playground__editor-wrap"
              onContextMenuCapture={(event) => {
                if (!activeRust) {
                  return;
                }
                openMenu(event, { kind: "editor", path: activeRust });
              }}
            >
              <div className="fr-playground__current-path">{activeRust}</div>
              <CodeEditor
                language="rust"
                value={files[activeRust] ?? ""}
                onChange={updateActiveSource}
                className="fr-playground__editor"
              />
            </div>
          </div>
        </div>

        <div className="fr-playground__body fr-playground__body--lua">
          {error ? (
            <pre className="fr-playground__error">{error}</pre>
          ) : (
            <div className="fr-playground__workspace">
              <FileTree
                paths={luaPaths}
                activePath={activeLua}
                onSelect={setActiveLua}
                label="Mod files"
              />
              <div className="fr-playground__editor-wrap">
                <div className="fr-playground__current-path">{activeLua}</div>
                <CodeEditor
                  language="lua"
                  value={luaFiles[activeLua] ?? ""}
                  readOnly
                  className="fr-playground__editor"
                />
              </div>
            </div>
          )}
        </div>
      </div>

      {menu ? (
        <ContextMenu
          x={menu.x}
          y={menu.y}
          items={menuItemsFor(menu.target, rustPaths)}
          onSelect={onMenuSelect}
          onClose={() => setMenu(null)}
        />
      ) : null}
    </div>
  );
}
