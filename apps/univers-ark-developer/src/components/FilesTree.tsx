import { FileTileArtwork } from "./FileTileArtwork";
import type { RemoteFileEntry } from "../types";
import {
  formatFileSize,
  type FilesBrowserView,
  type FilesSortDirection,
  type FilesSortKey,
  type FilesTreeNode,
} from "../hooks/useFilesPaneState";

type FilesTreeProps = {
  browserView: FilesBrowserView;
  error: string | null;
  flatTree: FilesTreeNode[];
  isLoading: boolean;
  onEnterDirectory: (entry: RemoteFileEntry) => void;
  onOpenFile: (entry: RemoteFileEntry) => void;
  onSelectEntry: (entry: RemoteFileEntry) => void;
  onSortBy: (key: FilesSortKey) => void;
  onToggleDirectory: (entry: RemoteFileEntry) => void;
  selectedPath: string | null;
  sortDirection: FilesSortDirection;
  sortKey: FilesSortKey;
};

function entryIcon(entry: RemoteFileEntry, isExpanded: boolean): string {
  if (entry.kind === "directory") {
    return isExpanded ? "⌄" : "›";
  }
  if (entry.kind === "symlink") {
    return "↗";
  }
  return "•";
}

function iconViewTone(entry: RemoteFileEntry): "default" | "code" | "json" | "text" {
  const extension = entry.name.split(".").pop()?.toLowerCase();
  if (extension === "json") {
    return "json";
  }

  if (
    extension &&
    [
      "ts",
      "tsx",
      "js",
      "jsx",
      "py",
      "rs",
      "go",
      "java",
      "kt",
      "c",
      "cc",
      "cpp",
      "hpp",
      "css",
      "scss",
      "html",
      "xml",
      "yml",
      "yaml",
      "toml",
      "sh",
      "bash",
      "zsh",
      "md",
    ].includes(extension)
  ) {
    return extension === "md" ? "text" : "code";
  }

  return "default";
}

function normalizeEntryKind(kind: string): "directory" | "file" | "symlink" {
  if (kind === "directory" || kind === "symlink") {
    return kind;
  }

  return "file";
}

export function FilesTree({
  browserView,
  error,
  flatTree,
  isLoading,
  onEnterDirectory,
  onOpenFile,
  onSelectEntry,
  onSortBy,
  onToggleDirectory,
  selectedPath,
  sortDirection,
  sortKey,
}: FilesTreeProps) {
  return (
    <section className="files-tree">
      {error ? (
        <div className="files-empty-state">
          <p className="files-empty-title">Unavailable</p>
          <p className="files-empty-copy">{error}</p>
        </div>
      ) : isLoading ? (
        <div className="files-empty-state">
          <p className="files-empty-title">Loading</p>
          <p className="files-empty-copy">Fetching remote directory listing.</p>
        </div>
      ) : flatTree.length > 0 ? (
        <div className={`files-tree-entries is-${browserView}`}>
          {browserView === "details" ? (
            <div className="files-tree-header" role="presentation">
              <button
                className={`files-tree-header-button ${sortKey === "name" ? "is-active" : ""}`}
                onClick={() => onSortBy("name")}
                type="button"
              >
                Name
                <span className="files-tree-sort-indicator">
                  {sortKey === "name"
                    ? sortDirection === "asc"
                      ? "↑"
                      : "↓"
                    : ""}
                </span>
              </button>
              <button
                className={`files-tree-header-button ${sortKey === "kind" ? "is-active" : ""}`}
                onClick={() => onSortBy("kind")}
                type="button"
              >
                Kind
                <span className="files-tree-sort-indicator">
                  {sortKey === "kind"
                    ? sortDirection === "asc"
                      ? "↑"
                      : "↓"
                    : ""}
                </span>
              </button>
              <button
                className={`files-tree-header-button is-align-right ${sortKey === "size" ? "is-active" : ""}`}
                onClick={() => onSortBy("size")}
                type="button"
              >
                Size
                <span className="files-tree-sort-indicator">
                  {sortKey === "size"
                    ? sortDirection === "asc"
                      ? "↑"
                      : "↓"
                    : ""}
                </span>
              </button>
            </div>
          ) : null}
          {flatTree.map((node) => (
            <button
              className={`files-tree-entry is-${browserView} ${node.entry.kind === "directory" ? "is-directory" : node.entry.kind === "symlink" ? "is-symlink" : "is-file"} ${selectedPath === node.entry.path ? "is-selected" : ""}`}
              key={node.entry.path}
              onClick={() => {
                if (browserView === "tree" && node.entry.kind === "directory") {
                  onToggleDirectory(node.entry);
                  return;
                }

                if (node.entry.kind === "directory") {
                  onSelectEntry(node.entry);
                } else {
                  onOpenFile(node.entry);
                }
              }}
              onDoubleClick={() => {
                if (browserView !== "tree" && node.entry.kind === "directory") {
                  onEnterDirectory(node.entry);
                }
              }}
              style={
                browserView === "tree"
                  ? { paddingLeft: `${0.5 + node.depth * 1}rem` }
                  : undefined
              }
              type="button"
            >
              <span className="files-tree-icon">
                {node.isLoading ? (
                  "…"
                ) : browserView === "icons" ? (
                  <span className="files-tree-icon-tile">
                    <FileTileArtwork
                      kind={normalizeEntryKind(node.entry.kind)}
                      tone={iconViewTone(node.entry)}
                    />
                  </span>
                ) : browserView === "details" ? (
                  <span className="files-tree-mini-artwork">
                    <FileTileArtwork
                      kind={normalizeEntryKind(node.entry.kind)}
                      tone={iconViewTone(node.entry)}
                    />
                  </span>
                ) : (
                  entryIcon(node.entry, node.isExpanded)
                )}
              </span>
              <span className="files-tree-name">
                {node.entry.name}
                {browserView !== "icons" && node.entry.kind === "directory"
                  ? "/"
                  : ""}
              </span>
              {browserView === "details" ? (
                <span className="files-tree-kind">
                  {node.entry.kind === "directory"
                    ? "Folder"
                    : node.entry.kind === "symlink"
                      ? "Alias"
                      : "File"}
                </span>
              ) : null}
              {node.entry.kind === "file" ? (
                <span className="files-tree-size">
                  {formatFileSize(node.entry.size)}
                </span>
              ) : browserView === "details" ? (
                <span className="files-tree-size">—</span>
              ) : null}
            </button>
          ))}
        </div>
      ) : (
        <div className="files-empty-state">
          <p className="files-empty-title">Empty</p>
          <p className="files-empty-copy">No files were found in this directory.</p>
        </div>
      )}
    </section>
  );
}
