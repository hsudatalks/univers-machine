import { useEffect, useState } from "react";

export type HomeViewMode = "dashboard" | "machines" | "grid" | "focus";

const HOME_VIEW_MODE_STORAGE_KEY = "univers-ark-developer:home-view-mode";
const LEGACY_ORCHESTRATION_VIEW_MODE_STORAGE_KEY =
  "univers-ark-developer:orchestration-view-mode";

function isHomeViewMode(value: string | null): value is HomeViewMode {
  return (
    value === "dashboard" ||
    value === "machines" ||
    value === "grid" ||
    value === "focus"
  );
}

export function useHomeViewMode() {
  const [homeViewMode, setHomeViewMode] = useState<HomeViewMode>(() => {
      if (typeof window === "undefined") {
        return "dashboard";
      }

      const stored = window.localStorage.getItem(HOME_VIEW_MODE_STORAGE_KEY);

      if (isHomeViewMode(stored)) {
        return stored;
      }

      const legacyStored = window.localStorage.getItem(
        LEGACY_ORCHESTRATION_VIEW_MODE_STORAGE_KEY,
      );

      return isHomeViewMode(legacyStored) ? legacyStored : "dashboard";
    });

  useEffect(() => {
    if (typeof window === "undefined") {
      return;
    }

    window.localStorage.setItem(HOME_VIEW_MODE_STORAGE_KEY, homeViewMode);
  }, [homeViewMode]);

  return {
    homeViewMode,
    setHomeViewMode,
  };
}
