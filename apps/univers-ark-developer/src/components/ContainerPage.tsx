import { useState, type CSSProperties, type PointerEvent as ReactPointerEvent } from "react";
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
  isTerminalCollapsed: boolean;
  onReloadBrowser: () => void;
  onRestartBrowser: () => void;
  onRestartContainer?: () => Promise<void>;
  onSelectTool: (panel: ContainerToolPanel) => void;
  onStartResize: (event: ReactPointerEvent<HTMLDivElement>) => void;
  onToggleTerminalCollapsed: () => void;
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
  isTerminalCollapsed,
  onReloadBrowser,
  onRestartBrowser,
  onRestartContainer,
  onSelectTool,
  onStartResize,
  onToggleTerminalCollapsed,
  pageVisible,
  previewPanel,
  previewSurface,
  target,
  workspaceStyle,
}: ContainerPageProps) {
  const [isRestarting, setIsRestarting] = useState(false);
  return (
    <>
      <header className="content-header content-header-container">
        <div className="content-header-copy">
          <div className="content-title-row">
            <button
              aria-label={
                isTerminalCollapsed ? "Show terminal pane" : "Hide terminal pane"
              }
              className="panel-button panel-button-icon panel-button-toolbar content-title-toggle"
              onClick={onToggleTerminalCollapsed}
              title={
                isTerminalCollapsed ? "Show terminal pane" : "Hide terminal pane"
              }
              type="button"
            >
              <svg
                aria-hidden="true"
                className="panel-button-icon-svg"
                fill="none"
                viewBox="0 0 16 16"
              >
                {isTerminalCollapsed ? (
                  <>
                    <path
                      d="M2.75 3.25h10.5v9.5H2.75z"
                      stroke="currentColor"
                      strokeWidth="1.25"
                    />
                    <path
                      d="M5.75 4.25v7.5"
                      stroke="currentColor"
                      strokeWidth="1.25"
                    />
                  </>
                ) : (
                  <>
                    <path
                      d="M2.75 3.25h10.5v9.5H2.75z"
                      stroke="currentColor"
                      strokeWidth="1.25"
                    />
                    <path
                      d="M5.75 4.25v7.5"
                      stroke="currentColor"
                      strokeWidth="1.25"
                    />
                    <path
                      d="M4.5 8 3.25 8"
                      stroke="currentColor"
                      strokeLinecap="round"
                      strokeWidth="1.25"
                    />
                  </>
                )}
              </svg>
            </button>
            <h1 className="content-title content-title-container">{target.label}</h1>
          </div>
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

          {onRestartContainer ? (
            <button
              className="panel-button panel-button-toolbar panel-button-icon"
              disabled={isRestarting}
              onClick={() => {
                setIsRestarting(true);
                onRestartContainer()
                  .catch(() => {})
                  .finally(() => {
                    setIsRestarting(false);
                  });
              }}
              title={isRestarting ? "Restarting container…" : "Restart container"}
              type="button"
            >
              <svg
                aria-hidden="true"
                className={`panel-button-icon-svg ${isRestarting ? "is-spinning" : ""}`}
                fill="none"
                viewBox="0 0 16 16"
              >
                <path
                  d="M13.25 8A5.25 5.25 0 1 1 11.7 4.29"
                  stroke="currentColor"
                  strokeLinecap="round"
                  strokeLinejoin="round"
                  strokeWidth="1.25"
                />
                <path
                  d="M10.75 2.75h2.5v2.5"
                  stroke="currentColor"
                  strokeLinecap="round"
                  strokeLinejoin="round"
                  strokeWidth="1.25"
                />
              </svg>
            </button>
          ) : null}
        </div>
      </header>

      <section className="page-section">
        <div
          className={`container-workspace ${isBrowserToolPanel(activeTool) ? "tool-browser" : "tool-files"} ${isTerminalCollapsed ? "is-terminal-collapsed" : ""}`}
          style={workspaceStyle}
        >
          <article className={`panel terminal-panel ${isTerminalCollapsed ? "is-collapsed" : ""}`}>
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

          {isBrowserToolPanel(activeTool) ? (
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
