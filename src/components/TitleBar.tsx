import { useCallback, useEffect, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import {
  desktop,
  windowClose,
  windowIsMaximized,
  windowMinimize,
  windowToggleMaximize,
} from "../api";

export function useWindowMaximized() {
  const [isMaximized, setIsMaximized] = useState(false);

  useEffect(() => {
    if (!desktop) return;
    let unlisten: (() => void) | undefined;

    const checkMaximized = async () => {
      const maximized = await windowIsMaximized();
      setIsMaximized(maximized);
    };

    void checkMaximized();

    try {
      const appWin = getCurrentWindow();
      void appWin
        .onResized(() => {
          void checkMaximized();
        })
        .then((fn) => {
          unlisten = fn;
        })
        .catch(() => {});
    } catch {}

    return () => {
      if (unlisten) unlisten();
    };
  }, []);

  const toggleMaximize = useCallback(async () => {
    await windowToggleMaximize();
    if (desktop) {
      setTimeout(async () => {
        const next = await windowIsMaximized();
        setIsMaximized(next);
      }, 50);
    } else {
      setIsMaximized((prev) => !prev);
    }
  }, []);

  return { isMaximized, toggleMaximize };
}

export function TitleBar({
  onToggleMaximize,
}: {
  onToggleMaximize?: () => void;
}) {
  const { isMaximized, toggleMaximize } = useWindowMaximized();
  const handleToggle = onToggleMaximize ?? toggleMaximize;

  return (
    <header
      className="window-titlebar"
      data-tauri-drag-region
      onDoubleClick={() => void handleToggle()}
    >
      <div className="window-titlebar-drag-area" data-tauri-drag-region />

      <div
        className="window-controls"
        onDoubleClick={(e) => e.stopPropagation()}
      >
        <button
          type="button"
          tabIndex={-1}
          className="window-control-btn minimize"
          aria-label="最小化"
          title="最小化"
          onClick={() => void windowMinimize()}
        >
          <svg
            width="11"
            height="11"
            viewBox="0 0 11 11"
            fill="none"
            aria-hidden="true"
          >
            <path
              d="M1.5 5.5h8"
              stroke="currentColor"
              strokeWidth="1.2"
              strokeLinecap="round"
            />
          </svg>
        </button>

        <button
          type="button"
          tabIndex={-1}
          className="window-control-btn maximize"
          aria-label={isMaximized ? "向下还原" : "最大化"}
          title={isMaximized ? "向下还原" : "最大化"}
          onClick={() => void handleToggle()}
        >
          {isMaximized ? (
            <svg
              width="11"
              height="11"
              viewBox="0 0 11 11"
              fill="none"
              aria-hidden="true"
            >
              <rect
                x="3"
                y="1"
                width="7"
                height="7"
                rx="1"
                stroke="currentColor"
                strokeWidth="1.1"
              />
              <path
                d="M1 3.5v5.5a1 1 0 0 0 1 1h5.5"
                stroke="currentColor"
                strokeWidth="1.1"
                strokeLinecap="round"
              />
            </svg>
          ) : (
            <svg
              width="11"
              height="11"
              viewBox="0 0 11 11"
              fill="none"
              aria-hidden="true"
            >
              <rect
                x="1"
                y="1"
                width="9"
                height="9"
                rx="1.5"
                stroke="currentColor"
                strokeWidth="1.1"
              />
            </svg>
          )}
        </button>

        <button
          type="button"
          tabIndex={-1}
          className="window-control-btn close"
          aria-label="关闭"
          title="关闭"
          onClick={() => void windowClose()}
        >
          <svg
            width="11"
            height="11"
            viewBox="0 0 11 11"
            fill="none"
            aria-hidden="true"
          >
            <path
              d="M2 2l7 7M9 2L2 9"
              stroke="currentColor"
              strokeWidth="1.2"
              strokeLinecap="round"
            />
          </svg>
        </button>
      </div>
    </header>
  );
}

export const ImmersiveTitleBar = TitleBar;
