import { useState, type CSSProperties, type PointerEvent as ReactPointerEvent } from "react";
import { BrowserPane, type BrowserFrameInstance } from "./BrowserPane";
import { FilesPane } from "./FilesPane";
import { TerminalPane } from "./TerminalPane";
import { Button } from "./ui/button";
import type { ContainerToolPanel } from "../lib/view-types";
import type { DeveloperSurface, DeveloperTarget } from "../types";

interface ContainerPageProps {
  activeTool: ContainerToolPanel;
  developmentPanel: ContainerToolPanel | null;
  developmentBrowserFrame?: BrowserFrameInstance;
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
  previewBrowserFrame?: BrowserFrameInstance;
  previewSurface?: DeveloperSurface;
  target: DeveloperTarget;
  workspaceStyle: CSSProperties;
}

export function ContainerPage({
  activeTool,
  developmentPanel,
  developmentBrowserFrame,
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
  previewBrowserFrame,
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
            <Button
              aria-label={
                isTerminalCollapsed ? "Show terminal pane" : "Hide terminal pane"
              }
              className="content-title-toggle"
              onClick={onToggleTerminalCollapsed}
              size="icon"
              title={
                isTerminalCollapsed ? "Show terminal pane" : "Hide terminal pane"
              }
              variant="ghost"
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
            </Button>
            <h1 className="content-title content-title-container">{target.label}</h1>
          </div>
        </div>

        <div className="content-header-tools content-header-tools-container">
          <Button
            isActive={activeTool === "files"}
            onClick={() => {
              onSelectTool("files");
            }}
            size="sm"
            variant={activeTool === "files" ? "default" : "outline"}
          >
            Files
          </Button>
          <Button
            disabled={!developmentSurface}
            isActive={activeTool === developmentPanel}
            onClick={() => {
              if (developmentPanel) {
                onSelectTool(developmentPanel);
              }
            }}
            size="sm"
            variant={activeTool === developmentPanel ? "default" : "outline"}
          >
            Dev
          </Button>
          <Button
            disabled={!previewSurface}
            isActive={activeTool === previewPanel}
            onClick={() => {
              if (previewPanel) {
                onSelectTool(previewPanel);
              }
            }}
            size="sm"
            variant={activeTool === previewPanel ? "default" : "outline"}
          >
            Pre
          </Button>

          {onRestartContainer ? (
            <Button
              disabled={isRestarting}
              onClick={() => {
                setIsRestarting(true);
                onRestartContainer()
                  .catch(() => {})
                  .finally(() => {
                    setIsRestarting(false);
                  });
              }}
              size="icon"
              title={isRestarting ? "Restarting container…" : "Restart container"}
              variant="ghost"
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
            </Button>
          ) : null}
        </div>
      </header>

      <section className="page-section">
        <div
          className={`container-workspace ${activeTool === "files" ? "tool-files" : "tool-browser"} ${isTerminalCollapsed ? "is-terminal-collapsed" : ""}`}
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

          <div className={activeTool === "files" ? "" : "is-hidden"}>
            <FilesPane active={pageVisible} target={target} />
          </div>

          {developmentSurface ? (
            <BrowserPane
              activeFrame={developmentBrowserFrame}
              isVisible={activeTool === developmentPanel}
              onReload={onReloadBrowser}
              onRestart={onRestartBrowser}
              retainedFrames={developmentBrowserFrame ? [developmentBrowserFrame] : []}
              slotLabel={developmentSurface.label}
            />
          ) : null}

          {previewSurface ? (
            <BrowserPane
              activeFrame={previewBrowserFrame}
              isVisible={activeTool === previewPanel}
              onReload={onReloadBrowser}
              onRestart={onRestartBrowser}
              retainedFrames={previewBrowserFrame ? [previewBrowserFrame] : []}
              slotLabel={previewSurface.label}
            />
          ) : null}
        </div>
      </section>
    </>
  );
}
