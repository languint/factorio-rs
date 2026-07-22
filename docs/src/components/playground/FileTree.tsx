import { useEffect, useMemo, useState, type MouseEvent } from "react";
import {
  buildFileTree,
  folderKeysForPath,
  type TreeNode,
} from "./fileTree";

export type TreeContextTarget =
  | { kind: "file"; path: string }
  | { kind: "folder"; path: string }
  | { kind: "root" };

type FileTreeProps = {
  paths: string[];
  activePath: string;
  onSelect: (path: string) => void;
  label: string;
  onContextMenu?: (event: MouseEvent, target: TreeContextTarget) => void;
};

function TreeItems({
  nodes,
  depth,
  parentKey,
  activePath,
  expanded,
  onToggle,
  onSelect,
  onContextMenu,
}: {
  nodes: TreeNode[];
  depth: number;
  parentKey: string;
  activePath: string;
  expanded: Set<string>;
  onToggle: (key: string) => void;
  onSelect: (path: string) => void;
  onContextMenu?: FileTreeProps["onContextMenu"];
}) {
  return (
    <ul className="fr-tree__list" role={depth === 0 ? "tree" : "group"}>
      {nodes.map((node) => {
        const key = parentKey ? `${parentKey}/${node.name}` : node.name;
        if (node.children) {
          const isOpen = expanded.has(key);
          return (
            <li key={key} role="treeitem" aria-expanded={isOpen}>
              <button
                type="button"
                className="fr-tree__row fr-tree__row--dir"
                style={{ paddingInlineStart: `${0.35 + depth * 0.7}rem` }}
                onClick={() => onToggle(key)}
                onContextMenu={(event) => {
                  event.preventDefault();
                  event.stopPropagation();
                  onContextMenu?.(event, { kind: "folder", path: key });
                }}
              >
                <span
                  className={
                    isOpen
                      ? "fr-tree__chevron is-open"
                      : "fr-tree__chevron"
                  }
                  aria-hidden="true"
                />
                <span className="fr-tree__name">{node.name}</span>
              </button>
              {isOpen ? (
                <TreeItems
                  nodes={node.children}
                  depth={depth + 1}
                  parentKey={key}
                  activePath={activePath}
                  expanded={expanded}
                  onToggle={onToggle}
                  onSelect={onSelect}
                  onContextMenu={onContextMenu}
                />
              ) : null}
            </li>
          );
        }

        const path = node.path ?? key;
        const isActive = path === activePath;
        return (
          <li key={key} role="treeitem">
            <button
              type="button"
              className={
                isActive
                  ? "fr-tree__row fr-tree__row--file is-active"
                  : "fr-tree__row fr-tree__row--file"
              }
              style={{ paddingInlineStart: `${0.35 + depth * 0.7}rem` }}
              aria-current={isActive ? "true" : undefined}
              onClick={() => onSelect(path)}
              onContextMenu={(event) => {
                event.preventDefault();
                event.stopPropagation();
                onContextMenu?.(event, { kind: "file", path });
              }}
            >
              <span className="fr-tree__chevron fr-tree__chevron--spacer" />
              <span className="fr-tree__name">{node.name}</span>
            </button>
          </li>
        );
      })}
    </ul>
  );
}

export function FileTree({
  paths,
  activePath,
  onSelect,
  label,
  onContextMenu,
}: FileTreeProps) {
  const nodes = useMemo(() => buildFileTree(paths), [paths]);
  const [expanded, setExpanded] = useState<Set<string>>(
    () => new Set(folderKeysForPath(activePath)),
  );

  useEffect(() => {
    setExpanded((current) => {
      const next = new Set(current);
      for (const path of paths) {
        for (const key of folderKeysForPath(path)) {
          next.add(key);
        }
      }
      for (const key of folderKeysForPath(activePath)) {
        next.add(key);
      }
      return next;
    });
  }, [activePath, paths]);

  function onToggle(key: string) {
    setExpanded((current) => {
      const next = new Set(current);
      if (next.has(key)) {
        next.delete(key);
      } else {
        next.add(key);
      }
      return next;
    });
  }

  return (
    <nav
      className="fr-tree"
      aria-label={label}
      onContextMenu={(event) => {
        event.preventDefault();
        onContextMenu?.(event, { kind: "root" });
      }}
    >
      <TreeItems
        nodes={nodes}
        depth={0}
        parentKey=""
        activePath={activePath}
        expanded={expanded}
        onToggle={onToggle}
        onSelect={onSelect}
        onContextMenu={onContextMenu}
      />
    </nav>
  );
}
