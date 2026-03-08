import * as React from "react";
import { cn } from "@/lib/utils";

interface SeparatorProps extends React.HTMLAttributes<HTMLDivElement> {
  decorative?: boolean;
  orientation?: "horizontal" | "vertical";
}

const Separator = React.forwardRef<HTMLDivElement, SeparatorProps>(
  ({ className, decorative = true, orientation = "horizontal", ...props }, ref) => (
    <div
      aria-hidden={decorative}
      className={cn(
        "shrink-0 bg-border",
        orientation === "horizontal" ? "h-px w-full" : "h-full w-px",
        className,
      )}
      ref={ref}
      role={decorative ? "presentation" : "separator"}
      {...props}
    />
  ),
);
Separator.displayName = "Separator";

export { Separator };
