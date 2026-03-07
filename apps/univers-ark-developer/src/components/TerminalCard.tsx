import { useEffect, useRef, useState, type CSSProperties } from "react";
import { TerminalPane } from "./TerminalPane";
import type { DeveloperTarget } from "../types";

interface TerminalCardProps {
  isGridFocused?: boolean;
  onFocusRequest?: () => void;
  onOpenWorkspace?: () => void;
  pageVisible?: boolean;
  registerElement?: (element: HTMLElement | null) => void;
  scale?: number;
  target: DeveloperTarget;
  title?: string;
}

const TERMINAL_VISIBILITY_ROOT_MARGIN = "320px 0px";

function WorkspaceButton({
  onClick,
}: {
  onClick: () => void;
}) {
  return (
    <button
      aria-label="Open workspace"
      className="panel-button panel-button-toolbar panel-button-icon"
      onClick={onClick}
      title="Open workspace"
      type="button"
    >
      <svg
        aria-hidden="true"
        className="panel-button-icon-svg"
        fill="none"
        viewBox="0 0 16 16"
      >
        <path
          d="M2.75 6V2.75H6M10 2.75h3.25V6M13.25 10v3.25H10M6 13.25H2.75V10"
          stroke="currentColor"
          strokeLinecap="round"
          strokeLinejoin="round"
          strokeWidth="1.25"
        />
      </svg>
    </button>
  );
}

export function TerminalCard({
  isGridFocused = false,
  onFocusRequest,
  onOpenWorkspace,
  pageVisible = true,
  registerElement,
  scale = 1,
  target,
  title,
}: TerminalCardProps) {
  const cardRef = useRef<HTMLElement | null>(null);
  const pageVisibleRef = useRef(pageVisible);
  const [isVisible, setIsVisible] = useState(
    () => typeof IntersectionObserver === "undefined",
  );
  const shouldRenderLiveTerminal = isVisible && pageVisible;
  const cardStyle = {
    "--terminal-card-scale": String(scale),
  } as CSSProperties;

  useEffect(() => {
    pageVisibleRef.current = pageVisible;
  }, [pageVisible]);

  useEffect(() => {
    const element = cardRef.current;

    registerElement?.(element);

    if (!element || typeof IntersectionObserver === "undefined") {
      return () => {
        registerElement?.(null);
      };
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
      registerElement?.(null);
    };
  }, [registerElement]);

  return (
    <article
      className={`panel terminal-card ${isGridFocused ? "is-grid-focused" : ""}`}
      onFocusCapture={() => {
        onFocusRequest?.();
      }}
      onMouseDown={() => {
        onFocusRequest?.();
      }}
      ref={cardRef}
      style={cardStyle}
    >
      {shouldRenderLiveTerminal ? (
        <TerminalPane
          active={pageVisible}
          actions={
            onOpenWorkspace ? <WorkspaceButton onClick={onOpenWorkspace} /> : undefined
          }
          autoFocus={false}
          fontScale={scale}
          isFocused={isGridFocused}
          target={target}
          title={title ?? target.label}
        />
      ) : (
        <>
          <header className="panel-header terminal-header terminal-header-compact">
            <div className="terminal-copy">
              <span className="panel-title">{title ?? target.label}</span>
            </div>

            <div className="terminal-meta">
              {onOpenWorkspace ? <WorkspaceButton onClick={onOpenWorkspace} /> : null}
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
