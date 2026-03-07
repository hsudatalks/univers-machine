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

export interface AppBootstrap {
  appName: string;
  configPath: string;
  selectedTargetId: string | null;
  targets: DeveloperTarget[];
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
