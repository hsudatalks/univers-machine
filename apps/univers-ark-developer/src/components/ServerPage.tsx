import { LayoutDashboard, Settings2, SquareTerminal } from "lucide-react";
import { useEffect, useMemo, useRef, useState } from "react";
import { useHorizontalPanelTrack } from "../hooks/useHorizontalPanelTrack";
import { useMediaQuery } from "../hooks/useMediaQuery";
import { visibleContainers } from "../lib/container-visibility";
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
const IS_MAC = navigator.platform.toUpperCase().includes("MAC");

function isPlatformModifier(event: KeyboardEvent): boolean {
  return IS_MAC ? event.metaKey && !event.ctrlKey : event.ctrlKey && !event.metaKey;
}

function isEditableEventTarget(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) {
    return false;
  }

  return (
    target instanceof HTMLInputElement ||
    target instanceof HTMLTextAreaElement ||
    target instanceof HTMLSelectElement ||
    target.isContentEditable
  );
}

function isXtermHelperTextarea(target: EventTarget | null): boolean {
  return (
    target instanceof HTMLTextAreaElement &&
    target.classList.contains("xterm-helper-textarea")
  );
}

export function ServerPage({
  onOpenSettings,
  onOpenWorkspace,
  pageVisible,
  resolveTarget,
  server,
}: ServerPageProps) {
  const [activeTool, setActiveTool] = useState<ServerToolPanel>("dashboard");
  const terminalTarget = resolveTarget(server.hostTargetId);
  const terminalCardElementsRef = useRef(new Map<string, HTMLElement>());
  const focusableContainerTargetIds = useMemo(
    () =>
      visibleContainers(server.containers)
        .map((container) => container.targetId)
        .filter((targetId) => Boolean(resolveTarget(targetId))),
    [resolveTarget, server.containers],
  );
  const focusableTerminalTargetIds = useMemo(
    () =>
      terminalTarget
        ? [terminalTarget.id, ...focusableContainerTargetIds]
        : focusableContainerTargetIds,
    [focusableContainerTargetIds, terminalTarget],
  );
  const [focusedTerminalTargetId, setFocusedTerminalTargetId] = useState<string>(
    () => terminalTarget?.id ?? focusableContainerTargetIds[0] ?? "",
  );
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

  useEffect(() => {
    if (!focusableTerminalTargetIds.length) {
      setFocusedTerminalTargetId("");
      return;
    }

    setFocusedTerminalTargetId((current) =>
      focusableTerminalTargetIds.includes(current)
        ? current
        : terminalTarget?.id ?? focusableContainerTargetIds[0] ?? focusableTerminalTargetIds[0],
    );
  }, [focusableContainerTargetIds, focusableTerminalTargetIds, terminalTarget]);

  useEffect(() => {
    if (!pageVisible || !focusedTerminalTargetId) {
      return;
    }

    if (focusedTerminalTargetId === terminalTarget?.id) {
      if (isMobileLayout) {
        setActivePanel("terminal");
        scrollToPanel("terminal");
      }
      return;
    }

    setActiveTool("terminals");

    if (isMobileLayout) {
      setActivePanel("terminals");
      scrollToPanel("terminals");
    }

    const element = terminalCardElementsRef.current.get(focusedTerminalTargetId);
    element?.scrollIntoView({
      behavior: "smooth",
      block: "nearest",
      inline: "nearest",
    });
  }, [
    focusedTerminalTargetId,
    isMobileLayout,
    pageVisible,
    scrollToPanel,
    setActivePanel,
    terminalTarget,
  ]);

  useEffect(() => {
    if (!pageVisible) {
      return;
    }

    const handleKeyDown = (event: KeyboardEvent) => {
      if (
        !isPlatformModifier(event) ||
        event.altKey ||
        event.shiftKey ||
        (event.code !== "Digit1" && event.code !== "Digit2") ||
        (isEditableEventTarget(event.target) && !isXtermHelperTextarea(event.target))
      ) {
        return;
      }

      event.preventDefault();
      event.stopPropagation();
      setActiveTool(event.code === "Digit1" ? "dashboard" : "terminals");
    };

    window.addEventListener("keydown", handleKeyDown, true);

    return () => {
      window.removeEventListener("keydown", handleKeyDown, true);
    };
  }, [pageVisible]);

  useEffect(() => {
    if (!pageVisible || focusableTerminalTargetIds.length <= 1) {
      return;
    }

    const handleKeyDown = (event: KeyboardEvent) => {
      if (
        !isPlatformModifier(event) ||
        event.altKey ||
        event.shiftKey ||
        (event.key !== "ArrowLeft" && event.key !== "ArrowRight") ||
        (isEditableEventTarget(event.target) && !isXtermHelperTextarea(event.target))
      ) {
        return;
      }

      const currentIndex = focusableTerminalTargetIds.findIndex(
        (targetId) => targetId === focusedTerminalTargetId,
      );

      if (currentIndex === -1) {
        return;
      }

      const nextIndex =
        event.key === "ArrowLeft"
          ? (currentIndex + focusableTerminalTargetIds.length - 1) %
            focusableTerminalTargetIds.length
          : (currentIndex + 1) % focusableTerminalTargetIds.length;

      event.preventDefault();
      event.stopPropagation();
      setFocusedTerminalTargetId(focusableTerminalTargetIds[nextIndex]);
    };

    window.addEventListener("keydown", handleKeyDown, true);

    return () => {
      window.removeEventListener("keydown", handleKeyDown, true);
    };
  }, [focusableTerminalTargetIds, focusedTerminalTargetId, pageVisible]);

  return (
    <>
      {isMobileLayout ? (
        <header className="content-header content-header-mobile">
          <div className="content-header-mobile-topline">
            <div className="content-header-copy">
              <span className="panel-title">Machine</span>
              <div className="content-title-row">
                <h1 className="content-title content-title-container">{server.label}</h1>
                <span className="content-chip">{server.host}</span>
              </div>
            </div>

            <div className="content-header-mobile-actions">
              <Button
                aria-label={`Open ${server.label} settings`}
                onClick={onOpenSettings}
                size="icon"
                title="Machine settings"
                variant="ghost"
              >
                <Settings2 size={16} />
              </Button>
            </div>
          </div>

          <div aria-label="Machine panels" className="content-panel-rail">
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
              Host
            </Button>
            <Button
              className="content-panel-rail-button"
              isActive={activeMobilePanel === "dashboard"}
              onClick={() => {
                setActiveTool("dashboard");
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
              isActive={activeMobilePanel === "terminals"}
              onClick={() => {
                setActiveTool("terminals");
                setActivePanel("terminals");
                scrollToPanel("terminals");
              }}
              size="sm"
              variant={activeMobilePanel === "terminals" ? "default" : "outline"}
            >
              <SquareTerminal size={14} />
              Containers
            </Button>
          </div>
        </header>
      ) : (
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
      )}

      <section className="page-section">
        {isMobileLayout ? (
          <div className="mobile-workspace server-mobile-workspace">
            <div className="mobile-panel-track" onScroll={handleTrackScroll} ref={trackRef}>
              <div
                className="mobile-panel-slide"
                data-mobile-panel="terminal"
                ref={registerPanel("terminal")}
              >
                <article
                  className={`panel terminal-panel mobile-panel-card ${
                    focusedTerminalTargetId === terminalTarget?.id ? "is-grid-focused" : ""
                  }`}
                  onFocusCapture={() => {
                    if (terminalTarget) {
                      setFocusedTerminalTargetId(terminalTarget.id);
                    }
                  }}
                  onMouseDown={() => {
                    if (terminalTarget) {
                      setFocusedTerminalTargetId(terminalTarget.id);
                    }
                  }}
                >
                  {terminalTarget ? (
                    <TerminalPane
                      active={pageVisible && activeMobilePanel === "terminal"}
                      isFocused={focusedTerminalTargetId === terminalTarget.id}
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
                  activeFocusedTargetId={focusedTerminalTargetId}
                  onFocusTarget={setFocusedTerminalTargetId}
                  onOpenWorkspace={onOpenWorkspace}
                  pageVisible={pageVisible && activeMobilePanel === "terminals"}
                  registerTerminalElement={(targetId, element) => {
                    if (element) {
                      terminalCardElementsRef.current.set(targetId, element);
                    } else {
                      terminalCardElementsRef.current.delete(targetId);
                    }
                  }}
                  resolveTarget={resolveTarget}
                  server={server}
                />
              </div>
            </div>
          </div>
        ) : (
          <div className="server-workspace">
            <article
              className={`panel terminal-panel ${
                focusedTerminalTargetId === terminalTarget?.id ? "is-grid-focused" : ""
              }`}
              onFocusCapture={() => {
                if (terminalTarget) {
                  setFocusedTerminalTargetId(terminalTarget.id);
                }
              }}
              onMouseDown={() => {
                if (terminalTarget) {
                  setFocusedTerminalTargetId(terminalTarget.id);
                }
              }}
              ref={(element) => {
                if (element && terminalTarget) {
                  terminalCardElementsRef.current.set(terminalTarget.id, element);
                } else if (terminalTarget) {
                  terminalCardElementsRef.current.delete(terminalTarget.id);
                }
              }}
            >
              {terminalTarget ? (
                <TerminalPane
                  active={pageVisible}
                  isFocused={focusedTerminalTargetId === terminalTarget.id}
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
                activeFocusedTargetId={focusedTerminalTargetId}
                onFocusTarget={setFocusedTerminalTargetId}
                onOpenWorkspace={onOpenWorkspace}
                pageVisible={pageVisible && activeTool === "terminals"}
                registerTerminalElement={(targetId, element) => {
                  if (element) {
                    terminalCardElementsRef.current.set(targetId, element);
                  } else {
                    terminalCardElementsRef.current.delete(targetId);
                  }
                }}
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
