import { useEffect } from "react";
import { openContainerCompanionWindow } from "../lib/tauri";
import type { HomeViewMode } from "../hooks/useOrchestrationViewMode";
import type { DeveloperTarget, ManagedMachine } from "../types";
import type { ActiveView } from "../lib/view-types";

const IS_MAC = navigator.platform.toUpperCase().includes("MAC");
const HOME_VIEW_MODES: HomeViewMode[] = [
  "dashboard",
  "machines",
  "grid",
  "focus",
];

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

interface UseGlobalShortcutsOptions {
  activeFocusCompanionTarget: Pick<DeveloperTarget, "id" | "label"> | null;
  activeView: ActiveView;
  homeViewMode: HomeViewMode;
  isCompanionWindow: boolean;
  isCompactHomeLayout: boolean;
  machines: ManagedMachine[];
  onOpenMachine: (machineId: string) => void;
  onSetHomeViewMode: (mode: HomeViewMode) => void;
  onToggleSettings: () => void;
}

export function useGlobalShortcuts({
  activeFocusCompanionTarget,
  activeView,
  homeViewMode,
  isCompanionWindow,
  isCompactHomeLayout,
  machines,
  onOpenMachine,
  onSetHomeViewMode,
  onToggleSettings,
}: UseGlobalShortcutsOptions) {
  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      if (
        !isPlatformModifier(event) ||
        event.altKey ||
        event.shiftKey ||
        event.code !== "KeyS" ||
        isEditableEventTarget(event.target)
      ) {
        return;
      }

      event.preventDefault();
      event.stopPropagation();
      onToggleSettings();
    };

    window.addEventListener("keydown", handleKeyDown, true);

    return () => {
      window.removeEventListener("keydown", handleKeyDown, true);
    };
  }, [onToggleSettings]);

  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      if (
        activeView.kind !== "home" ||
        !isPlatformModifier(event) ||
        event.altKey ||
        (isCompactHomeLayout
          ? event.code !== "Digit1"
          : event.key !== "Tab" &&
            event.code !== "Digit1" &&
            event.code !== "Digit2" &&
            event.code !== "Digit3" &&
            event.code !== "Digit4") ||
        (isEditableEventTarget(event.target) && !isXtermHelperTextarea(event.target))
      ) {
        return;
      }

      event.preventDefault();
      event.stopPropagation();

      if (event.code === "Digit1" && !event.shiftKey) {
        onSetHomeViewMode("dashboard");
        return;
      }

      if (isCompactHomeLayout) {
        return;
      }

      if (event.code === "Digit2" && !event.shiftKey) {
        onSetHomeViewMode("machines");
        return;
      }

      if (event.code === "Digit3" && !event.shiftKey) {
        onSetHomeViewMode("grid");
        return;
      }

      if (event.code === "Digit4" && !event.shiftKey) {
        onSetHomeViewMode("focus");
        return;
      }

      if (event.key !== "Tab") {
        return;
      }

      const currentIndex = HOME_VIEW_MODES.indexOf(homeViewMode);
      const nextIndex =
        currentIndex === -1
          ? 0
          : (currentIndex +
              (event.shiftKey ? HOME_VIEW_MODES.length - 1 : 1)) %
            HOME_VIEW_MODES.length;

      onSetHomeViewMode(HOME_VIEW_MODES[nextIndex]);
    };

    window.addEventListener("keydown", handleKeyDown, true);

    return () => {
      window.removeEventListener("keydown", handleKeyDown, true);
    };
  }, [activeView.kind, homeViewMode, isCompactHomeLayout, onSetHomeViewMode]);

  useEffect(() => {
    if (isCompanionWindow) {
      return;
    }

    const handleKeyDown = (event: KeyboardEvent) => {
      if (
        activeView.kind !== "home" ||
        homeViewMode !== "focus" ||
        !isPlatformModifier(event) ||
        !event.altKey ||
        event.shiftKey ||
        event.code !== "KeyO" ||
        (isEditableEventTarget(event.target) && !isXtermHelperTextarea(event.target))
      ) {
        return;
      }

      if (!activeFocusCompanionTarget) {
        return;
      }

      event.preventDefault();
      event.stopPropagation();
      void openContainerCompanionWindow(
        activeFocusCompanionTarget.id,
        activeFocusCompanionTarget.label,
      ).catch(() => undefined);
    };

    window.addEventListener("keydown", handleKeyDown, true);

    return () => {
      window.removeEventListener("keydown", handleKeyDown, true);
    };
  }, [
    activeFocusCompanionTarget,
    activeView.kind,
    homeViewMode,
    isCompanionWindow,
  ]);

  useEffect(() => {
    if (activeView.kind !== "machine" || machines.length <= 1) {
      return;
    }

    const handleKeyDown = (event: KeyboardEvent) => {
      if (
        !isPlatformModifier(event) ||
        !event.altKey ||
        event.shiftKey ||
        (event.key !== "ArrowLeft" && event.key !== "ArrowRight") ||
        (isEditableEventTarget(event.target) && !isXtermHelperTextarea(event.target))
      ) {
        return;
      }

      const currentIndex = machines.findIndex(
        (machine) => machine.id === activeView.machineId,
      );

      if (currentIndex === -1) {
        return;
      }

      const nextIndex =
        event.key === "ArrowLeft"
          ? (currentIndex + machines.length - 1) % machines.length
          : (currentIndex + 1) % machines.length;
      const nextMachine = machines[nextIndex];

      event.preventDefault();
      event.stopPropagation();
      onOpenMachine(nextMachine.id);
    };

    window.addEventListener("keydown", handleKeyDown, true);

    return () => {
      window.removeEventListener("keydown", handleKeyDown, true);
    };
  }, [activeView, machines, onOpenMachine]);
}
