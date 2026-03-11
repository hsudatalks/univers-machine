import { useEffect, useMemo, useRef, useState, type CSSProperties } from "react";
import type { HomeViewMode } from "../hooks/useOrchestrationViewMode";
import { connectionStatusClass } from "../lib/connectivity-state";
import { TerminalCard } from "./TerminalCard";
import type { DeveloperTarget, ManagedContainer, ManagedMachine } from "../types";

interface OverviewEntry {
  container: ManagedContainer;
  machine: ManagedMachine;
  target?: DeveloperTarget;
}

interface OverviewPageProps {
  activeFocusedTargetId: string;
  isRefreshing: boolean;
  onFocusTarget: (targetId: string) => void;
  onOpenWorkspace: (targetId: string) => void;
  onRefreshInventory: () => void;
  homeViewMode: Exclude<HomeViewMode, "dashboard">;
  overviewContainers: OverviewEntry[];
  overviewZoom: number;
  overviewZoomStyle: CSSProperties;
  pageVisible: boolean;
  registerOverviewCardElement: (targetId: string, element: HTMLElement | null) => void;
  standaloneTargets: DeveloperTarget[];
}

interface OverviewCardEntry {
  container?: ManagedContainer;
  key: string;
  target?: DeveloperTarget;
  title: string;
}

function UnavailableTerminalCard({
  container,
  isRefreshing,
  onRetry,
}: {
  container: ManagedContainer;
  isRefreshing: boolean;
  onRetry: () => void;
}) {
  return (
    <article className="panel terminal-card terminal-card-unavailable">
      <header className="panel-header terminal-placeholder-header">
        <div className="terminal-copy">
          <span className="panel-title">{container.label}</span>
        </div>

        <div className="terminal-meta">
          <span className={`terminal-status ${connectionStatusClass(container.sshState)}`}>
            {container.sshState}
          </span>
        </div>
      </header>

      <div className="terminal-placeholder-body">
        <p className="terminal-placeholder-copy">{container.sshMessage}</p>
        <button
          className="panel-button panel-button-retry"
          disabled={isRefreshing}
          onClick={onRetry}
          type="button"
        >
          {isRefreshing ? "Retrying…" : "Retry"}
        </button>
      </div>
    </article>
  );
}

export function OverviewPage({
  activeFocusedTargetId,
  isRefreshing,
  onFocusTarget,
  onOpenWorkspace,
  onRefreshInventory,
  homeViewMode,
  overviewContainers,
  overviewZoom,
  overviewZoomStyle,
  pageVisible,
  registerOverviewCardElement,
  standaloneTargets,
}: OverviewPageProps) {
  const [compactSide, setCompactSide] = useState<"left" | "right">("right");
  const previousFocusedTargetOrderIndexRef = useRef<number | null>(null);
  const overviewEntries = useMemo<OverviewCardEntry[]>(
    () => [
      ...overviewContainers.map(({ container, target }) => ({
        container,
        key: container.targetId,
        target,
        title: container.label,
      })),
      ...standaloneTargets.map((target) => ({
        key: target.id,
        target,
        title: target.label,
      })),
    ],
    [overviewContainers, standaloneTargets],
  );
  const focusableEntries = useMemo(
    () =>
      overviewEntries.filter(
        (entry): entry is OverviewCardEntry & { target: DeveloperTarget } =>
          Boolean(entry.target),
      ),
    [overviewEntries],
  );
  const focusedEntry =
    focusableEntries.find((entry) => entry.target.id === activeFocusedTargetId) ??
    focusableEntries[0];
  const focusedEntryIndex = focusedEntry
    ? overviewEntries.findIndex((entry) => entry.target?.id === focusedEntry.target.id)
    : -1;
  const focusedTargetOrderIndex = focusedEntry
    ? focusableEntries.findIndex((entry) => entry.target.id === focusedEntry.target.id)
    : -1;
  const surroundingEntryCount = Math.max(overviewEntries.length - 1, 0);
  const leftCount = Math.floor(surroundingEntryCount / 2);
  const rightCount = surroundingEntryCount - leftCount;
  const leftEntries = Array.from({ length: leftCount }, (_, index) => {
    if (focusedEntryIndex < 0 || overviewEntries.length === 0) {
      return undefined;
    }

    const nextIndex =
      (focusedEntryIndex - (index + 1) + overviewEntries.length) % overviewEntries.length;

    return overviewEntries[nextIndex];
  }).filter((entry): entry is OverviewCardEntry => Boolean(entry));
  const rightEntries = Array.from({ length: rightCount }, (_, index) => {
    if (focusedEntryIndex < 0 || overviewEntries.length === 0) {
      return undefined;
    }

    const nextIndex = (focusedEntryIndex + index + 1) % overviewEntries.length;

    return overviewEntries[nextIndex];
  }).filter((entry): entry is OverviewCardEntry => Boolean(entry));
  const effectiveCompactSide =
    compactSide === "left"
      ? leftEntries.length > 0
        ? "left"
        : "right"
      : rightEntries.length > 0
        ? "right"
        : "left";
  const sideScale = Math.max(0.82, Number((overviewZoom - 0.15).toFixed(2)));
  const focusScale = Math.min(1.45, Math.max(1.08, Number((overviewZoom + 0.18).toFixed(2))));

  useEffect(() => {
    if (homeViewMode !== "focus" || focusedTargetOrderIndex < 0) {
      previousFocusedTargetOrderIndexRef.current =
        focusedTargetOrderIndex >= 0 ? focusedTargetOrderIndex : null;
      return;
    }

    const previousFocusedTargetOrderIndex = previousFocusedTargetOrderIndexRef.current;

    if (
      previousFocusedTargetOrderIndex !== null &&
      previousFocusedTargetOrderIndex !== focusedTargetOrderIndex &&
      focusableEntries.length > 1
    ) {
      const totalTargets = focusableEntries.length;
      const forwardDistance =
        (focusedTargetOrderIndex - previousFocusedTargetOrderIndex + totalTargets) %
        totalTargets;
      const backwardDistance =
        (previousFocusedTargetOrderIndex - focusedTargetOrderIndex + totalTargets) %
        totalTargets;

      setCompactSide(forwardDistance <= backwardDistance ? "right" : "left");
    }

    previousFocusedTargetOrderIndexRef.current = focusedTargetOrderIndex;
  }, [focusableEntries.length, focusedTargetOrderIndex, homeViewMode]);

  const renderOverviewCard = (
    entry: OverviewCardEntry,
    options?: {
      isFocused?: boolean;
      scale?: number;
    },
  ) => {
    if (entry.target) {
      const targetId = entry.target.id;

      return (
        <TerminalCard
          isGridFocused={Boolean(options?.isFocused)}
          key={entry.key}
          onFocusRequest={() => {
            onFocusTarget(targetId);
          }}
          onOpenWorkspace={() => {
            onOpenWorkspace(targetId);
          }}
          pageVisible={pageVisible}
          registerElement={(element) => {
            registerOverviewCardElement(targetId, element);
          }}
          scale={options?.scale ?? overviewZoom}
          target={entry.target}
          title={entry.title}
        />
      );
    }

    if (!entry.container) {
      return null;
    }

    return (
      <UnavailableTerminalCard
        container={entry.container}
        isRefreshing={isRefreshing}
        key={entry.key}
        onRetry={onRefreshInventory}
      />
    );
  };

  if (homeViewMode === "focus" && focusedEntry) {
    return (
      <section className="page-section">
        <div
          className={`overview-focus-layout ${
            effectiveCompactSide === "left" ? "is-compact-left" : "is-compact-right"
          }`}
          style={overviewZoomStyle}
        >
          {leftEntries.length > 0 ? (
            <div className="overview-focus-column overview-focus-column-left">
              {leftEntries.map((entry) =>
                renderOverviewCard(entry, {
                  scale: sideScale,
                }),
              )}
            </div>
          ) : null}

          <div className="overview-focus-main">
            {renderOverviewCard(focusedEntry, {
              isFocused: true,
              scale: focusScale,
            })}
          </div>

          {rightEntries.length > 0 ? (
            <div className="overview-focus-column overview-focus-column-right">
              {rightEntries.map((entry) =>
                renderOverviewCard(entry, {
                  scale: sideScale,
                }),
              )}
            </div>
          ) : null}
        </div>
      </section>
    );
  }

  return (
    <section className="page-section">
      <div className="terminal-grid" style={overviewZoomStyle}>
        {overviewEntries.map((entry) =>
          renderOverviewCard(entry, {
            isFocused: entry.target?.id === activeFocusedTargetId,
          }),
        )}
      </div>
    </section>
  );
}
