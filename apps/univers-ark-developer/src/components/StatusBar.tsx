import { GithubPopover } from "./GithubPopover";

type StatusBarProps = {
  activeStatusLabel: string;
  containerCount: number;
  isOverviewView: boolean;
  isSidebarHidden: boolean;
  onOpenSettings: () => void;
  onToggleSidebar: () => void;
  overviewZoom: number;
  reachableContainerCount: number;
  serverCount: number;
};

export function StatusBar({
  activeStatusLabel,
  containerCount,
  isOverviewView,
  isSidebarHidden,
  onOpenSettings,
  onToggleSidebar,
  overviewZoom,
  reachableContainerCount,
  serverCount,
}: StatusBarProps) {
  return (
    <footer className="status-bar" role="status">
      <div className="status-bar-section">
        <button
          aria-label={isSidebarHidden ? "Show sidebar menu" : "Hide sidebar menu"}
          className="panel-button panel-button-toolbar panel-button-icon status-bar-toggle"
          onClick={onToggleSidebar}
          title={isSidebarHidden ? "Show sidebar menu" : "Hide sidebar menu"}
          type="button"
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
        </button>
        <span className="status-bar-chip">{activeStatusLabel}</span>
        <span className="status-bar-text">Inventory ready</span>
      </div>

      <div className="status-bar-section">
        <span className="status-bar-text">{serverCount} server(s)</span>
        <span className="status-bar-text">{containerCount} container(s)</span>
        <span className="status-bar-text">{reachableContainerCount} SSH ready</span>
        {isOverviewView ? (
          <span className="status-bar-text">
            Overview zoom {Math.round(overviewZoom * 100)}%
          </span>
        ) : null}
        <GithubPopover />
        <button
          aria-label="Settings"
          className="panel-button panel-button-toolbar panel-button-icon"
          onClick={onOpenSettings}
          title="Settings"
          type="button"
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
        </button>
      </div>
    </footer>
  );
}
