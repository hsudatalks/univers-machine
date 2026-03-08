import type { DeveloperTarget } from "../types";
import { FilesEditor } from "./FilesEditor";
import { FilesTree } from "./FilesTree";
import { useFilesPaneState } from "../hooks/useFilesPaneState";
import { Button } from "./ui/button";

interface FilesPaneProps {
  active: boolean;
  target: DeveloperTarget;
}

export function FilesPane({ active, target }: FilesPaneProps) {
  const {
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
  } = useFilesPaneState(active, target);

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
          <Button
            disabled={!rootParentPath || loadingPaths.size > 0}
            onClick={navigateUp}
            size="sm"
            variant="outline"
          >
            Up
          </Button>
          <Button
            disabled={loadingPaths.size > 0}
            onClick={refresh}
            size="sm"
            variant="outline"
          >
            Refresh
          </Button>
        </div>
      </header>

      <div className="files-panel-body">
        <FilesTree
          error={error && !preview ? error : null}
          flatTree={flatTree}
          isLoading={!rootPath && loadingPaths.size > 0}
          onOpenFile={openFile}
          onToggleDirectory={toggleDirectory}
          rootPath={rootPath}
          selectedPath={selectedPath}
        />

        <FilesEditor
          editorLanguage={editorLanguage}
          isLoadingPreview={isLoadingPreview}
          preview={preview}
          selectedEntry={selectedEntry}
        />
      </div>
    </article>
  );
}
