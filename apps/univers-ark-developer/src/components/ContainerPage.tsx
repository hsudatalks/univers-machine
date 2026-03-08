import type { CSSProperties, PointerEvent as ReactPointerEvent } from "react";
import { BrowserPane, type BrowserFrameInstance } from "./BrowserPane";
import { FilesPane } from "./FilesPane";
import { TerminalPane } from "./TerminalPane";
import type { ContainerToolPanel } from "../lib/view-types";
import type { DeveloperSurface, DeveloperTarget } from "../types";

interface ContainerPageProps {
  activeTool: ContainerToolPanel;
  browserFrame?: BrowserFrameInstance;
  browserSurface?: DeveloperSurface;
  developmentPanel: ContainerToolPanel | null;
  developmentSurface?: DeveloperSurface;
  onReloadBrowser: () => void;
  onRestartBrowser: () => void;
  onSelectTool: (panel: ContainerToolPanel) => void;
  onStartResize: (event: ReactPointerEvent<HTMLDivElement>) => void;
  pageVisible: boolean;
  previewPanel: ContainerToolPanel | null;
  previewSurface?: DeveloperSurface;
  target: DeveloperTarget;
  workspaceStyle: CSSProperties;
}

function isBrowserToolPanel(
  panel: ContainerToolPanel | null | undefined,
): panel is `browser:${string}` {
  return Boolean(panel?.startsWith("browser:"));
}

export function ContainerPage({
  activeTool,
  browserFrame,
  browserSurface,
  developmentPanel,
  developmentSurface,
  onReloadBrowser,
  onRestartBrowser,
  onSelectTool,
  onStartResize,
  pageVisible,
  previewPanel,
  previewSurface,
  target,
  workspaceStyle,
}: ContainerPageProps) {
  return (
    <>
      <header className="content-header content-header-container">
        <div className="content-header-copy">
          <h1 className="content-title content-title-container">{target.label}</h1>
        </div>

        <div className="content-header-tools content-header-tools-container">
          <button
            className={`panel-button panel-button-toolbar ${activeTool === "files" ? "is-active" : ""}`}
            onClick={() => {
              onSelectTool("files");
            }}
            type="button"
          >
            Files
          </button>
          <button
            className={`panel-button panel-button-toolbar ${activeTool === developmentPanel ? "is-active" : ""}`}
            disabled={!developmentSurface}
            onClick={() => {
              if (developmentPanel) {
                onSelectTool(developmentPanel);
              }
            }}
            type="button"
          >
            Dev
          </button>
          <button
            className={`panel-button panel-button-toolbar ${activeTool === previewPanel ? "is-active" : ""}`}
            disabled={!previewSurface}
            onClick={() => {
              if (previewPanel) {
                onSelectTool(previewPanel);
              }
            }}
            type="button"
          >
            Pre
          </button>
        </div>
      </header>

      <section className="page-section">
        <div
          className={`container-workspace ${isBrowserToolPanel(activeTool) ? "tool-browser" : "tool-files"}`}
          style={workspaceStyle}
        >
          <article className="panel terminal-panel">
            <TerminalPane active={pageVisible} target={target} />
          </article>

          <div
            aria-label="Resize terminal and tool panels"
            aria-orientation="vertical"
            className="container-resizer"
            onPointerDown={onStartResize}
            role="separator"
          />

          {activeTool === "files" ? (
            <FilesPane active={pageVisible} target={target} />
          ) : null}

          {isBrowserToolPanel(activeTool) && pageVisible ? (
            <BrowserPane
              activeFrame={browserFrame}
              onReload={onReloadBrowser}
              onRestart={onRestartBrowser}
              retainedFrames={browserFrame ? [browserFrame] : []}
              slotLabel={browserSurface?.label ?? "Browser"}
            />
          ) : null}
        </div>
      </section>
    </>
  );
}
