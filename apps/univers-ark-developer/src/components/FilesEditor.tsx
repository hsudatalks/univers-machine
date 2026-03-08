import Editor from "@monaco-editor/react";
import type { RemoteFileEntry, RemoteFilePreview } from "../types";

type FilesEditorProps = {
  editorLanguage: string;
  isLoadingPreview: boolean;
  preview: RemoteFilePreview | null;
  selectedEntry: RemoteFileEntry | undefined;
};

export function FilesEditor({
  editorLanguage,
  isLoadingPreview,
  preview,
  selectedEntry,
}: FilesEditorProps) {
  return (
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
            <div className="files-editor-monaco">
              <Editor
                height="100%"
                language={editorLanguage}
                options={{
                  contextmenu: false,
                  domReadOnly: true,
                  folding: true,
                  fontFamily: "Iosevka, SFMono-Regular, Consolas, monospace",
                  fontSize: 12,
                  hideCursorInOverviewRuler: true,
                  lineHeight: 18,
                  minimap: { enabled: true },
                  overviewRulerBorder: false,
                  overviewRulerLanes: 0,
                  readOnly: true,
                  renderLineHighlight: "none",
                  scrollBeyondLastLine: false,
                  scrollbar: {
                    verticalScrollbarSize: 8,
                    horizontalScrollbarSize: 8,
                  },
                  wordWrap: "off",
                }}
                theme="vs-dark"
                value={preview.content}
              />
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
  );
}
