import { useEffect, useRef, useState } from "react";
import { TerminalPane } from "./TerminalPane";
import type { DeveloperTarget } from "../types";

interface TerminalCardProps {
  meta?: string;
  onOpenWorkspace?: () => void;
  pageVisible?: boolean;
  target: DeveloperTarget;
  title?: string;
}

const TERMINAL_VISIBILITY_ROOT_MARGIN = "320px 0px";

export function TerminalCard({
  meta,
  onOpenWorkspace,
  pageVisible = true,
  target,
  title,
}: TerminalCardProps) {
  const cardRef = useRef<HTMLElement | null>(null);
  const pageVisibleRef = useRef(pageVisible);
  const [isVisible, setIsVisible] = useState(
    () => typeof IntersectionObserver === "undefined",
  );
  const shouldRenderLiveTerminal = isVisible && pageVisible;

  useEffect(() => {
    pageVisibleRef.current = pageVisible;
  }, [pageVisible]);

  useEffect(() => {
    const element = cardRef.current;

    if (!element || typeof IntersectionObserver === "undefined") {
      return undefined;
    }

    const observer = new IntersectionObserver(
      ([entry]) => {
        if (entry?.isIntersecting) {
          setIsVisible(true);
          return;
        }

        if (pageVisibleRef.current) {
          setIsVisible(false);
        }
      },
      {
        rootMargin: TERMINAL_VISIBILITY_ROOT_MARGIN,
        threshold: 0,
      },
    );

    observer.observe(element);

    return () => {
      observer.disconnect();
    };
  }, []);

  return (
    <article className="panel terminal-card" ref={cardRef}>
      {shouldRenderLiveTerminal ? (
        <TerminalPane
          active={pageVisible}
          actions={
            onOpenWorkspace ? (
              <button
                className="panel-button"
                onClick={onOpenWorkspace}
                type="button"
              >
                Workspace
              </button>
            ) : undefined
          }
          autoFocus={false}
          meta={meta}
          target={target}
          title={title ?? target.label}
        />
      ) : (
        <>
          <header className="panel-header terminal-header terminal-header-compact">
            <div className="terminal-copy">
              <span className="panel-title">{title ?? target.label}</span>
              <span className="panel-meta">{meta ?? target.host}</span>
            </div>

            <div className="terminal-meta">
              {onOpenWorkspace ? (
                <button
                  className="panel-button"
                  onClick={onOpenWorkspace}
                  type="button"
                >
                  Workspace
                </button>
              ) : null}
              <span className="terminal-status status-starting">Standby</span>
            </div>
          </header>

          <div className="terminal-placeholder-body terminal-placeholder-body-quiet">
            <p className="terminal-placeholder-copy">
              {pageVisible
                ? "This terminal attaches when the card scrolls into view."
                : "This terminal stays warm while the page is hidden."}
            </p>
          </div>
        </>
      )}
    </article>
  );
}
