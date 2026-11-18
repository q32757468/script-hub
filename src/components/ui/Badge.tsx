import type { ReactNode } from "react";
import { statusText } from "../../domain";
import { classes } from "./classNames";

const badgeVariants: Record<string, string> = {
  idle: "bg-[#e9edf469] text-muted",
  running: "bg-[#68c7ae18] text-status-success",
  succeeded: "bg-[#68c7ae18] text-status-success",
  failed: "bg-[#ee937e22] text-status-danger",
  abnormal: "bg-[#ee937e22] text-status-danger",
  starting: "bg-[#f0d8aa3b] text-status-warning",
  stopping: "bg-[#f0d8aa3b] text-status-warning",
};

export function Badge({
  status = "idle",
  children,
  className,
}: {
  status?: string;
  children?: ReactNode;
  className?: string;
}) {
  return (
    <span
      className={classes(
        "inline-flex items-center gap-[5px] whitespace-nowrap rounded-md px-[7px] py-[3px] text-xs",
        badgeVariants[status] ?? badgeVariants.idle,
        className,
      )}
    >
      <span aria-hidden="true" className="size-[5px] rounded-full bg-current" />
      {children ?? statusText[status] ?? status}
    </span>
  );
}
