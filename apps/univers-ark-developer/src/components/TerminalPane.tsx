import { useEffect, useMemo, useRef, useSyncExternalStore, type ReactNode } from "react";
import {
  claimTerminalSession,
  fitClaimedTerminalSession,
  focusClaimedTerminalSession,
  getTerminalStatus,
  restartTerminalSession,
  releaseTerminalSession,
  subscribeTerminalStatus,
} from "../lib/terminal-cache";
import type { DeveloperTarget } from "../types";

interface TerminalPaneProps {
  active?: boolean;
  actions?: ReactNode;
  autoFocus?: boolean;
  fontScale?: number;
  isFocused?: boolean;
  target: DeveloperTarget;
  title?: string;
}

const DEFAULT_TERMINAL_SCROLLBACK = 1500;
const TMUX_TERMINAL_SCROLLBACK = 0;

function preferredTerminalScrollback(target: DeveloperTarget): number {
  const startupCommand = target.terminalStartupCommand?.trim() ?? "";
  const terminalCommand = target.terminalCommand.trim();
  const commandText = `${startupCommand}\n${terminalCommand}`;

  return commandText.includes("tmux-mobile-view attach")
    ? TMUX_TERMINAL_SCROLLBACK
    : DEFAULT_TERMINAL_SCROLLBACK;
}

export function TerminalPane({
  active = true,
  actions,
  autoFocus = true,
  fontScale = 1,
  isFocused = false,
  target,
  title = "Terminal",
}: TerminalPaneProps) {
  const mountRef = useRef<HTMLDivElement | null>(null);
  const ownerId = useMemo(() => Symbol(target.id), [target.id]);
  const status = useSyncExternalStore(
    (listener) => subscribeTerminalStatus(target.id, listener),
    () => getTerminalStatus(target.id),
    () => "Connecting",
  );
  const compactStatus = status === "Connected" || status === "Running";

  useEffect(() => {
    const mountElement = mountRef.current;

    if (!mountElement || !active) {
      return undefined;
    }

    claimTerminalSession(target.id, ownerId, mountElement, {
      autoFocus,
      fontScale,
      scrollback: preferredTerminalScrollback(target),
    });

    const syncLayout = () => {
      fitClaimedTerminalSession(target.id, ownerId);
    };

    const resizeObserver = new ResizeObserver(() => {
      syncLayout();
    });

    resizeObserver.observe(mountElement);

    window.addEventListener("resize", syncLayout);
    syncLayout();

    if (autoFocus) {
      focusClaimedTerminalSession(target.id, ownerId);
    }

    return () => {
      resizeObserver.disconnect();
      window.removeEventListener("resize", syncLayout);
      releaseTerminalSession(target.id, ownerId);
    };
  }, [active, autoFocus, fontScale, ownerId, target.id]);

  useEffect(() => {
    if (!active || !isFocused) {
      return;
    }

    focusClaimedTerminalSession(target.id, ownerId);
  }, [active, isFocused, ownerId, target.id]);

  return (
    <>
      <header className="panel-header terminal-header">
        <div className="terminal-copy">
          <span className="panel-title">{title}</span>
        </div>

        <div className="terminal-meta">
          {actions}
          {compactStatus ? (
            <span
              aria-label={status}
              className={`terminal-status terminal-status-dot status-${status.toLowerCase()}`}
              title={status}
            />
          ) : (
            <span className={`terminal-status status-${status.toLowerCase()}`}>
              {status}
            </span>
          )}
          <button
            aria-label="Reconnect terminal"
            className="panel-button panel-button-toolbar panel-button-icon"
            onClick={() => {
              void restartTerminalSession(target.id);
            }}
            title="Reconnect terminal"
            type="button"
          >
            <svg
              aria-hidden="true"
              className="panel-button-icon-svg"
              fill="none"
              viewBox="0 0 16 16"
            >
              <path
                d="M13.25 8A5.25 5.25 0 1 1 11.7 4.29"
                stroke="currentColor"
                strokeLinecap="round"
                strokeLinejoin="round"
                strokeWidth="1.25"
              />
              <path
                d="M10.75 2.75h2.5v2.5"
                stroke="currentColor"
                strokeLinecap="round"
                strokeLinejoin="round"
                strokeWidth="1.25"
              />
            </svg>
          </button>
        </div>
      </header>

      <div
        className="terminal-mount"
        onClick={() => {
          if (active) {
            focusClaimedTerminalSession(target.id, ownerId);
          }
        }}
        ref={mountRef}
      />
    </>
  );
}
