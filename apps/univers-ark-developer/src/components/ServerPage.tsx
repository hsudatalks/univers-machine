import { LayoutDashboard, Settings2, SquareTerminal } from "lucide-react";
import { useState } from "react";
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

export function ServerPage({
  onOpenSettings,
  onOpenWorkspace,
  pageVisible,
  resolveTarget,
  server,
}: ServerPageProps) {
  const [activeTool, setActiveTool] = useState<ServerToolPanel>("dashboard");
  const terminalTarget = resolveTarget(server.hostTargetId);

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
            aria-label="Machine dashboard"
            isActive={activeTool === "dashboard"}
            onClick={() => {
              setActiveTool("dashboard");
            }}
            size="icon"
            title="Machine dashboard"
            variant={activeTool === "dashboard" ? "default" : "ghost"}
          >
            <LayoutDashboard size={16} />
          </Button>
          <Button
            aria-label="Container terminals"
            isActive={activeTool === "terminals"}
            onClick={() => {
              setActiveTool("terminals");
            }}
            size="icon"
            title="Container terminals"
            variant={activeTool === "terminals" ? "default" : "ghost"}
          >
            <SquareTerminal size={16} />
          </Button>
        </div>
      </header>

      <section className="page-section">
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
      </section>
    </>
  );
}
