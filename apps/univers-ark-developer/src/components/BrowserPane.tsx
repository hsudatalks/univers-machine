import { useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import { Maximize2, Minimize2 } from "lucide-react";
import { releaseBrowserFrames, syncBrowserFrames } from "../lib/browser-cache";
import { openExternalLink } from "../lib/tauri";
import { Button } from "./ui/button";
import { Badge } from "./ui/badge";
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
  activeServiceId: string | null;
  isVisible: boolean;
  onReload: () => void;
  onSelectService: (serviceId: string) => void;
  retainedFrames: BrowserFrameInstance[];
  services: Array<{ id: string; label: string }>;
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
  activeServiceId,
  isVisible,
  onReload,
  onSelectService,
  retainedFrames,
  services,
  slotLabel,
}: BrowserPaneProps) {
  const stageRef = useRef<HTMLDivElement | null>(null);
  const ownerId = useMemo(() => Symbol("browser-pane"), []);
  const [isFullscreen, setIsFullscreen] = useState(false);
  const tunnelStatusLabel = activeFrame
    ? TUNNEL_STATUS_LABELS[activeFrame.status.state] ?? activeFrame.status.state
    : "Unavailable";
  const activeLocalUrl =
    activeFrame?.status.localUrl ?? activeFrame?.surface.localUrl ?? null;
  const compactStatus =
    activeFrame?.status.state === "direct" || activeFrame?.status.state === "running";
  const showBrowserOverlay =
    !activeFrame ||
    (activeFrame.status.state === "starting" && !activeLocalUrl) ||
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
        src: frame.status.localUrl ?? frame.surface.localUrl,
        title: `${frame.target.label} ${frame.surface.label}`,
      })),
    );
  }, [ownerId, retainedFrames]);

  useEffect(() => {
    return () => {
      releaseBrowserFrames(ownerId);
    };
  }, [ownerId]);

  useEffect(() => {
    if (!isFullscreen) {
      return;
    }

    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        event.preventDefault();
        setIsFullscreen(false);
      }
    };

    window.addEventListener("keydown", handleKeyDown, true);

    return () => {
      window.removeEventListener("keydown", handleKeyDown, true);
    };
  }, [isFullscreen]);

  useEffect(() => {
    if (!isVisible) {
      setIsFullscreen(false);
    }
  }, [isVisible]);

  return (
    <article
      className={`panel browser-panel tool-panel ${isVisible ? "" : "is-hidden"} ${isFullscreen ? "is-pane-fullscreen" : ""}`}
    >
      <header className="panel-header browser-header browser-header-compact tool-panel-header">
        <div className="browser-heading browser-heading-compact">
          <select
            aria-label="Select web service"
            className="browser-service-select"
            disabled={services.length === 0}
            onChange={(event) => {
              if (event.target.value) {
                onSelectService(event.target.value);
              }
            }}
            value={activeServiceId ?? ""}
          >
            {services.length === 0 ? <option value="">No web services</option> : null}
            {services.map((service) => (
              <option key={service.id} value={service.id}>
                {service.label}
              </option>
            ))}
          </select>

          <code className="browser-url browser-url-compact">
            {activeLocalUrl ?? "No local browser URL"}
          </code>
        </div>

        <div className="browser-bar">
          <Button
            aria-label={isFullscreen ? "Exit fullscreen" : "Fullscreen browser"}
            isActive={isFullscreen}
            onClick={() => {
              setIsFullscreen((current) => !current);
            }}
            size="icon"
            title={isFullscreen ? "Exit fullscreen" : "Fullscreen browser"}
            variant={isFullscreen ? "default" : "ghost"}
          >
            {isFullscreen ? <Minimize2 size={14} /> : <Maximize2 size={14} />}
          </Button>
          {compactStatus ? (
            <span
              aria-label={tunnelStatusLabel}
              className={`terminal-status terminal-status-dot status-${activeFrame?.status.state ?? "stopped"}`}
              title={tunnelStatusLabel}
            />
          ) : (
            <Badge variant={activeFrame?.status.state === "error" || activeFrame?.status.state === "stopped" ? "destructive" : activeFrame?.status.state === "starting" ? "warning" : "success"}>
              {tunnelStatusLabel}
            </Badge>
          )}
          <Button
            disabled={!activeFrame}
            onClick={onReload}
            size="sm"
            variant="outline"
          >
            Reload
          </Button>
          {activeFrame ? (
            <Button
              className="panel-link"
              onClick={() => {
                if (activeLocalUrl) {
                  void openExternalLink(activeLocalUrl);
                }
              }}
              size="sm"
              variant="outline"
            >
              Open
            </Button>
          ) : (
            <Button disabled size="sm" variant="outline">
              Open
            </Button>
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
