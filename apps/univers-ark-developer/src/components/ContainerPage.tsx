import { useState, type CSSProperties, type PointerEvent as ReactPointerEvent } from "react";
import { BrowserPane, type BrowserFrameInstance } from "./BrowserPane";
import { DashboardPane } from "./DashboardPane";
import { FilesPane } from "./FilesPane";
import { TerminalPane } from "./TerminalPane";
import { Button } from "./ui/button";
import type { ContainerToolPanel } from "../lib/view-types";
import type { DeveloperSurface, DeveloperTarget, TunnelStatus } from "../types";
import { FolderOpen, Globe, LayoutDashboard } from "lucide-react";

interface ContainerPageProps {
  activeTool: ContainerToolPanel;
  dashboardRefreshSeconds: number;
  developmentPanel: ContainerToolPanel | null;
  developmentBrowserFrame?: BrowserFrameInstance;
  developmentSurface?: DeveloperSurface;
  developmentStatus?: TunnelStatus;
  isTerminalCollapsed: boolean;
  onReloadBrowser: () => void;
  onRestartBrowser: () => void;
  onRestartContainer?: () => Promise<void>;
  onSelectTool: (panel: ContainerToolPanel) => void;
  onStartResize: (event: ReactPointerEvent<HTMLDivElement>) => void;
  onToggleTerminalCollapsed: () => void;
  pageVisible: boolean;
  target: DeveloperTarget;
  workspaceStyle: CSSProperties;
}

export function ContainerPage({
  activeTool,
  dashboardRefreshSeconds,
  developmentPanel,
  developmentBrowserFrame,
  developmentSurface,
  developmentStatus,
  isTerminalCollapsed,
  onReloadBrowser,
  onRestartBrowser,
  onRestartContainer,
  onSelectTool,
  onStartResize,
  onToggleTerminalCollapsed,
  pageVisible,
  target,
  workspaceStyle,
}: ContainerPageProps) {
  const [isRestarting, setIsRestarting] = useState(false);
  return (
    <>
      <header className="content-header content-header-container">
        <div className="content-header-leading">
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
              <path
                d="M2.75 3.25h10.5v9.5H2.75z"
                stroke="currentColor"
                strokeWidth="1.25"
              />
              <path
                d="M4.5 6.1 6.55 8 4.5 9.9"
                stroke="currentColor"
                strokeLinecap="round"
                strokeLinejoin="round"
                strokeWidth="1.25"
              />
              <path
                d="M7.85 10.1h3.2"
                stroke="currentColor"
                strokeLinecap="round"
                strokeWidth="1.25"
              />
              {isTerminalCollapsed ? (
                <path
                  d="M3.35 3.35 12.65 12.65"
                  stroke="currentColor"
                  strokeLinecap="round"
                  strokeWidth="1.1"
                />
              ) : null}
            </svg>
          </Button>
        </div>

        <div className="content-header-copy">
          <div className="content-title-row">
            <h1 className="content-title content-title-container">{target.label}</h1>
          </div>
        </div>

        <div className="content-header-tools content-header-tools-container">
          <Button
            aria-label="Dashboard"
            isActive={activeTool === "dashboard"}
            onClick={() => {
              onSelectTool("dashboard");
            }}
            size="icon"
            title="Dashboard"
            variant={activeTool === "dashboard" ? "default" : "ghost"}
          >
            <LayoutDashboard size={16} />
          </Button>
          <Button
            aria-label="Files"
            isActive={activeTool === "files"}
            onClick={() => {
              onSelectTool("files");
            }}
            size="icon"
            title="Files"
            variant={activeTool === "files" ? "default" : "ghost"}
          >
            <FolderOpen size={16} />
          </Button>
          <Button
            aria-label="Development browser"
            disabled={!developmentSurface}
            isActive={activeTool === developmentPanel}
            onClick={() => {
              if (developmentPanel) {
                onSelectTool(developmentPanel);
              }
            }}
            size="icon"
            title="Development browser"
            variant={activeTool === developmentPanel ? "default" : "ghost"}
          >
            <Globe size={16} />
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
          className={`container-workspace ${activeTool === "dashboard" ? "tool-dashboard" : activeTool === "files" ? "tool-files" : "tool-browser"} ${isTerminalCollapsed ? "is-terminal-collapsed" : ""}`}
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

          <div className={`dashboard-pane-slot ${activeTool === "dashboard" ? "" : "is-hidden"}`}>
            <DashboardPane
              key={`${target.id}:${dashboardRefreshSeconds}`}
              dashboardRefreshSeconds={dashboardRefreshSeconds}
              developmentStatus={developmentStatus}
              developmentSurfaceLocalUrl={developmentStatus?.localUrl ?? developmentSurface?.localUrl}
              onOpenDev={() => {
                if (developmentPanel) {
                  onSelectTool(developmentPanel);
                }
              }}
              onOpenFiles={() => {
                onSelectTool("files");
              }}
              target={target}
            />
          </div>

          <div className={`files-pane-slot ${activeTool === "files" ? "" : "is-hidden"}`}>
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
        </div>
      </section>
    </>
  );
}
