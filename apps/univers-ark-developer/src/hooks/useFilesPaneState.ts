import { useEffect, useEffectEvent, useMemo, useState } from "react";
import { listRemoteDirectory, readRemoteFilePreview } from "../lib/tauri";
import type {
  DeveloperTarget,
  RemoteDirectoryListing,
  RemoteFileEntry,
  RemoteFilePreview,
} from "../types";

export interface FilesTreeNode {
  entry: RemoteFileEntry;
  depth: number;
  isExpanded: boolean;
  isLoading: boolean;
}

export function formatFileSize(size: number): string {
  if (size < 1024) return `${size} B`;
  if (size < 1024 * 1024) return `${(size / 1024).toFixed(1)} KB`;
  return `${(size / (1024 * 1024)).toFixed(1)} MB`;
}

export function languageFromPath(path: string): string {
  const ext = path.split(".").pop()?.toLowerCase();
  if (!ext) return "plaintext";

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
    sh: "shell",
    bash: "shell",
    zsh: "shell",
    fish: "shell",
    yml: "yaml",
    yaml: "yaml",
    json: "json",
    toml: "plaintext",
    xml: "xml",
    html: "html",
    svg: "xml",
    css: "css",
    scss: "scss",
    less: "less",
    sql: "sql",
    md: "markdown",
    dockerfile: "dockerfile",
    makefile: "plaintext",
    lua: "lua",
    php: "php",
    r: "r",
    ini: "ini",
    conf: "ini",
    diff: "plaintext",
    vue: "html",
    graphql: "graphql",
  };

  return map[ext] ?? "plaintext";
}

function buildFlatTree(
  rootPath: string | null,
  childrenByPath: Map<string, RemoteFileEntry[]>,
  expandedPaths: Set<string>,
  loadingPaths: Set<string>,
): FilesTreeNode[] {
  if (!rootPath) {
    return [];
  }

  const nodes: FilesTreeNode[] = [];

  const walk = (dirPath: string, depth: number) => {
    const children = childrenByPath.get(dirPath);
    if (!children) {
      return;
    }

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
}

export function useFilesPaneState(active: boolean, target: DeveloperTarget) {
  const [childrenByPath, setChildrenByPath] = useState<Map<string, RemoteFileEntry[]>>(
    new Map(),
  );
  const [expandedPaths, setExpandedPaths] = useState<Set<string>>(new Set());
  const [loadingPaths, setLoadingPaths] = useState<Set<string>>(new Set());
  const [rootPath, setRootPath] = useState<string | null>(null);
  const [rootParentPath, setRootParentPath] = useState<string | null>(null);
  const [preview, setPreview] = useState<RemoteFilePreview | null>(null);
  const [selectedPath, setSelectedPath] = useState<string | null>(null);
  const [isLoadingPreview, setIsLoadingPreview] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const resetBrowser = () => {
    setChildrenByPath(new Map());
    setExpandedPaths(new Set());
    setRootPath(null);
    setRootParentPath(null);
    setPreview(null);
    setSelectedPath(null);
  };

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

        if (!rootPath || dirPath === null || dirPath === undefined) {
          setRootPath(listing.path);
          setRootParentPath(listing.parentPath ?? null);
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

  const toggleDirectory = (entry: RemoteFileEntry) => {
    const path = entry.path;

    if (expandedPaths.has(path)) {
      setExpandedPaths((prev) => {
        const next = new Set(prev);
        next.delete(path);
        return next;
      });
      return;
    }

    setExpandedPaths((prev) => new Set(prev).add(path));
    if (!childrenByPath.has(path)) {
      loadDirectory(path);
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
    resetBrowser();
    loadDirectory(rootParentPath);
  };

  const refresh = () => {
    const nextPath = rootPath;
    resetBrowser();
    loadDirectory(nextPath);
  };

  const flatTree = useMemo(
    () => buildFlatTree(rootPath, childrenByPath, expandedPaths, loadingPaths),
    [childrenByPath, expandedPaths, loadingPaths, rootPath],
  );

  const selectedEntry = useMemo(() => {
    for (const [, entries] of childrenByPath) {
      const found = entries.find((entry) => entry.path === selectedPath);
      if (found) {
        return found;
      }
    }
    return undefined;
  }, [childrenByPath, selectedPath]);

  const editorLanguage = preview ? languageFromPath(preview.path) : "plaintext";

  return {
    editorLanguage,
    error,
    flatTree,
    isLoadingPreview,
    loadingPaths,
    navigateUp,
    openFile,
    preview,
    refresh,
    rootParentPath,
    rootPath,
    selectedEntry,
    selectedPath,
    toggleDirectory,
  };
}
