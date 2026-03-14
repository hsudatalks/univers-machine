declare module "react-vnc" {
  import * as React from "react";

  export interface VncScreenHandle {
    disconnect(): void;
    sendCredentials(credentials: Record<string, string>): void;
  }

  export interface VncScreenProps {
    url: string;
    scaleViewport?: boolean;
    resizeSession?: boolean;
    focusOnClick?: boolean;
    autoConnect?: boolean;
    qualityLevel?: number;
    compressionLevel?: number;
    showDotCursor?: boolean;
    style?: React.CSSProperties;
    onCredentialsRequired?: () => void;
    onDisconnect?: () => void;
  }

  export const VncScreen: React.ForwardRefExoticComponent<
    VncScreenProps & React.RefAttributes<VncScreenHandle>
  >;
}
