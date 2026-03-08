import { ArrowUp, Eye, EyeOff, RefreshCw } from "lucide-react";
import type { DeveloperTarget } from "../types";
import { FilesEditor } from "./FilesEditor";
import { FilesTree } from "./FilesTree";
import { useFilesPaneState } from "../hooks/useFilesPaneState";
import { Button } from "./ui/button";
import { Tabs, TabsList, TabsTrigger } from "./ui/tabs";

interface FilesPaneProps {
  active: boolean;
  target: DeveloperTarget;
}

export function FilesPane({ active, target }: FilesPaneProps) {
  const {
    browserView,
    closePreview,
    currentDirectoryPath,
    editorLanguage,
    enterDirectory,
    error,
    flatTree,
    isLoadingPreview,
    isLoadingRoots,
    loadingPaths,
    navigateUp,
    openFile,
    preview,
    refresh,
    rootOptions,
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
  } = useFilesPaneState(active, target);
  const isPreviewVisible = Boolean(isLoadingPreview || preview);
  const normalizedBrowserView = browserView === "list" ? "details" : browserView;
  const effectiveBrowserView = isPreviewVisible ? "tree" : normalizedBrowserView;
  const isLoadingTree =
    (!rootPath && loadingPaths.size > 0) ||
    isLoadingRoots ||
    (effectiveBrowserView !== "tree" &&
      !!currentDirectoryPath &&
      loadingPaths.has(currentDirectoryPath));

  return (
    <article className="panel tool-panel files-panel">
      <header className="panel-header tool-panel-header">
        <div className="files-toolbar">
          <Button
            disabled={
              browserView === "tree" ||
              !currentDirectoryPath ||
              currentDirectoryPath === rootPath ||
              loadingPaths.size > 0
            }
            onClick={navigateUp}
            size="sm"
            title="Up"
            variant="ghost"
          >
            <ArrowUp aria-hidden="true" size={14} />
          </Button>
          <label className="files-root-picker">
            <span className="sr-only">Choose root folder</span>
            <select
              className="files-root-select"
              disabled={isLoadingRoots || loadingPaths.size > 0}
              onChange={(event) => {
                selectRoot(event.target.value);
              }}
              value={rootPath ?? ""}
            >
              {!rootPath ? (
                <option value="">
                  {isLoadingRoots ? "Loading folders…" : "Choose folder"}
                </option>
              ) : null}
              {rootOptions.map((option) => (
                <option key={option.path} value={option.path}>
                  {option.label}
                </option>
              ))}
            </select>
          </label>
          {browserView !== "tree" && currentDirectoryPath ? (
            <div className="files-current-directory" title={currentDirectoryPath}>
              {currentDirectoryPath}
            </div>
          ) : null}
        </div>

        <div className="tool-panel-actions">
          <Tabs
            onValueChange={(value) => {
              setBrowserView(value as "tree" | "list" | "icons" | "details");
            }}
            value={browserView}
          >
            <TabsList className="files-view-tabs">
              <TabsTrigger
                className="files-view-trigger"
                disabled={isPreviewVisible}
                value="icons"
              >
                Icons
              </TabsTrigger>
              <TabsTrigger
                className="files-view-trigger"
                disabled={isPreviewVisible}
                value="details"
              >
                Details
              </TabsTrigger>
            </TabsList>
          </Tabs>
          <Button
            onClick={() => {
              setShowHiddenDirectories((current) => !current);
            }}
            size="sm"
            title={
              showHiddenDirectories
                ? "Hide hidden folders"
                : "Show hidden folders"
            }
            variant="ghost"
          >
            {showHiddenDirectories ? (
              <EyeOff aria-hidden="true" size={14} />
            ) : (
              <Eye aria-hidden="true" size={14} />
            )}
          </Button>
          <Button
            disabled={loadingPaths.size > 0}
            onClick={refresh}
            size="sm"
            title="Refresh"
            variant="ghost"
          >
            <RefreshCw aria-hidden="true" size={14} />
          </Button>
        </div>
      </header>

      <div
        className={`files-panel-body ${isPreviewVisible ? "is-preview-visible" : ""}`}
      >
        <FilesTree
          browserView={effectiveBrowserView}
          error={error && !preview ? error : null}
          flatTree={flatTree}
          isLoading={isLoadingTree}
          onEnterDirectory={enterDirectory}
          onOpenFile={openFile}
          onSelectEntry={selectEntry}
          onSortBy={toggleSort}
          onToggleDirectory={toggleDirectory}
          selectedPath={selectedPath}
          sortDirection={sortDirection}
          sortKey={sortKey}
        />

        {isPreviewVisible ? (
          <FilesEditor
            onClosePreview={closePreview}
            editorLanguage={editorLanguage}
            isLoadingPreview={isLoadingPreview}
            preview={preview}
            selectedEntry={selectedEntry}
          />
        ) : null}
      </div>
    </article>
  );
}
