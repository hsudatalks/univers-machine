import { useCallback, useEffect, useRef, useState } from "react";

interface UseHorizontalPanelTrackOptions<T extends string> {
  enabled: boolean;
  initialPanel: T;
  panelIds: readonly T[];
}

export function useHorizontalPanelTrack<T extends string>({
  enabled,
  initialPanel,
  panelIds,
}: UseHorizontalPanelTrackOptions<T>) {
  const trackRef = useRef<HTMLDivElement | null>(null);
  const panelRefs = useRef(new Map<T, HTMLDivElement>());
  const frameRef = useRef<number | null>(null);
  const [activePanel, setActivePanel] = useState<T>(initialPanel);

  useEffect(() => {
    if (panelIds.includes(activePanel)) {
      return;
    }

    const fallback = panelIds[0] ?? initialPanel;

    if (fallback !== activePanel) {
      setActivePanel(fallback);
    }
  }, [activePanel, initialPanel, panelIds]);

  useEffect(() => {
    return () => {
      if (frameRef.current !== null) {
        window.cancelAnimationFrame(frameRef.current);
      }
    };
  }, []);

  const registerPanel = useCallback(
    (panelId: T) => (node: HTMLDivElement | null) => {
      if (node) {
        panelRefs.current.set(panelId, node);
        return;
      }

      panelRefs.current.delete(panelId);
    },
    [],
  );

  const scrollToPanel = useCallback(
    (panelId: T, behavior: ScrollBehavior = "smooth") => {
      const track = trackRef.current;
      const panel = panelRefs.current.get(panelId);

      if (!enabled || !track || !panel) {
        return;
      }

      track.scrollTo({
        left: panel.offsetLeft,
        behavior,
      });
      setActivePanel(panelId);
    },
    [enabled],
  );

  const updateActivePanel = useCallback(() => {
    const track = trackRef.current;

    if (!enabled || !track || panelIds.length === 0) {
      return;
    }

    const trackCenter = track.scrollLeft + track.clientWidth / 2;
    let nearestPanel = panelIds[0];
    let nearestDistance = Number.POSITIVE_INFINITY;

    for (const panelId of panelIds) {
      const panel = panelRefs.current.get(panelId);

      if (!panel) {
        continue;
      }

      const panelCenter = panel.offsetLeft + panel.clientWidth / 2;
      const distance = Math.abs(panelCenter - trackCenter);

      if (distance < nearestDistance) {
        nearestDistance = distance;
        nearestPanel = panelId;
      }
    }

    setActivePanel((current) => (current === nearestPanel ? current : nearestPanel));
  }, [enabled, panelIds]);

  const handleTrackScroll = useCallback(() => {
    if (!enabled) {
      return;
    }

    if (frameRef.current !== null) {
      window.cancelAnimationFrame(frameRef.current);
    }

    frameRef.current = window.requestAnimationFrame(() => {
      frameRef.current = null;
      updateActivePanel();
    });
  }, [enabled, updateActivePanel]);

  return {
    activePanel,
    handleTrackScroll,
    registerPanel,
    scrollToPanel,
    setActivePanel,
    trackRef,
  };
}
