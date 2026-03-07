import { useEffect, useEffectEvent, useMemo, useState } from "react";
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

function entryTone(entry: RemoteFileEntry): string {
  switch (entry.kind) {
    case "directory":
      return "is-directory";
    case "symlink":
      return "is-symlink";
    default:
      return "is-file";
  }
}

function entryLabel(entry: RemoteFileEntry): string {
  if (entry.kind === "directory") {
    return `${entry.name}/`;
  }

  return entry.name;
}

function formatFileSize(size: number): string {
  if (size < 1024) {
    return `${size} B`;
  }

  if (size < 1024 * 1024) {
    return `${(size / 1024).toFixed(1)} KB`;
  }

  return `${(size / (1024 * 1024)).toFixed(1)} MB`;
}

export function FilesPane({ active, target }: FilesPaneProps) {
  const [listing, setListing] = useState<RemoteDirectoryListing | null>(null);
  const [preview, setPreview] = useState<RemoteFilePreview | null>(null);
  const [selectedPath, setSelectedPath] = useState<string | null>(null);
  const [isLoadingListing, setIsLoadingListing] = useState(false);
  const [isLoadingPreview, setIsLoadingPreview] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const selectedEntry = useMemo(
    () => listing?.entries.find((entry) => entry.path === selectedPath),
    [listing, selectedPath],
  );

  const loadDirectory = (nextPath?: string | null) => {
    setIsLoadingListing(true);
    setError(null);

    void listRemoteDirectory(target.id, nextPath)
      .then((nextListing) => {
        setListing(nextListing);
        setPreview(null);
        setSelectedPath(null);
      })
      .catch((loadError) => {
        setError(
          loadError instanceof Error
            ? loadError.message
            : "Failed to load remote files.",
        );
      })
      .finally(() => {
        setIsLoadingListing(false);
      });
  };

  const loadDirectoryFromEffect = useEffectEvent((nextPath?: string | null) => {
    loadDirectory(nextPath);
  });

  useEffect(() => {
    if (!active || listing) {
      return;
    }

    loadDirectoryFromEffect();
  }, [active, listing, target.id]);

  const openEntry = (entry: RemoteFileEntry) => {
    if (entry.kind === "directory") {
      loadDirectory(entry.path);
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

  return (
    <article className="panel tool-panel files-panel">
      <header className="panel-header tool-panel-header">
        <div className="tool-panel-heading">
          <span className="panel-title">Files</span>
          <code className="tool-panel-path">
            {listing?.path ?? "Loading remote workspace"}
          </code>
        </div>

        <div className="tool-panel-actions">
          <button
            className="panel-button panel-button-toolbar"
            disabled={!listing?.parentPath || isLoadingListing}
            onClick={() => {
              loadDirectory(listing?.parentPath ?? null);
            }}
            type="button"
          >
            Up
          </button>
          <button
            className="panel-button panel-button-toolbar"
            disabled={isLoadingListing}
            onClick={() => {
              loadDirectory(listing?.path ?? null);
            }}
            type="button"
          >
            Refresh
          </button>
        </div>
      </header>

      <div className="files-panel-body">
        <section className="files-list">
          {error ? (
            <div className="files-empty-state">
              <p className="files-empty-title">Unavailable</p>
              <p className="files-empty-copy">{error}</p>
            </div>
          ) : isLoadingListing && !listing ? (
            <div className="files-empty-state">
              <p className="files-empty-title">Loading</p>
              <p className="files-empty-copy">Fetching remote directory listing.</p>
            </div>
          ) : listing && listing.entries.length > 0 ? (
            <div className="files-entry-list">
              {listing.entries.map((entry) => (
                <button
                  className={`files-entry ${entryTone(entry)} ${selectedPath === entry.path ? "is-selected" : ""}`}
                  key={entry.path}
                  onClick={() => {
                    openEntry(entry);
                  }}
                  type="button"
                >
                  <span className="files-entry-name">{entryLabel(entry)}</span>
                  <span className="files-entry-meta">
                    {entry.kind === "file" ? formatFileSize(entry.size) : entry.kind}
                  </span>
                </button>
              ))}
            </div>
          ) : (
            <div className="files-empty-state">
              <p className="files-empty-title">Empty</p>
              <p className="files-empty-copy">
                No files were found in this directory.
              </p>
            </div>
          )}
        </section>

        <section className="files-preview">
          {isLoadingPreview ? (
            <div className="files-empty-state">
              <p className="files-empty-title">Loading</p>
              <p className="files-empty-copy">Rendering file preview.</p>
            </div>
          ) : preview ? (
            <>
              <div className="files-preview-header">
                <span className="files-preview-name">
                  {selectedEntry?.name ?? preview.path}
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
                <pre className="files-preview-code">{preview.content}</pre>
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
