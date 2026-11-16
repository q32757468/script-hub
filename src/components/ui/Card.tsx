import type { ComponentProps } from "react";
import { classes } from "./classNames";

export function Card({ className, ...props }: ComponentProps<"section">) {
  return (
    <section
      {...props}
      className={classes(
        "rounded-card border border-glass-edge bg-glass shadow-[0_5px_22px_#3b568306] backdrop-blur-[18px]",
        className,
      )}
    />
  );
}
