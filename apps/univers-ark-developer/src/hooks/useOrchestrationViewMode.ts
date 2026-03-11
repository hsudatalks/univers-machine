import { useEffect, useState } from "react";

export type OrchestrationViewMode = "grid" | "focus";

const ORCHESTRATION_VIEW_MODE_STORAGE_KEY =
  "univers-ark-developer:orchestration-view-mode";

function isOrchestrationViewMode(value: string | null): value is OrchestrationViewMode {
  return value === "grid" || value === "focus";
}

export function useOrchestrationViewMode() {
  const [orchestrationViewMode, setOrchestrationViewMode] =
    useState<OrchestrationViewMode>(() => {
      if (typeof window === "undefined") {
        return "grid";
      }

      const stored = window.localStorage.getItem(
        ORCHESTRATION_VIEW_MODE_STORAGE_KEY,
      );

      return isOrchestrationViewMode(stored) ? stored : "grid";
    });

  useEffect(() => {
    if (typeof window === "undefined") {
      return;
    }

    window.localStorage.setItem(
      ORCHESTRATION_VIEW_MODE_STORAGE_KEY,
      orchestrationViewMode,
    );
  }, [orchestrationViewMode]);

  return {
    orchestrationViewMode,
    setOrchestrationViewMode,
  };
}
