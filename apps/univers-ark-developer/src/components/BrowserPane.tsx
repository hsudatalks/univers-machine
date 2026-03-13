import { useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import {
  Camera,
  Check,
  ExternalLink,
  Maximize2,
  Minimize2,
  RotateCw,
} from "lucide-react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { releaseBrowserFrames, syncBrowserFrames } from "../lib/browser-cache";
import { captureBrowserScreenshot, clipboardWrite, openExternalLink } from "../lib/tauri";
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
  const [isCapturingScreenshot, setIsCapturingScreenshot] = useState(false);
  const [captureFeedback, setCaptureFeedback] = useState<string | null>(null);
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

  useEffect(() => {
    if (!captureFeedback) {
      return;
    }

    const timeout = window.setTimeout(() => {
      setCaptureFeedback(null);
    }, 1800);

    return () => {
      window.clearTimeout(timeout);
    };
  }, [captureFeedback]);

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
            aria-label={isFullscreen ? "Exit fullscreen" : "Fullscreen browser"}
            isActive={isFullscreen}
            onClick={() => {
              setIsFullscreen((current) => !current);
            }}
            size="icon"
            title={isFullscreen ? "Exit fullscreen" : "Fullscreen browser"}
            variant="outline"
          >
            {isFullscreen ? <Minimize2 size={14} /> : <Maximize2 size={14} />}
          </Button>
          <Button
            aria-label={
              isCapturingScreenshot
                ? "Capturing browser screenshot"
                : captureFeedback ?? "Capture browser screenshot"
            }
            disabled={!activeFrame || !activeLocalUrl || isCapturingScreenshot}
            onClick={async () => {
              const stageElement = stageRef.current;

              if (!activeFrame || !activeLocalUrl || !stageElement) {
                return;
              }

              const bounds = stageElement.getBoundingClientRect();
              if (bounds.width < 2 || bounds.height < 2) {
                return;
              }

              setIsCapturingScreenshot(true);

              try {
                const currentWindow = getCurrentWindow();
                const [innerPosition, scaleFactor] = await Promise.all([
                  currentWindow.innerPosition(),
                  currentWindow.scaleFactor(),
                ]);

                const result = await captureBrowserScreenshot(
                  activeFrame.target.id,
                  activeFrame.surface.id,
                  {
                    x: Math.round(innerPosition.x + bounds.left * scaleFactor),
                    y: Math.round(innerPosition.y + bounds.top * scaleFactor),
                    width: Math.max(1, Math.round(bounds.width * scaleFactor)),
                    height: Math.max(1, Math.round(bounds.height * scaleFactor)),
                  },
                );

                await clipboardWrite(result.path);
                setCaptureFeedback("Path copied");
              } catch (error) {
                const message =
                  error instanceof Error ? error.message : "Failed to capture screenshot.";
                setCaptureFeedback(message);
              } finally {
                setIsCapturingScreenshot(false);
              }
            }}
            title={
              isCapturingScreenshot
                ? "Capturing screenshot…"
                : captureFeedback ?? "Capture browser screenshot to the target container"
            }
            size="icon"
            variant="outline"
          >
            {captureFeedback === "Path copied" ? <Check size={14} /> : <Camera size={14} />}
          </Button>
          <Button
            aria-label="Reload browser"
            disabled={!activeFrame}
            onClick={onReload}
            size="icon"
            title="Reload browser"
            variant="outline"
          >
            <RotateCw size={14} />
          </Button>
          {activeFrame ? (
            <Button
              aria-label="Open browser externally"
              className="panel-link"
              onClick={() => {
                if (activeLocalUrl) {
                  void openExternalLink(activeLocalUrl);
                }
              }}
              size="icon"
              title="Open browser externally"
              variant="outline"
            >
              <ExternalLink size={14} />
            </Button>
          ) : (
            <Button aria-label="Open browser externally" disabled size="icon" title="Open browser externally" variant="outline">
              <ExternalLink size={14} />
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
