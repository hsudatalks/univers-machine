import { useEffect, useEffectEvent, useMemo, useRef, useState } from "react";
import hljs from "highlight.js";
import { listRemoteDirectory, readRemoteFilePreview } from "../lib/tauri";
import type {
  DeveloperTarget,
  RemoteDirectoryListing,
  RemoteFileEntry,
  RemoteFilePreview,
} from "../types";

interface FilesPaneProps {
  active: boolean;
  target: DeveloperTarget;
}

/* ─── Helpers ─────────────────────────────────────────── */

function entryIcon(entry: RemoteFileEntry, isExpanded: boolean): string {
  if (entry.kind === "directory") {
    return isExpanded ? "▾" : "▸";
  }
  return " ";
}

function formatFileSize(size: number): string {
  if (size < 1024) return `${size} B`;
  if (size < 1024 * 1024) return `${(size / 1024).toFixed(1)} KB`;
  return `${(size / (1024 * 1024)).toFixed(1)} MB`;
}

/** Map file extension to highlight.js language alias. */
function languageFromPath(path: string): string | undefined {
  const ext = path.split(".").pop()?.toLowerCase();
  if (!ext) return undefined;

  const map: Record<string, string> = {
    ts: "typescript",
    tsx: "typescript",
    js: "javascript",
    jsx: "javascript",
    mjs: "javascript",
    cjs: "javascript",
    py: "python",
    rs: "rust",
    go: "go",
    rb: "ruby",
    java: "java",
    kt: "kotlin",
    c: "c",
    h: "c",
    cpp: "cpp",
    cc: "cpp",
    hpp: "cpp",
    cs: "csharp",
    swift: "swift",
    sh: "bash",
    bash: "bash",
    zsh: "bash",
    fish: "bash",
    yml: "yaml",
    yaml: "yaml",
    json: "json",
    toml: "toml",
    xml: "xml",
    html: "xml",
    svg: "xml",
    css: "css",
    scss: "scss",
    less: "less",
    sql: "sql",
    md: "markdown",
    dockerfile: "dockerfile",
    makefile: "makefile",
    lua: "lua",
    php: "php",
    r: "r",
    zig: "zig",
    nix: "nix",
    tf: "hcl",
    ini: "ini",
    conf: "ini",
    diff: "diff",
    vue: "xml",
  };

  return map[ext];
}

/* ─── Tree node for flattened rendering ───────────────── */

interface TreeNode {
  entry: RemoteFileEntry;
  depth: number;
  isExpanded: boolean;
  isLoading: boolean;
}

/* ─── Component ───────────────────────────────────────── */

export function FilesPane({ active, target }: FilesPaneProps) {
  // Tree state: maps directory path → its children entries
  const [childrenByPath, setChildrenByPath] = useState<Map<string, RemoteFileEntry[]>>(new Map());
  const [expandedPaths, setExpandedPaths] = useState<Set<string>>(new Set());
  const [loadingPaths, setLoadingPaths] = useState<Set<string>>(new Set());
  const [rootPath, setRootPath] = useState<string | null>(null);
  const [rootParentPath, setRootParentPath] = useState<string | null>(null);

  // Preview state
  const [preview, setPreview] = useState<RemoteFilePreview | null>(null);
  const [selectedPath, setSelectedPath] = useState<string | null>(null);
  const [isLoadingPreview, setIsLoadingPreview] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const codeRef = useRef<HTMLElement>(null);

  /* ─── Highlight code after preview changes ──────────── */

  useEffect(() => {
    if (!codeRef.current || !preview || preview.isBinary) return;

    const el = codeRef.current;
    el.textContent = preview.content;
    el.removeAttribute("data-highlighted");

    const lang = languageFromPath(preview.path);
    if (lang) {
      el.className = `language-${lang}`;
    } else {
      el.className = "";
    }

    hljs.highlightElement(el);
  }, [preview]);

  /* ─── Directory loading ─────────────────────────────── */

  const loadDirectory = (dirPath: string | null | undefined) => {
    const targetPath = dirPath ?? undefined;
    const pathKey = dirPath ?? "__root__";

    setLoadingPaths((prev) => new Set(prev).add(pathKey));
    setError(null);

    void listRemoteDirectory(target.id, targetPath)
      .then((listing: RemoteDirectoryListing) => {
        setChildrenByPath((prev) => {
          const next = new Map(prev);
          next.set(listing.path, listing.entries);
          return next;
        });

        // If this is the first load (root), set root path
        if (!rootPath || dirPath === null || dirPath === undefined) {
          setRootPath(listing.path);
          setRootParentPath(listing.parentPath ?? null);

          // Auto-expand root
          setExpandedPaths((prev) => new Set(prev).add(listing.path));
        }
      })
      .catch((loadError) => {
        setError(
          loadError instanceof Error
            ? loadError.message
            : "Failed to load remote files.",
        );
      })
      .finally(() => {
        setLoadingPaths((prev) => {
          const next = new Set(prev);
          next.delete(pathKey);
          return next;
        });
      });
  };

  const loadDirectoryFromEffect = useEffectEvent((nextPath?: string | null) => {
    loadDirectory(nextPath);
  });

  useEffect(() => {
    if (!active || rootPath) return;
    loadDirectoryFromEffect();
  }, [active, rootPath, target.id]);

  /* ─── Tree toggle / navigate ────────────────────────── */

  const toggleDirectory = (entry: RemoteFileEntry) => {
    const path = entry.path;

    if (expandedPaths.has(path)) {
      setExpandedPaths((prev) => {
        const next = new Set(prev);
        next.delete(path);
        return next;
      });
    } else {
      setExpandedPaths((prev) => new Set(prev).add(path));

      // Lazy-load children if we haven't yet
      if (!childrenByPath.has(path)) {
        loadDirectory(path);
      }
    }
  };

  const openFile = (entry: RemoteFileEntry) => {
    setSelectedPath(entry.path);
    setIsLoadingPreview(true);
    setError(null);

    void readRemoteFilePreview(target.id, entry.path)
      .then((nextPreview) => {
        setPreview(nextPreview);
      })
      .catch((loadError) => {
        setError(
          loadError instanceof Error
            ? loadError.message
            : "Failed to load file preview.",
        );
      })
      .finally(() => {
        setIsLoadingPreview(false);
      });
  };

  const navigateUp = () => {
    if (!rootParentPath) return;

    // Reset tree and load parent directory as new root
    setChildrenByPath(new Map());
    setExpandedPaths(new Set());
    setRootPath(null);
    setPreview(null);
    setSelectedPath(null);
    loadDirectory(rootParentPath);
  };

  /* ─── Flatten tree for rendering ────────────────────── */

  const flatTree = useMemo(() => {
    if (!rootPath) return [];

    const nodes: TreeNode[] = [];

    const walk = (dirPath: string, depth: number) => {
      const children = childrenByPath.get(dirPath);
      if (!children) return;

      for (const entry of children) {
        const isDir = entry.kind === "directory";
        const isExpanded = isDir && expandedPaths.has(entry.path);
        const isLoading = loadingPaths.has(entry.path);

        nodes.push({ entry, depth, isExpanded, isLoading });

        if (isDir && isExpanded) {
          walk(entry.path, depth + 1);
        }
      }
    };

    walk(rootPath, 0);
    return nodes;
  }, [rootPath, childrenByPath, expandedPaths, loadingPaths]);

  /* ─── Selected entry info ───────────────────────────── */

  const selectedEntry = useMemo(
    () => {
      for (const [, entries] of childrenByPath) {
        const found = entries.find((e) => e.path === selectedPath);
        if (found) return found;
      }
      return undefined;
    },
    [childrenByPath, selectedPath],
  );

  /* ─── Line numbers for preview ──────────────────────── */

  const lineCount = preview && !preview.isBinary
    ? preview.content.split("\n").length
    : 0;

  /* ─── Render ────────────────────────────────────────── */

  return (
    <article className="panel tool-panel files-panel">
      <header className="panel-header tool-panel-header">
        <div className="tool-panel-heading">
          <span className="panel-title">Explorer</span>
          <code className="tool-panel-path">
            {rootPath ?? "Loading remote workspace"}
          </code>
        </div>

        <div className="tool-panel-actions">
          <button
            className="panel-button panel-button-toolbar"
            disabled={!rootParentPath || loadingPaths.size > 0}
            onClick={navigateUp}
            type="button"
          >
            Up
          </button>
          <button
            className="panel-button panel-button-toolbar"
            disabled={loadingPaths.size > 0}
            onClick={() => {
              setChildrenByPath(new Map());
              setExpandedPaths(new Set());
              setRootPath(null);
              setPreview(null);
              setSelectedPath(null);
              loadDirectory(rootPath);
            }}
            type="button"
          >
            Refresh
          </button>
        </div>
      </header>

      <div className="files-panel-body">
        {/* ── Explorer Tree ── */}
        <section className="files-tree">
          {error && !preview ? (
            <div className="files-empty-state">
              <p className="files-empty-title">Unavailable</p>
              <p className="files-empty-copy">{error}</p>
            </div>
          ) : !rootPath && loadingPaths.size > 0 ? (
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
                      toggleDirectory(node.entry);
                    } else {
                      openFile(node.entry);
                    }
                  }}
                  style={{ paddingLeft: `${0.5 + node.depth * 1}rem` }}
                  type="button"
                >
                  <span className="files-tree-icon">
                    {node.isLoading ? "…" : entryIcon(node.entry, node.isExpanded)}
                  </span>
                  <span className="files-tree-name">
                    {node.entry.name}{node.entry.kind === "directory" ? "/" : ""}
                  </span>
                  {node.entry.kind === "file" && (
                    <span className="files-tree-size">
                      {formatFileSize(node.entry.size)}
                    </span>
                  )}
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

        {/* ── Editor Preview ── */}
        <section className="files-editor">
          {isLoadingPreview ? (
            <div className="files-empty-state">
              <p className="files-empty-title">Loading</p>
              <p className="files-empty-copy">Rendering file preview.</p>
            </div>
          ) : preview ? (
            <>
              <div className="files-editor-tab-bar">
                <span className="files-editor-tab is-active">
                  {selectedEntry?.name ?? preview.path.split("/").pop() ?? preview.path}
                </span>
                {preview.truncated ? (
                  <span className="content-chip">Preview truncated</span>
                ) : null}
              </div>

              {preview.isBinary ? (
                <div className="files-empty-state">
                  <p className="files-empty-title">Binary file</p>
                  <p className="files-empty-copy">
                    Text preview is not available for this file type.
                  </p>
                </div>
              ) : (
                <div className="files-editor-code-area">
                  <div className="files-editor-gutter" aria-hidden="true">
                    {Array.from({ length: lineCount }, (_, i) => (
                      <span key={i} className="files-editor-line-number">
                        {i + 1}
                      </span>
                    ))}
                  </div>
                  <pre className="files-editor-code"><code ref={codeRef} /></pre>
                </div>
              )}
            </>
          ) : (
            <div className="files-empty-state">
              <p className="files-empty-title">Select a file</p>
              <p className="files-empty-copy">
                Choose a file to inspect its contents.
              </p>
            </div>
          )}
        </section>
      </div>
    </article>
  );
}
