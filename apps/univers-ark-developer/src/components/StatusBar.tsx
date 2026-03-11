import { ConnectionStatusLight } from "./ConnectionStatusLight";
import { GithubPopover } from "./GithubPopover";
import { Badge } from "./ui/badge";
import { Button } from "./ui/button";

type StatusBarProps = {
  activeStatusLabel: string;
  containerCount: number;
  isOverviewView: boolean;
  isSidebarHidden: boolean;
  onOpenSettings: () => void;
  onResetOverviewZoom: () => void;
  onToggleSidebar: () => void;
  onZoomInOverview: () => void;
  onZoomOutOverview: () => void;
  overviewZoom: number;
  overviewZoomDefault: number;
  overviewZoomMax: number;
  overviewZoomMin: number;
  reachableContainerCount: number;
  serverCount: number;
};

export function StatusBar({
  activeStatusLabel,
  containerCount,
  isOverviewView,
  isSidebarHidden,
  onOpenSettings,
  onResetOverviewZoom,
  onToggleSidebar,
  onZoomInOverview,
  onZoomOutOverview,
  overviewZoom,
  overviewZoomDefault,
  overviewZoomMax,
  overviewZoomMin,
  reachableContainerCount,
  serverCount,
}: StatusBarProps) {
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
        {isOverviewView ? (
          <div className="status-bar-zoom" aria-label="Overview zoom controls">
            <Button
              aria-label="Zoom out overview terminals"
              className="status-bar-button"
              disabled={overviewZoom <= overviewZoomMin}
              onClick={onZoomOutOverview}
              size="sm"
              title="Zoom out overview terminals"
              variant="ghost"
            >
              -
            </Button>
            <Button
              aria-label="Reset overview zoom"
              className="status-bar-zoom-readout"
              disabled={overviewZoom === overviewZoomDefault}
              onClick={onResetOverviewZoom}
              size="sm"
              title="Reset overview zoom"
              variant="ghost"
            >
              {Math.round(overviewZoom * 100)}%
            </Button>
            <Button
              aria-label="Zoom in overview terminals"
              className="status-bar-button"
              disabled={overviewZoom >= overviewZoomMax}
              onClick={onZoomInOverview}
              size="sm"
              title="Zoom in overview terminals"
              variant="ghost"
            >
              +
            </Button>
          </div>
        ) : null}
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
