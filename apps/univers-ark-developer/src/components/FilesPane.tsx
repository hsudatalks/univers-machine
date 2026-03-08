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
    editorLanguage,
    error,
    flatTree,
    isLoadingPreview,
    isLoadingRoots,
    loadingPaths,
    openFile,
    preview,
    refresh,
    rootOptions,
    rootPath,
    selectedEntry,
    selectedPath,
    selectRoot,
    setBrowserView,
    toggleDirectory,
  } = useFilesPaneState(active, target);
  const isPreviewVisible = Boolean(isLoadingPreview || preview);

  return (
    <article className="panel tool-panel files-panel">
      <header className="panel-header tool-panel-header">
        <div className="files-toolbar">
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
        </div>

        <div className="tool-panel-actions">
          <Tabs
            onValueChange={(value) => {
              setBrowserView(value as "list" | "details");
            }}
            value={browserView}
          >
            <TabsList className="files-view-tabs">
              <TabsTrigger className="files-view-trigger" value="list">
                List
              </TabsTrigger>
              <TabsTrigger className="files-view-trigger" value="details">
                Details
              </TabsTrigger>
            </TabsList>
          </Tabs>
          <Button
            disabled={loadingPaths.size > 0}
            onClick={refresh}
            size="sm"
            variant="ghost"
          >
            Refresh
          </Button>
        </div>
      </header>

      <div
        className={`files-panel-body ${isPreviewVisible ? "is-preview-visible" : ""}`}
      >
        <FilesTree
          browserView={browserView}
          error={error && !preview ? error : null}
          flatTree={flatTree}
          isLoading={(!rootPath && loadingPaths.size > 0) || isLoadingRoots}
          onOpenFile={openFile}
          onToggleDirectory={toggleDirectory}
          rootPath={rootPath}
          selectedPath={selectedPath}
        />

        {isPreviewVisible ? (
          <FilesEditor
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
