import type { DeveloperTarget, ManagedServer } from "../types";

export const SERVER_HOST_TARGET_PREFIX = "server-host::";

export function serverHostTargetId(serverId: string): string {
  return `${SERVER_HOST_TARGET_PREFIX}${serverId}`;
}

export function serverIdFromHostTargetId(targetId: string): string | null {
  return targetId.startsWith(SERVER_HOST_TARGET_PREFIX)
    ? targetId.slice(SERVER_HOST_TARGET_PREFIX.length)
    : null;
}

export function isServerHostTargetId(targetId: string): boolean {
  return targetId.startsWith(SERVER_HOST_TARGET_PREFIX);
}

export function serverHostTarget(server: ManagedServer): DeveloperTarget {
  return {
    id: serverHostTargetId(server.id),
    label: `${server.label} host`,
    host: server.host,
    description: server.description || `Interactive shell on ${server.host}.`,
    terminalCommand: `ssh ${server.host}`,
    terminalStartupCommand: "",
    notes: [],
    workspace: {
      profile: "",
      defaultTool: "dashboard",
      projectPath: "",
      filesRoot: "",
      primaryWebServiceId: "",
      primaryBrowserServiceId: "",
      tmuxCommandServiceId: "",
    },
    services: [],
    surfaces: [],
  };
}
