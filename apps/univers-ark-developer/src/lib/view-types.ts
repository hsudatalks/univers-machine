export type ActiveView =
  | { kind: "home" }
  | { kind: "settings" }
  | { kind: "machine"; machineId: string }
  | { kind: "container"; targetId: string };

function decodeRouteSegment(value: string): string {
  try {
    return decodeURIComponent(value);
  } catch {
    return value;
  }
}

export function parseActiveViewFromHash(hash: string): ActiveView | null {
  const normalized = hash.replace(/^#/, "").trim();

  if (!normalized) {
    return null;
  }

  const [route, ...rest] = normalized.replace(/^\/+/, "").split("/");

  switch (route) {
    case "home":
    case "dashboard":
    case "overview":
      return { kind: "home" };
    case "settings":
      return { kind: "settings" };
    case "machine": {
      const machineId = decodeRouteSegment(rest.join("/"));
      return machineId ? { kind: "machine", machineId } : null;
    }
    case "container": {
      const targetId = decodeRouteSegment(rest.join("/"));
      return targetId ? { kind: "container", targetId } : null;
    }
    default:
      return null;
  }
}

export function hashForActiveView(view: ActiveView): string {
  switch (view.kind) {
    case "home":
      return "#/home";
    case "settings":
      return "#/settings";
    case "machine":
      return `#/machine/${encodeURIComponent(view.machineId)}`;
    case "container":
      return `#/container/${encodeURIComponent(view.targetId)}`;
  }
}

export function sameActiveView(left: ActiveView, right: ActiveView): boolean {
  switch (left.kind) {
    case "home":
      return right.kind === "home";
    case "settings":
      return right.kind === "settings";
    case "machine":
      return right.kind === "machine" && left.machineId === right.machineId;
    case "container":
      return right.kind === "container" && left.targetId === right.targetId;
  }
}

export type ContainerToolPanel =
  | "dashboard"
  | "services"
  | "files"
  | `browser:${string}`;

export function isBrowserToolPanel(
  panel: ContainerToolPanel | null | undefined,
): panel is `browser:${string}` {
  return Boolean(panel?.startsWith("browser:"));
}

export function browserSurfaceIdFromPanel(
  panel: ContainerToolPanel | null | undefined,
): string | null {
  if (!isBrowserToolPanel(panel)) {
    return null;
  }

  return panel.slice("browser:".length) || null;
}
