import { useEffect, useRef, useState, type ReactNode } from "react";
import { createPortal } from "react-dom";
import { classes } from "./classNames";

type ModalProps = {
  open: boolean;
  titleId: string;
  onClose: () => void;
  onExited?: () => void;
  backdropClose?: boolean;
  active?: boolean;
  className?: string;
  backdropClassName?: string;
  children: ReactNode;
};

const exitDuration = 220;
const focusable =
  'button:not(:disabled), input:not(:disabled), select:not(:disabled), textarea:not(:disabled), summary, [tabindex="0"]';
let scrollLocks = 0;
let originalOverflow = "";

export function Modal({
  open,
  titleId,
  onClose,
  onExited,
  backdropClose = false,
  active = true,
  className,
  backdropClassName,
  children,
}: ModalProps) {
  const [mounted, setMounted] = useState(open);
  const dialogRef = useRef<HTMLElement>(null);
  const previousFocus = useRef<HTMLElement | null>(null);
  const lastChildren = useRef(children);
  const exited = useRef(onExited);
  exited.current = onExited;
  if (open) lastChildren.current = children;

  useEffect(() => {
    if (open) {
      previousFocus.current ??= document.activeElement as HTMLElement | null;
      setMounted(true);
      return;
    }
    if (!mounted) return;
    const duration = window.matchMedia("(prefers-reduced-motion: reduce)")
      .matches
      ? 0
      : exitDuration;
    const timer = window.setTimeout(() => {
      setMounted(false);
      exited.current?.();
      if (
        previousFocus.current?.isConnected &&
        !previousFocus.current.closest("[inert]")
      )
        previousFocus.current.focus();
      previousFocus.current = null;
    }, duration);
    return () => window.clearTimeout(timer);
  }, [open, mounted]);

  useEffect(() => {
    if (!mounted) return;
    if (scrollLocks++ === 0) {
      originalOverflow = document.body.style.overflow;
      document.body.style.overflow = "hidden";
    }
    return () => {
      if (--scrollLocks === 0) document.body.style.overflow = originalOverflow;
    };
  }, [mounted]);

  useEffect(() => {
    if (!open || !mounted || !active) return;
    const dialog = dialogRef.current;
    const controls = () =>
      Array.from(dialog?.querySelectorAll<HTMLElement>(focusable) ?? []);
    (controls()[0] ?? dialog)?.focus();
    function keydown(event: KeyboardEvent) {
      if (event.key === "Escape") {
        event.preventDefault();
        event.stopImmediatePropagation();
        onClose();
      } else if (event.key === "Tab") {
        const items = controls();
        const first = items[0];
        const last = items[items.length - 1];
        if (!first) {
          event.preventDefault();
          dialog?.focus();
        } else if (event.shiftKey && document.activeElement === first) {
          event.preventDefault();
          last.focus();
        } else if (!event.shiftKey && document.activeElement === last) {
          event.preventDefault();
          first.focus();
        }
      }
    }
    document.addEventListener("keydown", keydown);
    return () => document.removeEventListener("keydown", keydown);
  }, [open, mounted, active, onClose]);

  if (!mounted) return null;

  return createPortal(
    <div
      className={classes("modal-backdrop", backdropClassName)}
      data-state={open ? "open" : "closed"}
      inert={!open || !active}
      onMouseDown={(event) => {
        if (
          open &&
          active &&
          backdropClose &&
          event.target === event.currentTarget
        )
          onClose();
      }}
    >
      <section
        ref={dialogRef}
        className={classes("modal", className)}
        role="dialog"
        aria-modal="true"
        aria-labelledby={titleId}
        tabIndex={-1}
      >
        {open ? children : lastChildren.current}
      </section>
    </div>,
    document.body,
  );
}
