import type { OrchestrationViewMode } from "../hooks/useOrchestrationViewMode";
import { ConnectionStatusLight } from "./ConnectionStatusLight";
import { GithubPopover } from "./GithubPopover";
import { Badge } from "./ui/badge";
import { Button } from "./ui/button";

type MachineNavEntry = {
  id: string;
  label: string;
};

type StatusBarProps = {
  activeMachineId?: string;
  activeStatusLabel: string;
  containerCount: number;
  isOrchestrationActive: boolean;
  isSidebarHidden: boolean;
  machineEntries: MachineNavEntry[];
  onNavigateMachine?: (machineId: string) => void;
  onSetOrchestrationViewMode: (viewMode: OrchestrationViewMode) => void;
  onOpenSettings: () => void;
  onResetOverviewZoom: () => void;
  onToggleSidebar: () => void;
  onZoomInOverview: () => void;
  onZoomOutOverview: () => void;
  orchestrationViewMode: OrchestrationViewMode;
  overviewZoom: number;
  overviewZoomDefault: number;
  overviewZoomMax: number;
  overviewZoomMin: number;
  reachableContainerCount: number;
  serverCount: number;
};

export function StatusBar({
  activeMachineId,
  activeStatusLabel,
  containerCount,
  isOrchestrationActive,
  isSidebarHidden,
  machineEntries,
  onNavigateMachine,
  onSetOrchestrationViewMode,
  onOpenSettings,
  onResetOverviewZoom,
  onToggleSidebar,
  onZoomInOverview,
  onZoomOutOverview,
  orchestrationViewMode,
  overviewZoom,
  overviewZoomDefault,
  overviewZoomMax,
  overviewZoomMin,
  reachableContainerCount,
  serverCount,
}: StatusBarProps) {
  const activeMachineIndex = activeMachineId
    ? machineEntries.findIndex((entry) => entry.id === activeMachineId)
    : -1;
  const hasMachineNav =
    activeMachineIndex >= 0 && machineEntries.length > 1 && onNavigateMachine;
  const prevMachine = hasMachineNav
    ? machineEntries[activeMachineIndex - 1]
    : undefined;
  const nextMachine = hasMachineNav
    ? machineEntries[activeMachineIndex + 1]
    : undefined;
  const activeMachineLabel = hasMachineNav
    ? machineEntries[activeMachineIndex].label
    : undefined;
  return (
    <footer className="status-bar" role="status">
      <div className="status-bar-section status-bar-section-primary">
        <Button
          aria-label={isSidebarHidden ? "Show sidebar menu" : "Hide sidebar menu"}
          className="status-bar-button"
          onClick={onToggleSidebar}
          size="sm"
          title={isSidebarHidden ? "Show sidebar menu" : "Hide sidebar menu"}
          variant="ghost"
        >
          <svg
            aria-hidden="true"
            className="panel-button-icon-svg"
            fill="none"
            viewBox="0 0 16 16"
          >
            {isSidebarHidden ? (
              <path
                d="M2.75 3.25h10.5M2.75 8h10.5M2.75 12.75h10.5"
                stroke="currentColor"
                strokeLinecap="round"
                strokeWidth="1.25"
              />
            ) : (
              <>
                <path
                  d="M3.25 3.25v9.5M6.25 4h6.5M6.25 8h6.5M6.25 12h6.5"
                  stroke="currentColor"
                  strokeLinecap="round"
                  strokeWidth="1.25"
                />
                <path
                  d="M3.25 8h1.5"
                  stroke="currentColor"
                  strokeLinecap="round"
                  strokeWidth="1.25"
                />
              </>
            )}
          </svg>
        </Button>
        <Badge className="status-bar-chip" variant="neutral">
          {activeStatusLabel}
        </Badge>
        <ConnectionStatusLight className="status-bar-state-light" state="ready" title="Ready" />
      </div>

      <div className="status-bar-section status-bar-section-center">
        {hasMachineNav ? (
          <div className="status-bar-machine-nav" aria-label="Machine navigation">
            <Button
              aria-label={prevMachine ? `Go to ${prevMachine.label}` : "No previous machine"}
              className="status-bar-button"
              disabled={!prevMachine}
              onClick={() => prevMachine && onNavigateMachine(prevMachine.id)}
              size="sm"
              title={prevMachine ? prevMachine.label : "No previous machine"}
              variant="ghost"
            >
              <svg aria-hidden="true" className="panel-button-icon-svg" fill="none" viewBox="0 0 16 16">
                <path d="M10 3L5.5 8l4.5 5" stroke="currentColor" strokeLinecap="round" strokeLinejoin="round" strokeWidth="1.4" />
              </svg>
            </Button>
            <span className="status-bar-machine-label">{activeMachineLabel}</span>
            <Button
              aria-label={nextMachine ? `Go to ${nextMachine.label}` : "No next machine"}
              className="status-bar-button"
              disabled={!nextMachine}
              onClick={() => nextMachine && onNavigateMachine(nextMachine.id)}
              size="sm"
              title={nextMachine ? nextMachine.label : "No next machine"}
              variant="ghost"
            >
              <svg aria-hidden="true" className="panel-button-icon-svg" fill="none" viewBox="0 0 16 16">
                <path d="M6 3l4.5 5L6 13" stroke="currentColor" strokeLinecap="round" strokeLinejoin="round" strokeWidth="1.4" />
              </svg>
            </Button>
          </div>
        ) : isOrchestrationActive ? (
          <>
            <div className="status-bar-view-switcher" aria-label="Orchestration views" role="tablist">
              {(["grid", "focus"] as const).map((viewMode) => (
                <Button
                  aria-pressed={orchestrationViewMode === viewMode}
                  className={`status-bar-view-button ${orchestrationViewMode === viewMode ? "is-active" : ""
                    }`}
                  key={viewMode}
                  onClick={() => {
                    onSetOrchestrationViewMode(viewMode);
                  }}
                  size="sm"
                  title={viewMode === "grid" ? "Show grid view" : "Show focus view"}
                  type="button"
                  variant="ghost"
                >
                  {viewMode === "grid" ? "Grid" : "Focus"}
                </Button>
              ))}
            </div>
            <div className="status-bar-zoom" aria-label="Orchestration zoom controls">
              <Button
                aria-label="Zoom out orchestration terminals"
                className="status-bar-button"
                disabled={overviewZoom <= overviewZoomMin}
                onClick={onZoomOutOverview}
                size="sm"
                title="Zoom out orchestration terminals"
                variant="ghost"
              >
                -
              </Button>
              <Button
                aria-label="Reset orchestration zoom"
                className="status-bar-zoom-readout"
                disabled={overviewZoom === overviewZoomDefault}
                onClick={onResetOverviewZoom}
                size="sm"
                title="Reset orchestration zoom"
                variant="ghost"
              >
                {Math.round(overviewZoom * 100)}%
              </Button>
              <Button
                aria-label="Zoom in orchestration terminals"
                className="status-bar-button"
                disabled={overviewZoom >= overviewZoomMax}
                onClick={onZoomInOverview}
                size="sm"
                title="Zoom in orchestration terminals"
                variant="ghost"
              >
                +
              </Button>
            </div>
          </>
        ) : null}
      </div>

      <div className="status-bar-section status-bar-section-secondary">
        <span className="status-bar-text">
          <span className="status-bar-metric">{serverCount}</span> srv
        </span>
        <span className="status-bar-text">
          <span className="status-bar-metric">{containerCount}</span> ctr
        </span>
        <span className="status-bar-text">
          <span className="status-bar-metric">{reachableContainerCount}</span> ssh
        </span>
        <GithubPopover />
        <Button
          aria-label="Settings"
          className="status-bar-button"
          size="sm"
          onClick={onOpenSettings}
          title="Settings"
          variant="ghost"
        >
          <svg
            aria-hidden="true"
            className="panel-button-icon-svg"
            fill="none"
            viewBox="0 0 16 16"
          >
            <path
              d="M6.8 1.5h2.4l.3 1.8.8.4 1.6-.9 1.7 1.7-.9 1.6.4.8 1.8.3v2.4l-1.8.3-.4.8.9 1.6-1.7 1.7-1.6-.9-.8.4-.3 1.8H6.8l-.3-1.8-.8-.4-1.6.9-1.7-1.7.9-1.6-.4-.8-1.8-.3V6.8l1.8-.3.4-.8-.9-1.6 1.7-1.7 1.6.9.8-.4z"
              stroke="currentColor"
              strokeLinecap="round"
              strokeLinejoin="round"
              strokeWidth="1.1"
            />
            <circle
              cx="8"
              cy="8"
              r="2"
              stroke="currentColor"
              strokeWidth="1.1"
            />
          </svg>
        </Button>
      </div>
    </footer>
  );
}
