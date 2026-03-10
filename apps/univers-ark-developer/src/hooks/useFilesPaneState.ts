import { useEffect, useEffectEvent, useMemo, useState } from "react";
import { listRemoteDirectory, readRemoteFilePreview } from "../lib/tauri";
import type {
  DeveloperTarget,
  RemoteDirectoryListing,
  RemoteFileEntry,
  RemoteFilePreview,
} from "../types";

function targetFilesRoot(target: DeveloperTarget): string {
  const filesRoot = target.workspace?.filesRoot?.trim();
  return filesRoot && filesRoot.length > 0 ? filesRoot : "~/repos";
}

export interface FilesTreeNode {
  entry: RemoteFileEntry;
  depth: number;
  isExpanded: boolean;
  isLoading: boolean;
}

export interface FilesRootOption {
  label: string;
  path: string;
}

export type FilesBrowserView = "tree" | "list" | "icons" | "details";
export type FilesSortKey = "name" | "kind" | "size";
export type FilesSortDirection = "asc" | "desc";

function isVisibleEntry(
  entry: RemoteFileEntry,
  showHiddenDirectories: boolean,
): boolean {
  if (showHiddenDirectories) {
    return true;
  }

  return !(entry.kind === "directory" && entry.name.startsWith("."));
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

function isPreviewableEntry(entry: RemoteFileEntry): boolean {
  return entry.kind === "file" || entry.kind === "symlink";
}

function buildFlatTree(
  rootPath: string | null,
  childrenByPath: Map<string, RemoteFileEntry[]>,
  expandedPaths: Set<string>,
  loadingPaths: Set<string>,
  showHiddenDirectories: boolean,
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
      if (!isVisibleEntry(entry, showHiddenDirectories)) {
        continue;
      }

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

function buildDirectoryEntries(
  currentDirectoryPath: string | null,
  childrenByPath: Map<string, RemoteFileEntry[]>,
  showHiddenDirectories: boolean,
  sortKey: FilesSortKey,
  sortDirection: FilesSortDirection,
): FilesTreeNode[] {
  if (!currentDirectoryPath) {
    return [];
  }

  const children = (childrenByPath.get(currentDirectoryPath) ?? [])
    .filter((entry) => isVisibleEntry(entry, showHiddenDirectories))
    .slice()
    .sort((left, right) => {
      if (left.kind !== right.kind) {
        if (left.kind === "directory") return -1;
        if (right.kind === "directory") return 1;
      }

      let result = 0;
      if (sortKey === "size") {
        result = left.size - right.size;
      } else if (sortKey === "kind") {
        result = left.kind.localeCompare(right.kind);
        if (result === 0) {
          result = left.name.localeCompare(right.name, undefined, {
            numeric: true,
            sensitivity: "base",
          });
        }
      } else {
        result = left.name.localeCompare(right.name, undefined, {
          numeric: true,
          sensitivity: "base",
        });
      }

      return sortDirection === "asc" ? result : -result;
    });

  return children.map((entry) => ({
    entry,
    depth: 0,
    isExpanded: false,
    isLoading: false,
  }));
}

export function useFilesPaneState(active: boolean, target: DeveloperTarget) {
  const [childrenByPath, setChildrenByPath] = useState<Map<string, RemoteFileEntry[]>>(
    new Map(),
  );
  const [parentByPath, setParentByPath] = useState<Map<string, string | null>>(
    new Map(),
  );
  const [expandedPaths, setExpandedPaths] = useState<Set<string>>(new Set());
  const [loadingPaths, setLoadingPaths] = useState<Set<string>>(new Set());
  const [rootPath, setRootPath] = useState<string | null>(null);
  const [rootParentPath, setRootParentPath] = useState<string | null>(null);
  const [currentDirectoryPath, setCurrentDirectoryPath] = useState<string | null>(null);
  const [preview, setPreview] = useState<RemoteFilePreview | null>(null);
  const [selectedPath, setSelectedPath] = useState<string | null>(null);
  const [isLoadingPreview, setIsLoadingPreview] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [rootOptions, setRootOptions] = useState<FilesRootOption[]>([]);
  const [isLoadingRoots, setIsLoadingRoots] = useState(false);
  const [browserView, setBrowserView] = useState<FilesBrowserView>("icons");
  const [showHiddenDirectories, setShowHiddenDirectories] = useState(false);
  const [sortKey, setSortKey] = useState<FilesSortKey>("name");
  const [sortDirection, setSortDirection] = useState<FilesSortDirection>("asc");

  const resetBrowser = () => {
    setChildrenByPath(new Map());
    setParentByPath(new Map());
    setExpandedPaths(new Set());
    setRootPath(null);
    setRootParentPath(null);
    setCurrentDirectoryPath(null);
    setPreview(null);
    setSelectedPath(null);
    setError(null);
  };

  const loadDirectory = (dirPath: string | null | undefined, asRoot = false) => {
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
        setParentByPath((prev) => {
          const next = new Map(prev);
          next.set(listing.path, listing.parentPath ?? null);
          return next;
        });

        if (asRoot || !rootPath || dirPath === null || dirPath === undefined) {
          setRootPath(listing.path);
          setRootParentPath(listing.parentPath ?? null);
          setCurrentDirectoryPath(listing.path);
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

  const initializeRootsFromEffect = useEffectEvent(() => {
    setIsLoadingRoots(true);
    setError(null);
    const preferredFilesRoot = targetFilesRoot(target);

    void Promise.all([
      listRemoteDirectory(target.id, "~"),
      listRemoteDirectory(target.id, "~/repos"),
    ])
      .then(([homeListing, reposListing]) => {
        // If ~/repos fell back to home dir (path is same), use home subdirs directly
        const reposIsSeparate = reposListing.path !== homeListing.path;
        const repoDirectories = reposListing.entries.filter(
          (entry) =>
            entry.kind === "directory" && isVisibleEntry(entry, showHiddenDirectories),
        );
        const homeDirectories = homeListing.entries.filter(
          (entry) =>
            entry.kind === "directory" && isVisibleEntry(entry, showHiddenDirectories),
        );

        const nextRootOptions: FilesRootOption[] = [];

        if (reposIsSeparate) {
          // Remote target: prefer ~/repos subdirs
          const preferredRepo =
            repoDirectories.find((entry) => entry.path === preferredFilesRoot) ?? repoDirectories[0];
          if (preferredRepo) {
            nextRootOptions.push({ label: `~/repos/${preferredRepo.name}`, path: preferredRepo.path });
          }
          for (const entry of repoDirectories) {
            if (entry.path === preferredRepo?.path) continue;
            nextRootOptions.push({ label: `~/repos/${entry.name}`, path: entry.path });
          }
          nextRootOptions.push({ label: "~", path: homeListing.path });
          loadDirectory(preferredRepo?.path ?? preferredFilesRoot ?? homeListing.path);
        } else {
          // Localhost: show home directory subdirs with their actual names
          nextRootOptions.push({ label: homeListing.path, path: homeListing.path });
          for (const entry of homeDirectories) {
            nextRootOptions.push({ label: entry.name, path: entry.path });
          }
          loadDirectory(homeListing.path);
        }

        setRootOptions(nextRootOptions);
      })
      .catch((loadError) => {
        setError(
          loadError instanceof Error
            ? loadError.message
            : "Failed to load remote files.",
        );
        loadDirectoryFromEffect("~");
      })
      .finally(() => {
        setIsLoadingRoots(false);
      });
  });

  useEffect(() => {
    if (!active || rootPath) return;

    initializeRootsFromEffect();
  }, [active, rootPath]);

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

  const enterDirectory = (entry: RemoteFileEntry) => {
    if (entry.kind !== "directory") {
      return;
    }

    setSelectedPath(entry.path);
    setPreview(null);
    setIsLoadingPreview(false);
    setCurrentDirectoryPath(entry.path);
    if (!childrenByPath.has(entry.path)) {
      loadDirectory(entry.path);
    }
  };

  const selectEntry = (entry: RemoteFileEntry) => {
    setSelectedPath(entry.path);
    if (entry.kind !== "file" && entry.kind !== "symlink") {
      setPreview(null);
      setIsLoadingPreview(false);
    }
  };

  const openFile = (entry: RemoteFileEntry) => {
    if (!isPreviewableEntry(entry)) {
      selectEntry(entry);
      return;
    }

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

  const closePreview = () => {
    setPreview(null);
    setIsLoadingPreview(false);
  };

  const navigateUp = () => {
    const parentPath =
      (currentDirectoryPath ? parentByPath.get(currentDirectoryPath) : null) ?? null;

    if (browserView === "tree") {
      if (!rootParentPath) return;
      resetBrowser();
      loadDirectory(rootParentPath);
      return;
    }

    if (!currentDirectoryPath || !parentPath) {
      return;
    }

    setCurrentDirectoryPath(parentPath);
    setPreview(null);
    setIsLoadingPreview(false);
    setSelectedPath(parentPath);
    if (!childrenByPath.has(parentPath)) {
      loadDirectory(parentPath);
    }
  };

  const selectRoot = (path: string) => {
    // Clear browser state WITHOUT nulling rootPath — nulling rootPath triggers
    // the init effect which would navigate back to the home directory.
    setChildrenByPath(new Map());
    setParentByPath(new Map());
    setExpandedPaths(new Set());
    setCurrentDirectoryPath(null);
    setPreview(null);
    setSelectedPath(null);
    setError(null);
    loadDirectory(path, true);
  };

  const refresh = () => {
    const nextPath = rootPath;
    resetBrowser();
    loadDirectory(nextPath);
  };

  const toggleSort = (nextSortKey: FilesSortKey) => {
    if (sortKey === nextSortKey) {
      setSortDirection((current) => (current === "asc" ? "desc" : "asc"));
      return;
    }

    setSortKey(nextSortKey);
    setSortDirection(nextSortKey === "size" ? "desc" : "asc");
  };

  const flatTree = useMemo(() => {
    if (browserView === "tree") {
      return buildFlatTree(
        rootPath,
        childrenByPath,
        expandedPaths,
        loadingPaths,
        showHiddenDirectories,
      );
    }

    return buildDirectoryEntries(
      currentDirectoryPath ?? rootPath,
      childrenByPath,
      showHiddenDirectories,
      sortKey,
      sortDirection,
    );
  }, [
    browserView,
    childrenByPath,
    currentDirectoryPath,
    expandedPaths,
    loadingPaths,
    rootPath,
    showHiddenDirectories,
    sortDirection,
    sortKey,
  ]);

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
    currentDirectoryPath,
    editorLanguage,
    enterDirectory,
    error,
    flatTree,
    browserView,
    closePreview,
    isLoadingPreview,
    isLoadingRoots,
    loadingPaths,
    navigateUp,
    openFile,
    preview,
    refresh,
    rootOptions,
    rootParentPath,
    rootPath,
    setShowHiddenDirectories,
    sortDirection,
    sortKey,
    selectEntry,
    selectedEntry,
    selectedPath,
    selectRoot,
    setBrowserView,
    showHiddenDirectories,
    toggleSort,
    toggleDirectory,
  };
}
