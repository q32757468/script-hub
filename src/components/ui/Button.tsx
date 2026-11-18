import type { ComponentProps } from "react";
import { Icon, type IconName } from "./Icon";
import { classes } from "./classNames";

type ButtonVariant =
  "default" | "primary" | "subtle-danger" | "stop" | "text" | "muted-icon";
type ButtonSize = "default" | "page" | "icon" | "text";

const buttonBase =
  "inline-flex items-center justify-center gap-1.5 whitespace-nowrap rounded-[7px] border text-[13px] leading-[1.5] transition-[background,border-color,box-shadow] duration-150 focus-visible:outline-[3px] focus-visible:outline-blue/40 focus-visible:outline-offset-[3px] disabled:cursor-not-allowed disabled:opacity-45 enabled:active:translate-y-px";

const buttonHover =
  "enabled:hover:border-blue-border enabled:hover:bg-white enabled:hover:shadow-[0_2px_7px] enabled:hover:shadow-blue/4";

const buttonVariants: Record<ButtonVariant, string> = {
  default: `border-[rgba(115,121,153,0.15)] bg-[rgba(255,255,255,0.7)] text-[#404a66] ${buttonHover}`,
  primary:
    "border-transparent bg-blue text-white shadow-[0_4px_10px] shadow-blue/14 enabled:hover:border-transparent enabled:hover:bg-blue-deep enabled:hover:text-white",
  "subtle-danger": `border-[rgba(115,121,153,0.15)] bg-[rgba(255,255,255,0.7)] text-[#b75b53] ${buttonHover}`,
  stop: `border-[#ee937e80] bg-[#fff1ec] text-[#a54e42] ${buttonHover}`,
  text: "border-transparent bg-transparent text-blue-deep enabled:hover:bg-white",
  "muted-icon": "border-transparent bg-transparent text-muted",
};

const iconButtonVariants: Record<Exclude<ButtonVariant, "text">, string> = {
  default: `border-transparent bg-transparent text-[#404a66] ${buttonHover}`,
  primary: `border-transparent bg-transparent text-blue-deep ${buttonHover}`,
  "subtle-danger": `border-transparent bg-transparent text-[#b75b53] ${buttonHover}`,
  stop: `border-transparent bg-transparent text-[#a54e42] ${buttonHover}`,
  "muted-icon": `border-transparent bg-transparent text-muted aria-expanded:text-blue-deep ${buttonHover}`,
};

const buttonSizes: Record<ButtonSize, string> = {
  default: "px-2.5 py-[5px]",
  page: "px-3 py-1.5 max-[760px]:px-2.5",
  icon: "min-w-[30px] p-1.5",
  text: "px-0 py-[5px]",
};

export type ButtonProps = ComponentProps<"button"> & {
  icon?: IconName;
  size?: ButtonSize;
  variant?: ButtonVariant;
};

export function Button({
  icon,
  size = "default",
  variant = "default",
  className,
  children,
  ...props
}: ButtonProps) {
  return (
    <button
      {...props}
      className={classes(
        buttonBase,
        size === "icon"
          ? iconButtonVariants[variant === "text" ? "default" : variant]
          : buttonVariants[variant],
        buttonSizes[size],
        className,
      )}
    >
      {icon && <Icon name={icon} size={15} />}
      {children}
    </button>
  );
}
