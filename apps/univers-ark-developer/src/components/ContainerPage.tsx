import {
  useEffect,
  useMemo,
  useRef,
  useState,
  type CSSProperties,
  type PointerEvent as ReactPointerEvent,
} from "react";
import { BrowserPane, type BrowserFrameInstance } from "./BrowserPane";
import { DashboardPane } from "./DashboardPane";
import { FilesPane } from "./FilesPane";
import { ServicesPane } from "./ServicesPane";
import { TerminalPane } from "./TerminalPane";
import { Button } from "./ui/button";
import type { ContainerToolPanel } from "../lib/view-types";
import { useHorizontalPanelTrack } from "../hooks/useHorizontalPanelTrack";
import { useMediaQuery } from "../hooks/useMediaQuery";
import type {
  DeveloperSurface,
  DeveloperTarget,
  ServiceStatus,
  TunnelStatus,
} from "../types";
import { FolderOpen, Globe, LayoutDashboard, Rows4, SquareTerminal } from "lucide-react";

interface ContainerPageProps {
  activeTool: ContainerToolPanel;
  dashboardRefreshSeconds: number;
  browserFrame?: BrowserFrameInstance;
  browserFrames: BrowserFrameInstance[];
  browserPanel: ContainerToolPanel | null;
  browserServices: Array<{ id: string; label: string }>;
  browserSurface?: DeveloperSurface;
  primaryBrowserStatus?: TunnelStatus;
  primaryBrowserSurface?: DeveloperSurface;
  isTerminalCollapsed: boolean;
  onExecuteCommandService: (serviceId: string, action: "restart") => Promise<void>;
  onOpenBrowserService: (serviceId: string) => void;
  onResetBrowser: () => void;
  onRestartContainer?: () => Promise<void>;
  onSelectTool: (panel: ContainerToolPanel) => void;
  onStartResize: (event: ReactPointerEvent<HTMLDivElement>) => void;
  onToggleTerminalCollapsed: () => void;
  pageVisible: boolean;
  serviceStatuses: Record<string, ServiceStatus>;
  target: DeveloperTarget;
  workspaceStyle: CSSProperties;
}

type MobileContainerPanel = "terminal" | "dashboard" | "services" | "files" | "browser";

export function ContainerPage({
  activeTool,
  dashboardRefreshSeconds,
  browserFrame,
  browserFrames,
  browserPanel,
  browserServices,
  browserSurface,
  primaryBrowserStatus,
  primaryBrowserSurface,
  isTerminalCollapsed,
  onExecuteCommandService,
  onOpenBrowserService,
  onResetBrowser,
  onRestartContainer,
  onSelectTool,
  onStartResize,
  onToggleTerminalCollapsed,
  pageVisible,
  serviceStatuses,
  target,
  workspaceStyle,
}: ContainerPageProps) {
  const [isRestarting, setIsRestarting] = useState(false);
  const isMobileLayout = useMediaQuery("(max-width: 960px)");
  const mobilePanelIds = useMemo(() => {
    const panels: MobileContainerPanel[] = ["terminal", "dashboard", "services", "files"];

    if (browserSurface) {
      panels.push("browser");
    }

    return panels;
  }, [browserSurface]);
  const {
    activePanel: activeMobilePanel,
    handleTrackScroll,
    registerPanel,
    scrollToPanel,
    setActivePanel,
    trackRef,
  } = useHorizontalPanelTrack({
    enabled: isMobileLayout,
    initialPanel: "terminal",
    panelIds: mobilePanelIds,
  });
  const hasSyncedMobilePanelRef = useRef(false);
  const selectedToolPanel: MobileContainerPanel =
    activeTool === "dashboard"
      ? "dashboard"
      : activeTool === "services"
        ? "services"
        : activeTool === "files"
          ? "files"
          : "browser";

  useEffect(() => {
    if (!isMobileLayout) {
      hasSyncedMobilePanelRef.current = false;
      return;
    }

    const behavior = hasSyncedMobilePanelRef.current ? "smooth" : "auto";

    hasSyncedMobilePanelRef.current = true;
    setActivePanel(selectedToolPanel);
    scrollToPanel(selectedToolPanel, behavior);
  }, [isMobileLayout, scrollToPanel, selectedToolPanel, setActivePanel]);

  return (
    <>
      {isMobileLayout ? (
        <header className="content-header content-header-mobile">
          <div className="content-header-mobile-topline">
            <div className="content-header-copy">
              <span className="panel-title">Container</span>
              <div className="content-title-row">
                <h1 className="content-title content-title-container">{target.label}</h1>
              </div>
            </div>

            {onRestartContainer ? (
              <div className="content-header-mobile-actions">
                <Button
                  aria-label={isRestarting ? "Restarting container" : "Restart container"}
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
              </div>
            ) : null}
          </div>

          <div aria-label="Container panels" className="content-panel-rail">
            <Button
              className="content-panel-rail-button"
              isActive={activeMobilePanel === "terminal"}
              onClick={() => {
                setActivePanel("terminal");
                scrollToPanel("terminal");
              }}
              size="sm"
              variant={activeMobilePanel === "terminal" ? "default" : "outline"}
            >
              <SquareTerminal size={14} />
              Terminal
            </Button>
            <Button
              className="content-panel-rail-button"
              isActive={activeMobilePanel === "dashboard"}
              onClick={() => {
                onSelectTool("dashboard");
                setActivePanel("dashboard");
                scrollToPanel("dashboard");
              }}
              size="sm"
              variant={activeMobilePanel === "dashboard" ? "default" : "outline"}
            >
              <LayoutDashboard size={14} />
              Dashboard
            </Button>
            <Button
              className="content-panel-rail-button"
              isActive={activeMobilePanel === "services"}
              onClick={() => {
                onSelectTool("services");
                setActivePanel("services");
                scrollToPanel("services");
              }}
              size="sm"
              variant={activeMobilePanel === "services" ? "default" : "outline"}
            >
              <Rows4 size={14} />
              Services
            </Button>
            <Button
              className="content-panel-rail-button"
              isActive={activeMobilePanel === "files"}
              onClick={() => {
                onSelectTool("files");
                setActivePanel("files");
                scrollToPanel("files");
              }}
              size="sm"
              variant={activeMobilePanel === "files" ? "default" : "outline"}
            >
              <FolderOpen size={14} />
              Files
            </Button>
            {browserSurface ? (
              <Button
                className="content-panel-rail-button"
                isActive={activeMobilePanel === "browser"}
                onClick={() => {
                  if (browserPanel) {
                    onSelectTool(browserPanel);
                    setActivePanel("browser");
                    scrollToPanel("browser");
                  }
                }}
                size="sm"
                variant={activeMobilePanel === "browser" ? "default" : "outline"}
              >
                <Globe size={14} />
                {primaryBrowserSurface?.label ?? "Browser"}
              </Button>
            ) : null}
          </div>
        </header>
      ) : (
        <header className="content-header content-header-container">
          <div className="content-header-leading">
            <Button
              aria-label={
                isTerminalCollapsed ? "Show terminal pane" : "Hide terminal pane"
              }
              className="content-title-toggle content-title-toggle-desktop"
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
              aria-label="Services"
              isActive={activeTool === "services"}
              onClick={() => {
                onSelectTool("services");
              }}
              size="icon"
              title="Services"
              variant={activeTool === "services" ? "default" : "ghost"}
            >
              <Rows4 size={16} />
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
              aria-label={primaryBrowserSurface?.label ?? "Primary browser"}
              disabled={!primaryBrowserSurface}
              isActive={activeTool === browserPanel}
              onClick={() => {
                if (browserPanel) {
                  onSelectTool(browserPanel);
                }
              }}
              size="icon"
              title={primaryBrowserSurface?.label ?? "Primary browser"}
              variant={activeTool === browserPanel ? "default" : "ghost"}
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
      )}

      <section className="page-section">
        {isMobileLayout ? (
          <div className="mobile-workspace container-mobile-workspace">
            <div className="mobile-panel-track" onScroll={handleTrackScroll} ref={trackRef}>
              <div
                className="mobile-panel-slide"
                data-mobile-panel="terminal"
                ref={registerPanel("terminal")}
              >
                <article className="panel terminal-panel mobile-panel-card">
                  <TerminalPane
                    active={pageVisible && activeMobilePanel === "terminal"}
                    target={target}
                  />
                </article>
              </div>

              <div
                className="mobile-panel-slide"
                data-mobile-panel="dashboard"
                ref={registerPanel("dashboard")}
              >
                <DashboardPane
                  key={`${target.id}:${dashboardRefreshSeconds}`}
                  dashboardRefreshSeconds={dashboardRefreshSeconds}
                  primaryBrowserLabel={primaryBrowserSurface?.label}
                  primaryBrowserStatus={primaryBrowserStatus}
                  primaryBrowserUrl={
                    primaryBrowserStatus?.localUrl ?? primaryBrowserSurface?.localUrl
                  }
                  serviceStatuses={serviceStatuses}
                  target={target}
                />
              </div>

              <div
                className="mobile-panel-slide"
                data-mobile-panel="services"
                ref={registerPanel("services")}
              >
                <ServicesPane
                  activeBrowserServiceId={browserSurface?.id ?? null}
                  onOpenBrowserService={onOpenBrowserService}
                  onRunCommandService={onExecuteCommandService}
                  serviceStatuses={serviceStatuses}
                  target={target}
                />
              </div>

              <div
                className="mobile-panel-slide"
                data-mobile-panel="files"
                ref={registerPanel("files")}
              >
                <FilesPane active={pageVisible && activeMobilePanel === "files"} target={target} />
              </div>

              {browserSurface ? (
                <div
                  className="mobile-panel-slide"
                  data-mobile-panel="browser"
                  ref={registerPanel("browser")}
                >
                  <BrowserPane
                    activeFrame={browserFrame}
                    activeServiceId={browserSurface.id}
                    isVisible={activeMobilePanel === "browser"}
                    onReset={onResetBrowser}
                    onSelectService={onOpenBrowserService}
                    retainedFrames={browserFrames}
                    services={browserServices}
                    slotLabel={browserSurface.label}
                  />
                </div>
              ) : null}
            </div>
          </div>
        ) : (
          <div
            className={`container-workspace ${activeTool === "dashboard" ? "tool-dashboard" : activeTool === "services" ? "tool-services" : activeTool === "files" ? "tool-files" : "tool-browser"} ${isTerminalCollapsed ? "is-terminal-collapsed" : ""}`}
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
                primaryBrowserLabel={primaryBrowserSurface?.label}
                primaryBrowserStatus={primaryBrowserStatus}
                primaryBrowserUrl={primaryBrowserStatus?.localUrl ?? primaryBrowserSurface?.localUrl}
                serviceStatuses={serviceStatuses}
                target={target}
              />
            </div>

            <div className={`services-pane-slot ${activeTool === "services" ? "" : "is-hidden"}`}>
              <ServicesPane
                activeBrowserServiceId={browserSurface?.id ?? null}
                onOpenBrowserService={onOpenBrowserService}
                onRunCommandService={onExecuteCommandService}
                serviceStatuses={serviceStatuses}
                target={target}
              />
            </div>

            <div className={`files-pane-slot ${activeTool === "files" ? "" : "is-hidden"}`}>
              <FilesPane active={pageVisible} target={target} />
            </div>

            {browserSurface ? (
              <BrowserPane
                activeFrame={browserFrame}
                activeServiceId={browserSurface.id}
                isVisible={activeTool === browserPanel}
                onReset={onResetBrowser}
                onSelectService={onOpenBrowserService}
                retainedFrames={browserFrames}
                services={browserServices}
                slotLabel={browserSurface.label}
              />
            ) : null}
          </div>
        )}
      </section>
    </>
  );
}
