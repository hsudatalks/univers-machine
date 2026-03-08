import type { RemoteFileEntry } from "../types";
import {
  formatFileSize,
  type FilesTreeNode,
} from "../hooks/useFilesPaneState";

type FilesTreeProps = {
  error: string | null;
  flatTree: FilesTreeNode[];
  isLoading: boolean;
  onOpenFile: (entry: RemoteFileEntry) => void;
  onToggleDirectory: (entry: RemoteFileEntry) => void;
  rootPath: string | null;
  selectedPath: string | null;
};

function entryIcon(entry: RemoteFileEntry, isExpanded: boolean): string {
  if (entry.kind === "directory") {
    return isExpanded ? "▾" : "▸";
  }
  return " ";
}

export function FilesTree({
  error,
  flatTree,
  isLoading,
  onOpenFile,
  onToggleDirectory,
  rootPath,
  selectedPath,
}: FilesTreeProps) {
  return (
    <section className="files-tree">
      {error ? (
        <div className="files-empty-state">
          <p className="files-empty-title">Unavailable</p>
          <p className="files-empty-copy">{error}</p>
        </div>
      ) : !rootPath && isLoading ? (
        <div className="files-empty-state">
          <p className="files-empty-title">Loading</p>
          <p className="files-empty-copy">Fetching remote directory listing.</p>
        </div>
      ) : flatTree.length > 0 ? (
        <div className="files-tree-entries">
          {flatTree.map((node) => (
            <button
              className={`files-tree-entry ${node.entry.kind === "directory" ? "is-directory" : node.entry.kind === "symlink" ? "is-symlink" : "is-file"} ${selectedPath === node.entry.path ? "is-selected" : ""}`}
              key={node.entry.path}
              onClick={() => {
                if (node.entry.kind === "directory") {
                  onToggleDirectory(node.entry);
                } else {
                  onOpenFile(node.entry);
                }
              }}
              style={{ paddingLeft: `${0.5 + node.depth * 1}rem` }}
              type="button"
            >
              <span className="files-tree-icon">
                {node.isLoading ? "…" : entryIcon(node.entry, node.isExpanded)}
              </span>
              <span className="files-tree-name">
                {node.entry.name}
                {node.entry.kind === "directory" ? "/" : ""}
              </span>
              {node.entry.kind === "file" ? (
                <span className="files-tree-size">
                  {formatFileSize(node.entry.size)}
                </span>
              ) : null}
            </button>
          ))}
        </div>
      ) : rootPath ? (
        <div className="files-empty-state">
          <p className="files-empty-title">Empty</p>
          <p className="files-empty-copy">No files were found in this directory.</p>
        </div>
      ) : null}
    </section>
  );
}
