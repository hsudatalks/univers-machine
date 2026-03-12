import { LayoutDashboard, Settings2, SquareTerminal } from "lucide-react";
import { useEffect, useMemo, useRef, useState } from "react";
import { useHorizontalPanelTrack } from "../hooks/useHorizontalPanelTrack";
import { useMediaQuery } from "../hooks/useMediaQuery";
import type { DeveloperTarget, ManagedMachine } from "../types";
import { TerminalPane } from "./TerminalPane";
import { Button } from "./ui/button";
import { ServerDashboardPane } from "./ServerDashboardPane";
import { ServerTerminalsPane } from "./ServerTerminalsPane";

interface ServerPageProps {
  onOpenSettings: () => void;
  onOpenWorkspace: (targetId: string) => void;
  pageVisible: boolean;
  resolveTarget: (targetId: string) => DeveloperTarget | undefined;
  server: ManagedMachine;
}

type ServerToolPanel = "dashboard" | "terminals";
type MobileServerPanel = "terminal" | ServerToolPanel;

export function ServerPage({
  onOpenSettings,
  onOpenWorkspace,
  pageVisible,
  resolveTarget,
  server,
}: ServerPageProps) {
  const [activeTool, setActiveTool] = useState<ServerToolPanel>("dashboard");
  const terminalTarget = resolveTarget(server.hostTargetId);
  const isMobileLayout = useMediaQuery("(max-width: 960px)");
  const mobilePanelIds = useMemo<MobileServerPanel[]>(
    () => ["terminal", "dashboard", "terminals"],
    [],
  );
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

  useEffect(() => {
    if (!isMobileLayout) {
      hasSyncedMobilePanelRef.current = false;
      return;
    }

    const behavior = hasSyncedMobilePanelRef.current ? "smooth" : "auto";

    hasSyncedMobilePanelRef.current = true;
    setActivePanel(activeTool);
    scrollToPanel(activeTool, behavior);
  }, [activeTool, isMobileLayout, scrollToPanel, setActivePanel]);

  return (
    <>
      <header className="content-header">
        <div className="content-header-copy">
          <span className="panel-title">Machine</span>
          <div className="content-title-row">
            <h1 className="content-title content-title-container">{server.label}</h1>
            <span className="content-chip">{server.host}</span>
          </div>
        </div>

        <div className="content-header-tools">
          <Button
            aria-label={`Open ${server.label} settings`}
            onClick={onOpenSettings}
            size="icon"
            title="Machine settings"
            variant="ghost"
          >
            <Settings2 size={16} />
          </Button>
          <Button
            aria-label="Host terminal"
            className="content-header-tool-mobile-only"
            isActive={isMobileLayout ? activeMobilePanel === "terminal" : false}
            onClick={() => {
              setActivePanel("terminal");
              scrollToPanel("terminal");
            }}
            size="icon"
            title="Host terminal"
            variant={isMobileLayout && activeMobilePanel === "terminal" ? "default" : "ghost"}
          >
            <SquareTerminal size={16} />
          </Button>
          <Button
            aria-label="Machine dashboard"
            isActive={isMobileLayout ? activeMobilePanel === "dashboard" : activeTool === "dashboard"}
            onClick={() => {
              setActiveTool("dashboard");

              if (isMobileLayout) {
                setActivePanel("dashboard");
                scrollToPanel("dashboard");
              }
            }}
            size="icon"
            title="Machine dashboard"
            variant={
              isMobileLayout
                ? activeMobilePanel === "dashboard"
                  ? "default"
                  : "ghost"
                : activeTool === "dashboard"
                  ? "default"
                  : "ghost"
            }
          >
            <LayoutDashboard size={16} />
          </Button>
          <Button
            aria-label="Container terminals"
            isActive={isMobileLayout ? activeMobilePanel === "terminals" : activeTool === "terminals"}
            onClick={() => {
              setActiveTool("terminals");

              if (isMobileLayout) {
                setActivePanel("terminals");
                scrollToPanel("terminals");
              }
            }}
            size="icon"
            title="Container terminals"
            variant={
              isMobileLayout
                ? activeMobilePanel === "terminals"
                  ? "default"
                  : "ghost"
                : activeTool === "terminals"
                  ? "default"
                  : "ghost"
            }
          >
            <SquareTerminal size={16} />
          </Button>
        </div>
      </header>

      <section className="page-section">
        {isMobileLayout ? (
          <div className="mobile-workspace server-mobile-workspace">
            <div className="mobile-panel-track" onScroll={handleTrackScroll} ref={trackRef}>
              <div
                className="mobile-panel-slide"
                data-mobile-panel="terminal"
                ref={registerPanel("terminal")}
              >
                <article className="panel terminal-panel mobile-panel-card">
                  {terminalTarget ? (
                    <TerminalPane
                      active={pageVisible && activeMobilePanel === "terminal"}
                      target={terminalTarget}
                      title={`${server.label} host`}
                    />
                  ) : (
                    <section className="state-panel">
                      <span className="state-label">Host unavailable</span>
                      <p className="state-copy">
                        The Host workspace for this machine is not available in the current inventory.
                      </p>
                    </section>
                  )}
                </article>
              </div>

              <div
                className="mobile-panel-slide"
                data-mobile-panel="dashboard"
                ref={registerPanel("dashboard")}
              >
                <ServerDashboardPane
                  onOpenWorkspace={onOpenWorkspace}
                  resolveTarget={resolveTarget}
                  server={server}
                />
              </div>

              <div
                className="mobile-panel-slide"
                data-mobile-panel="terminals"
                ref={registerPanel("terminals")}
              >
                <ServerTerminalsPane
                  onOpenWorkspace={onOpenWorkspace}
                  pageVisible={pageVisible && activeMobilePanel === "terminals"}
                  resolveTarget={resolveTarget}
                  server={server}
                />
              </div>
            </div>
          </div>
        ) : (
          <div className="server-workspace">
            <article className="panel terminal-panel">
              {terminalTarget ? (
                <TerminalPane
                  active={pageVisible}
                  target={terminalTarget}
                  title={`${server.label} host`}
                />
              ) : (
                <section className="state-panel">
                  <span className="state-label">Host unavailable</span>
                  <p className="state-copy">
                    The Host workspace for this machine is not available in the current inventory.
                  </p>
                </section>
              )}
            </article>

            <div className={`server-pane-slot ${activeTool === "dashboard" ? "" : "is-hidden"}`}>
              <ServerDashboardPane
                onOpenWorkspace={onOpenWorkspace}
                resolveTarget={resolveTarget}
                server={server}
              />
            </div>

            <div className={`server-pane-slot ${activeTool === "terminals" ? "" : "is-hidden"}`}>
              <ServerTerminalsPane
                onOpenWorkspace={onOpenWorkspace}
                pageVisible={pageVisible && activeTool === "terminals"}
                resolveTarget={resolveTarget}
                server={server}
              />
            </div>
          </div>
        )}
      </section>
    </>
  );
}
