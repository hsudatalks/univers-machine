import Editor, { type Monaco } from "@monaco-editor/react";
import type { RemoteFileEntry, RemoteFilePreview } from "../types";

type FilesEditorProps = {
  editorLanguage: string;
  isLoadingPreview: boolean;
  preview: RemoteFilePreview | null;
  selectedEntry: RemoteFileEntry | undefined;
};

function configureMonacoTheme(monaco: Monaco) {
  monaco.editor.defineTheme("universWorkbenchDark", {
    base: "vs-dark",
    inherit: true,
    rules: [
      { token: "comment", foreground: "8b949e" },
      { token: "keyword", foreground: "ff7b72" },
      { token: "string", foreground: "a5d6ff" },
      { token: "number", foreground: "79c0ff" },
      { token: "type", foreground: "ffa657" },
      { token: "function", foreground: "d2a8ff" },
    ],
    colors: {
      "editor.background": "#0d1117",
      "editor.foreground": "#e6edf3",
      "editorCursor.foreground": "#58a6ff",
      "editorGutter.background": "#0d1117",
      "editorIndentGuide.activeBackground1": "#6e7681",
      "editorIndentGuide.background1": "#30363d",
      "editorLineNumber.activeForeground": "#e6edf3",
      "editorLineNumber.foreground": "#6e7681",
      "editor.lineHighlightBackground": "#161b22",
      "editor.selectionBackground": "#264f78",
      "editor.selectionHighlightBackground": "#1f6feb33",
      "editor.inactiveSelectionBackground": "#1f293733",
      "editorWhitespace.foreground": "#30363d",
      "minimap.selectionHighlight": "#264f78",
      "scrollbar.shadow": "#00000000",
    },
  });
}

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
                <span className="files-editor-chip">Preview truncated</span>
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
                beforeMount={configureMonacoTheme}
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
                theme="universWorkbenchDark"
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
