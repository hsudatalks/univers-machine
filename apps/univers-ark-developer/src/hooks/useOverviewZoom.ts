import { useEffect, useState } from "react";

const OVERVIEW_ZOOM_STORAGE_KEY = "univers-ark-developer:overview-zoom";
export const OVERVIEW_ZOOM_MIN = 0.8;
export const OVERVIEW_ZOOM_MAX = 1.3;
export const OVERVIEW_ZOOM_STEP = 0.1;
export const OVERVIEW_ZOOM_DEFAULT = 1;

function clampOverviewZoom(value: number): number {
  return Math.min(OVERVIEW_ZOOM_MAX, Math.max(OVERVIEW_ZOOM_MIN, value));
}

function roundOverviewZoom(value: number): number {
  return Math.round(value * 10) / 10;
}

export function useOverviewZoom() {
  const [overviewZoom, setOverviewZoom] = useState(() => {
    if (typeof window === "undefined") {
      return OVERVIEW_ZOOM_DEFAULT;
    }

    const stored = window.localStorage.getItem(OVERVIEW_ZOOM_STORAGE_KEY);
    const parsed = stored ? Number(stored) : Number.NaN;

    if (!Number.isFinite(parsed)) {
      return OVERVIEW_ZOOM_DEFAULT;
    }

    return clampOverviewZoom(parsed);
  });

  useEffect(() => {
    if (typeof window === "undefined") {
      return;
    }

    window.localStorage.setItem(
      OVERVIEW_ZOOM_STORAGE_KEY,
      String(roundOverviewZoom(overviewZoom)),
    );
  }, [overviewZoom]);

  return {
    overviewZoom,
    setOverviewZoom,
    clampOverviewZoom,
    roundOverviewZoom,
  };
}
