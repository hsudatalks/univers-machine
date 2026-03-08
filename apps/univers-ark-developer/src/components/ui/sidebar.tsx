import * as React from "react";
import { cn } from "@/lib/utils";

function Sidebar({ className, ...props }: React.HTMLAttributes<HTMLElement>) {
  return (
    <aside
      className={cn("min-h-0 overflow-hidden bg-transparent", className)}
      {...props}
    />
  );
}

function SidebarContent({ className, ...props }: React.HTMLAttributes<HTMLDivElement>) {
  return (
    <div
      className={cn("grid min-h-0 content-start gap-3 overflow-auto", className)}
      {...props}
    />
  );
}

function SidebarGroup({ className, ...props }: React.HTMLAttributes<HTMLDivElement>) {
  return <section className={cn("grid gap-1.5", className)} {...props} />;
}

function SidebarGroupLabel({
  className,
  ...props
}: React.HTMLAttributes<HTMLSpanElement>) {
  return (
    <span
      className={cn(
        "px-2 text-[0.62rem] font-semibold uppercase tracking-[0.14em] text-muted-foreground",
        className,
      )}
      {...props}
    />
  );
}

function SidebarMenu({ className, ...props }: React.HTMLAttributes<HTMLDivElement>) {
  return <div className={cn("grid gap-1", className)} {...props} />;
}

function SidebarMenuItem({ className, ...props }: React.HTMLAttributes<HTMLDivElement>) {
  return <div className={cn("grid gap-1", className)} {...props} />;
}

interface SidebarMenuButtonProps
  extends React.ButtonHTMLAttributes<HTMLButtonElement> {
  isActive?: boolean;
}

function SidebarMenuButton({
  className,
  isActive = false,
  ...props
}: SidebarMenuButtonProps) {
  return (
    <button
      className={cn(
        "sidebar-node",
        "inline-flex min-h-9 w-full items-center justify-between gap-2 rounded-[calc(var(--radius)-0.25rem)] px-2.5 py-1.5 text-left text-sm font-medium text-foreground transition-colors hover:bg-accent hover:text-accent-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 focus-visible:ring-offset-background disabled:pointer-events-none disabled:opacity-50",
        isActive && "is-active",
        isActive && "bg-accent text-accent-foreground shadow-sm",
        className,
      )}
      {...props}
    />
  );
}

function SidebarMenuSub({ className, ...props }: React.HTMLAttributes<HTMLDivElement>) {
  return (
    <div
      className={cn(
        "ml-4 grid gap-1 border-l border-border pl-3",
        className,
      )}
      {...props}
    />
  );
}

export {
  Sidebar,
  SidebarContent,
  SidebarGroup,
  SidebarGroupLabel,
  SidebarMenu,
  SidebarMenuButton,
  SidebarMenuItem,
  SidebarMenuSub,
};
