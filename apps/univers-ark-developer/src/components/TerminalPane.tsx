import { useEffect, useMemo, useRef, useSyncExternalStore, type ReactNode } from "react";
import {
  claimTerminalSession,
  fitClaimedTerminalSession,
  focusClaimedTerminalSession,
  getTerminalStatus,
  releaseTerminalSession,
  subscribeTerminalStatus,
} from "../lib/terminal-cache";
import type { DeveloperTarget } from "../types";

interface TerminalPaneProps {
  active?: boolean;
  actions?: ReactNode;
  autoFocus?: boolean;
  meta?: string;
  target: DeveloperTarget;
  title?: string;
}

export function TerminalPane({
  active = true,
  actions,
  autoFocus = true,
  meta,
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

  useEffect(() => {
    const mountElement = mountRef.current;

    if (!mountElement || !active) {
      return undefined;
    }

    claimTerminalSession(target.id, ownerId, mountElement, { autoFocus });

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
  }, [active, autoFocus, ownerId, target.id]);

  return (
    <>
      <header className="panel-header terminal-header">
        <div className="terminal-copy">
          <span className="panel-title">{title}</span>
          <span className="panel-meta">{meta ?? target.host}</span>
        </div>

        <div className="terminal-meta">
          {actions}
          <span className={`terminal-status status-${status.toLowerCase()}`}>
            {status}
          </span>
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
