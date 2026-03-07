export interface DeveloperSurface {
  id: string;
  label: string;
  tunnelCommand: string;
  localUrl: string;
  remoteUrl: string;
  viteHmrTunnelCommand?: string;
}

export interface DeveloperTarget {
  id: string;
  label: string;
  host: string;
  description: string;
  terminalCommand: string;
  notes: string[];
  surfaces: DeveloperSurface[];
}

export interface ManagedContainer {
  serverId: string;
  serverLabel: string;
  targetId: string;
  name: string;
  label: string;
  status: string;
  ipv4: string;
  sshUser: string;
  sshDestination: string;
  sshCommand: string;
  sshState: string;
  sshMessage: string;
  sshReachable: boolean;
}

export interface ManagedServer {
  id: string;
  label: string;
  host: string;
  description: string;
  state: string;
  message: string;
  containers: ManagedContainer[];
}

export interface RemoteFileEntry {
  name: string;
  path: string;
  kind: string;
  size: number;
  isHidden: boolean;
}

export interface RemoteDirectoryListing {
  targetId: string;
  path: string;
  parentPath: string | null;
  entries: RemoteFileEntry[];
}

export interface RemoteFilePreview {
  targetId: string;
  path: string;
  content: string;
  isBinary: boolean;
  truncated: boolean;
}

export interface AppBootstrap {
  appName: string;
  configPath: string;
  selectedTargetId: string | null;
  targets: DeveloperTarget[];
  servers: ManagedServer[];
}

export interface TerminalSnapshot {
  targetId: string;
  output: string;
}

export interface TerminalOutputEvent {
  targetId: string;
  data: string;
}

export interface TerminalExitEvent {
  targetId: string;
  reason: string;
}

export interface TunnelStatus {
  targetId: string;
  surfaceId: string;
  state: string;
  message: string;
}
