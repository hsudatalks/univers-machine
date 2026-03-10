export type ActiveView =
  | { kind: "dashboard" }
  | { kind: "overview" }
  | { kind: "settings" }
  | { kind: "server"; serverId: string }
  | { kind: "container"; targetId: string };

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
