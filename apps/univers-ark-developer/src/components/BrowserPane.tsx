import { useEffect, useLayoutEffect, useMemo, useRef } from "react";
import { releaseBrowserFrames, syncBrowserFrames } from "../lib/browser-cache";
import { openExternalLink } from "../lib/tauri";
import type {
  DeveloperSurface,
  DeveloperTarget,
  TunnelStatus,
} from "../types";

export interface BrowserFrameInstance {
  cacheKey: string;
  frameVersion: number;
  isActive: boolean;
  status: TunnelStatus;
  surface: DeveloperSurface;
  target: DeveloperTarget;
}

interface BrowserPaneProps {
  activeFrame?: BrowserFrameInstance;
  isVisible: boolean;
  onReload: () => void;
  onRestart: () => void;
  retainedFrames: BrowserFrameInstance[];
  slotLabel: string;
}

const TUNNEL_STATUS_LABELS: Record<string, string> = {
  direct: "Direct",
  error: "Error",
  running: "Running",
  starting: "Starting",
  stopped: "Stopped",
};

export function BrowserPane({
  activeFrame,
  isVisible,
  onReload,
  onRestart,
  retainedFrames,
  slotLabel,
}: BrowserPaneProps) {
  const stageRef = useRef<HTMLDivElement | null>(null);
  const ownerId = useMemo(() => Symbol(slotLabel), [slotLabel]);
  const tunnelStatusLabel = activeFrame
    ? TUNNEL_STATUS_LABELS[activeFrame.status.state] ?? activeFrame.status.state
    : "Unavailable";
  const compactStatus =
    activeFrame?.status.state === "direct" || activeFrame?.status.state === "running";
  const showBrowserOverlay =
    !activeFrame ||
    activeFrame.status.state === "starting" ||
    activeFrame.status.state === "stopped" ||
    activeFrame.status.state === "error";
  const overlayMessage = activeFrame
    ? activeFrame.status.message
    : `The selected container does not currently expose a ${slotLabel.toLowerCase()} browser surface.`;

  useLayoutEffect(() => {
    const stageElement = stageRef.current;

    if (!stageElement) {
      return;
    }

    syncBrowserFrames(
      ownerId,
      stageElement,
      retainedFrames.map((frame) => ({
        cacheKey: frame.cacheKey,
        frameVersion: frame.frameVersion,
        isActive: frame.isActive,
        src: frame.surface.localUrl,
        title: `${frame.target.label} ${frame.surface.label}`,
      })),
    );
  }, [ownerId, retainedFrames]);

  useEffect(() => {
    return () => {
      releaseBrowserFrames(ownerId);
    };
  }, [ownerId]);

  return (
    <article className={`panel browser-panel tool-panel ${isVisible ? "" : "is-hidden"}`}>
      <header className="panel-header browser-header browser-header-compact tool-panel-header">
        <code className="browser-url browser-url-compact">
          {activeFrame?.surface.localUrl ?? "No local browser URL"}
        </code>

        <div className="browser-bar">
          {compactStatus ? (
            <span
              aria-label={tunnelStatusLabel}
              className={`terminal-status terminal-status-dot status-${activeFrame?.status.state ?? "stopped"}`}
              title={tunnelStatusLabel}
            />
          ) : (
            <span
              className={`terminal-status status-${activeFrame?.status.state ?? "stopped"}`}
            >
              {tunnelStatusLabel}
            </span>
          )}
          <button
            className="panel-button"
            disabled={!activeFrame?.surface.tunnelCommand}
            onClick={onRestart}
            type="button"
          >
            Restart Tunnel
          </button>
          <button
            className="panel-button"
            disabled={!activeFrame}
            onClick={onReload}
            type="button"
          >
            Reload
          </button>
          {activeFrame ? (
            <button
              className="panel-button panel-link"
              onClick={() => {
                void openExternalLink(activeFrame.surface.localUrl);
              }}
              type="button"
            >
              Open
            </button>
          ) : (
            <button className="panel-button" disabled type="button">
              Open
            </button>
          )}
        </div>
      </header>

      <div className="browser-stage" ref={stageRef}>
        {showBrowserOverlay ? (
          <div className="browser-overlay">
            <div className="browser-placeholder">
              <span className="state-label">{slotLabel}</span>
              <p className="browser-placeholder-title">{tunnelStatusLabel}</p>
              <p className="browser-placeholder-copy">{overlayMessage}</p>
            </div>
          </div>
        ) : null}
      </div>
    </article>
  );
}
