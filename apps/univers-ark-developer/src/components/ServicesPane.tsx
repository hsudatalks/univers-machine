import { Globe, PlugZap, RotateCw, SquareTerminal } from "lucide-react";
import { openExternalLink } from "../lib/tauri";
import {
  commandServices,
  endpointServices,
  webServices,
  type CommandDeveloperService,
  type EndpointDeveloperService,
  type WebDeveloperService,
} from "../lib/target-services";
import type { DeveloperTarget, ServiceStatus } from "../types";
import { Badge } from "./ui/badge";
import { Button } from "./ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "./ui/card";
import { Separator } from "./ui/separator";

interface ServicesPaneProps {
  activeBrowserServiceId: string | null;
  onOpenBrowserService: (serviceId: string) => void;
  onRestartBrowserService: (serviceId: string) => void;
  onRunCommandService: (serviceId: string, action: "restart") => Promise<void>;
  serviceStatuses: Record<string, ServiceStatus>;
  target: DeveloperTarget;
}

function serviceKey(targetId: string, serviceId: string): string {
  return `${targetId}::${serviceId}`;
}

function statusVariant(
  state: string | undefined,
): "neutral" | "success" | "warning" | "destructive" {
  switch (state) {
    case "running":
    case "ready":
    case "healthy":
    case "loaded":
    case "embedded":
    case "direct":
      return "success";
    case "starting":
    case "loading":
    case "unknown":
      return "warning";
    case "down":
    case "error":
    case "failed":
    case "stopped":
      return "destructive";
    default:
      return "neutral";
  }
}

function WebServiceRow({
  activeBrowserServiceId,
  onOpenBrowserService,
  onRestartBrowserService,
  service,
  status,
}: {
  activeBrowserServiceId: string | null;
  onOpenBrowserService: (serviceId: string) => void;
  onRestartBrowserService: (serviceId: string) => void;
  service: WebDeveloperService;
  status?: ServiceStatus;
}) {
  const localUrl = status?.localUrl ?? service.web.localUrl ?? null;
  const state = status?.state ?? "unknown";

  return (
    <div className="service-row" key={service.id}>
      <div className="service-row-copy">
        <div className="service-row-header">
          <span className="service-row-title">{service.label}</span>
          <Badge variant={statusVariant(state)}>{state}</Badge>
        </div>
        <p className="service-row-detail">
          {status?.message || service.description || service.web.remoteUrl}
        </p>
        {localUrl ? <code className="service-row-url">{localUrl}</code> : null}
      </div>
      <div className="service-row-actions">
        <Button
          isActive={activeBrowserServiceId === service.id}
          onClick={() => {
            onOpenBrowserService(service.id);
          }}
          size="sm"
          variant={activeBrowserServiceId === service.id ? "default" : "outline"}
        >
          View
        </Button>
        <Button
          disabled={!localUrl}
          onClick={() => {
            if (localUrl) {
              void openExternalLink(localUrl);
            }
          }}
          size="sm"
          variant="outline"
        >
          Open
        </Button>
        <Button
          onClick={() => {
            onRestartBrowserService(service.id);
          }}
          size="icon"
          title="Restart service tunnel"
          variant="ghost"
        >
          <RotateCw size={14} />
        </Button>
      </div>
    </div>
  );
}

function EndpointServiceRow({
  service,
  status,
}: {
  service: EndpointDeveloperService;
  status?: ServiceStatus;
}) {
  const fallbackUrl =
    service.endpoint.url ||
    `${service.endpoint.probeType}://${service.endpoint.host || "127.0.0.1"}:${service.endpoint.port}${service.endpoint.path || ""}`;
  const state = status?.state ?? "unknown";

  return (
    <div className="service-row" key={service.id}>
      <div className="service-row-copy">
        <div className="service-row-header">
          <span className="service-row-title">{service.label}</span>
          <Badge variant={statusVariant(state)}>{state}</Badge>
        </div>
        <p className="service-row-detail">
          {status?.message || service.description || fallbackUrl}
        </p>
        <code className="service-row-url">{status?.localUrl ?? fallbackUrl}</code>
      </div>
    </div>
  );
}

function CommandServiceRow({
  onRunCommandService,
  service,
  status,
}: {
  onRunCommandService: (serviceId: string, action: "restart") => Promise<void>;
  service: CommandDeveloperService;
  status?: ServiceStatus;
}) {
  const state = status?.state ?? "idle";

  return (
    <div className="service-row" key={service.id}>
      <div className="service-row-copy">
        <div className="service-row-header">
          <span className="service-row-title">{service.label}</span>
          <Badge variant={statusVariant(state)}>{state}</Badge>
        </div>
        <p className="service-row-detail">
          {status?.message || service.description || "Command service"}
        </p>
        <code className="service-row-url">{service.command.restart}</code>
      </div>
      <div className="service-row-actions">
        <Button
          onClick={() => {
            void onRunCommandService(service.id, "restart");
          }}
          size="sm"
          variant="outline"
        >
          Restart
        </Button>
      </div>
    </div>
  );
}

export function ServicesPane({
  activeBrowserServiceId,
  onOpenBrowserService,
  onRestartBrowserService,
  onRunCommandService,
  serviceStatuses,
  target,
}: ServicesPaneProps) {
  const web = webServices(target);
  const endpoints = endpointServices(target);
  const commands = commandServices(target);

  return (
    <article className="panel tool-panel services-panel">
      <div className="services-grid">
        <Card className="border-border/80 bg-card/95">
          <CardHeader className="pb-3">
            <CardTitle className="dashboard-section-title">
              <Globe size={14} />
              Web
            </CardTitle>
          </CardHeader>
          <Separator />
          <CardContent className="pt-4">
            {web.length === 0 ? (
              <p className="dashboard-copy">No web services declared.</p>
            ) : (
              <div className="services-list">
                {web.map((service) => (
                  <WebServiceRow
                    activeBrowserServiceId={activeBrowserServiceId}
                    key={service.id}
                    onOpenBrowserService={onOpenBrowserService}
                    onRestartBrowserService={onRestartBrowserService}
                    service={service}
                    status={serviceStatuses[serviceKey(target.id, service.id)]}
                  />
                ))}
              </div>
            )}
          </CardContent>
        </Card>

        <Card className="border-border/80 bg-card/95">
          <CardHeader className="pb-3">
            <CardTitle className="dashboard-section-title">
              <PlugZap size={14} />
              Endpoints
            </CardTitle>
          </CardHeader>
          <Separator />
          <CardContent className="pt-4">
            {endpoints.length === 0 ? (
              <p className="dashboard-copy">No endpoint services declared.</p>
            ) : (
              <div className="services-list">
                {endpoints.map((service) => (
                  <EndpointServiceRow
                    key={service.id}
                    service={service}
                    status={serviceStatuses[serviceKey(target.id, service.id)]}
                  />
                ))}
              </div>
            )}
          </CardContent>
        </Card>

        <Card className="dashboard-card-wide border-border/80 bg-card/95">
          <CardHeader className="pb-3">
            <CardTitle className="dashboard-section-title">
              <SquareTerminal size={14} />
              Commands
            </CardTitle>
          </CardHeader>
          <Separator />
          <CardContent className="pt-4">
            {commands.length === 0 ? (
              <p className="dashboard-copy">No command services declared.</p>
            ) : (
              <div className="services-list">
                {commands.map((service) => (
                  <CommandServiceRow
                    key={service.id}
                    onRunCommandService={onRunCommandService}
                    service={service}
                    status={serviceStatuses[serviceKey(target.id, service.id)]}
                  />
                ))}
              </div>
            )}
          </CardContent>
        </Card>
      </div>
    </article>
  );
}
