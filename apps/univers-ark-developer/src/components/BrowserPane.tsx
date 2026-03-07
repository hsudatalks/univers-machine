import { useEffect, useLayoutEffect, useMemo, useRef } from "react";
import { releaseBrowserFrames, syncBrowserFrames } from "../lib/browser-cache";
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
    <article className="panel browser-panel tool-panel">
      <header className="panel-header browser-header tool-panel-header">
        <div className="browser-heading">
          <div className="browser-pane-copy">
            <span className="panel-title">{slotLabel}</span>
            <span className="panel-meta">
              {activeFrame?.target.host ?? "No surface on current target"}
            </span>
          </div>

          <code className="browser-url">
            {activeFrame?.surface.localUrl ?? "No local browser URL"}
          </code>
        </div>

        <div className="browser-bar">
          <span
            className={`terminal-status status-${activeFrame?.status.state ?? "stopped"}`}
          >
            {tunnelStatusLabel}
          </span>
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
            <a
              className="panel-button panel-link"
              href={activeFrame.surface.localUrl}
              rel="noreferrer"
              target="_blank"
            >
              Open
            </a>
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
