import type { ComponentProps } from "react";
import { Icon, type IconName } from "./Icon";
import { classes } from "./classNames";

type ButtonVariant =
  "default" | "primary" | "subtle-danger" | "stop" | "text" | "muted-icon";
type ButtonSize = "default" | "page" | "icon" | "text";

const buttonBase =
  "inline-flex items-center justify-center gap-1.5 whitespace-nowrap rounded-[7px] border text-[13px] leading-[1.5] transition-[background,border-color,box-shadow] duration-150 focus-visible:outline-[3px] focus-visible:outline-[#5688ea60] focus-visible:outline-offset-[3px] disabled:cursor-not-allowed disabled:opacity-45 enabled:active:translate-y-px";

const buttonVariants: Record<ButtonVariant, string> = {
  default:
    "border-[rgba(115,121,153,0.15)] bg-[rgba(255,255,255,0.7)] text-[#404a66] enabled:hover:border-[#abc2ee] enabled:hover:bg-white enabled:hover:shadow-[0_2px_7px_#3768c60a]",
  primary:
    "border-transparent bg-[#4271c7] text-white shadow-[0_4px_10px_#5688ea24] enabled:hover:border-transparent enabled:hover:bg-[#3768c6] enabled:hover:text-white",
  "subtle-danger":
    "border-[rgba(115,121,153,0.15)] bg-[rgba(255,255,255,0.7)] text-[#b75b53] enabled:hover:border-[#abc2ee] enabled:hover:bg-white enabled:hover:shadow-[0_2px_7px_#3768c60a]",
  stop: "border-[#ee937e80] bg-[#fff1ec] text-[#a54e42] enabled:hover:border-[#abc2ee] enabled:hover:bg-white enabled:hover:shadow-[0_2px_7px_#3768c60a]",
  text: "border-transparent bg-transparent text-[#5688ea] enabled:hover:bg-white",
  "muted-icon": "border-transparent bg-transparent text-[#8a93a6]",
};

const iconButtonVariants: Record<Exclude<ButtonVariant, "text">, string> = {
  default:
    "border-transparent bg-transparent text-[#404a66] enabled:hover:border-[#abc2ee] enabled:hover:bg-white enabled:hover:shadow-[0_2px_7px_#3768c60a]",
  primary:
    "border-transparent bg-transparent text-[#3768c6] enabled:hover:border-[#abc2ee] enabled:hover:bg-white enabled:hover:shadow-[0_2px_7px_#3768c60a]",
  "subtle-danger":
    "border-transparent bg-transparent text-[#b75b53] enabled:hover:border-[#abc2ee] enabled:hover:bg-white enabled:hover:shadow-[0_2px_7px_#3768c60a]",
  stop: "border-transparent bg-transparent text-[#a54e42] enabled:hover:border-[#abc2ee] enabled:hover:bg-white enabled:hover:shadow-[0_2px_7px_#3768c60a]",
  "muted-icon":
    "border-transparent bg-transparent text-[#8a93a6] enabled:hover:border-[#abc2ee] enabled:hover:bg-white enabled:hover:shadow-[0_2px_7px_#3768c60a] aria-expanded:text-[#3768c6]",
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
