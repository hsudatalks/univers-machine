import { useEffect, useEffectEvent, useMemo, useRef, useState } from "react";

const IS_MAC = navigator.platform.toUpperCase().includes("MAC");

function isPlatformModifier(event: KeyboardEvent): boolean {
  return IS_MAC ? event.metaKey && !event.ctrlKey : event.ctrlKey && !event.metaKey;
}

type OverviewMoveDirection = "left" | "right" | "up" | "down";

function adjacentOverviewTargetId(
  direction: OverviewMoveDirection,
  currentTargetId: string,
  targetIds: string[],
  elements: Map<string, HTMLElement>,
): string {
  if (direction === "left" || direction === "right") {
    const currentIndex = targetIds.indexOf(currentTargetId);

    if (currentIndex === -1) {
      return targetIds[0] ?? currentTargetId;
    }

    const nextIndex =
      direction === "left" ? currentIndex - 1 : currentIndex + 1;

    return targetIds[nextIndex] ?? currentTargetId;
  }

  const cards = targetIds
    .map((targetId) => {
      const element = elements.get(targetId);

      if (!element) {
        return null;
      }

      const rect = element.getBoundingClientRect();

      return {
        centerX: rect.left + rect.width / 2,
        centerY: rect.top + rect.height / 2,
        id: targetId,
      };
    })
    .filter(
      (
        card,
      ): card is {
        centerX: number;
        centerY: number;
        id: string;
      } => Boolean(card),
    );

  if (cards.length === 0) {
    return currentTargetId;
  }

  const currentCard = cards.find((card) => card.id === currentTargetId) ?? cards[0];
  const candidates = cards.filter((card) => {
    if (direction === "up") {
      return card.centerY < currentCard.centerY - 4;
    }

    return card.centerY > currentCard.centerY + 4;
  });

  if (candidates.length === 0) {
    return currentCard.id;
  }

  const perpendicularWeight = 4;

  const bestCandidate = candidates.reduce((best, candidate) => {
    const axisDistance =
      Math.abs(candidate.centerY - currentCard.centerY);
    const perpendicularDistance =
      Math.abs(candidate.centerX - currentCard.centerX);
    const score = axisDistance + perpendicularDistance * perpendicularWeight;

    if (!best || score < best.score) {
      return {
        id: candidate.id,
        score,
      };
    }

    return best;
  }, null as { id: string; score: number } | null);

  return bestCandidate?.id ?? currentCard.id;
}

interface UseOverviewNavigationOptions {
  activeViewKind: "overview" | "server" | "container";
  onOpenWorkspace: (targetId: string) => void;
  targetIds: string[];
}

export function useOverviewNavigation({
  activeViewKind,
  onOpenWorkspace,
  targetIds,
}: UseOverviewNavigationOptions) {
  const [overviewFocusedTargetId, setOverviewFocusedTargetId] = useState("");
  const overviewCardElementsRef = useRef(new Map<string, HTMLElement>());
  const activeOverviewFocusedTargetId = useMemo(() => {
    if (targetIds.length === 0) {
      return "";
    }

    if (
      overviewFocusedTargetId &&
      targetIds.includes(overviewFocusedTargetId)
    ) {
      return overviewFocusedTargetId;
    }

    return targetIds[0] ?? "";
  }, [overviewFocusedTargetId, targetIds]);
  const openContainerViewFromShortcut = useEffectEvent(onOpenWorkspace);

  useEffect(() => {
    if (activeViewKind !== "overview" || !activeOverviewFocusedTargetId) {
      return;
    }

    const element = overviewCardElementsRef.current.get(activeOverviewFocusedTargetId);

    if (!element) {
      return;
    }

    element.scrollIntoView({
      behavior: "smooth",
      block: "nearest",
      inline: "nearest",
    });
  }, [activeOverviewFocusedTargetId, activeViewKind]);

  useEffect(() => {
    if (activeViewKind !== "overview") {
      return;
    }

    const fallbackTargetId = targetIds[0];

    const handleKeyDown = (event: KeyboardEvent) => {
      if (!isPlatformModifier(event) || event.altKey || event.shiftKey) {
        return;
      }

      const currentTargetId = activeOverviewFocusedTargetId || fallbackTargetId;

      if (event.key === "Enter") {
        if (!currentTargetId) {
          return;
        }

        event.preventDefault();
        event.stopPropagation();
        openContainerViewFromShortcut(currentTargetId);
        return;
      }

      let direction: OverviewMoveDirection | null = null;

      switch (event.key) {
        case "ArrowLeft":
          direction = "left";
          break;
        case "ArrowRight":
          direction = "right";
          break;
        case "ArrowUp":
          direction = "up";
          break;
        case "ArrowDown":
          direction = "down";
          break;
        default:
          return;
      }

      if (!direction || !currentTargetId) {
        return;
      }

      const nextTargetId = adjacentOverviewTargetId(
        direction,
        currentTargetId,
        targetIds,
        overviewCardElementsRef.current,
      );

      if (!nextTargetId || nextTargetId === currentTargetId) {
        return;
      }

      event.preventDefault();
      event.stopPropagation();
      setOverviewFocusedTargetId(nextTargetId);
    };

    window.addEventListener("keydown", handleKeyDown, true);

    return () => {
      window.removeEventListener("keydown", handleKeyDown, true);
    };
  }, [activeOverviewFocusedTargetId, activeViewKind, targetIds]);

  return {
    activeOverviewFocusedTargetId,
    registerOverviewCardElement(targetId: string, element: HTMLElement | null) {
      if (element) {
        overviewCardElementsRef.current.set(targetId, element);
      } else {
        overviewCardElementsRef.current.delete(targetId);
      }
    },
    setOverviewFocusedTargetId,
  };
}
