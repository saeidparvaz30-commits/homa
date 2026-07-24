import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import type { TraySummary } from "../types";

const COLOR: Record<string, string> = {
  waiting: "bg-red-500",
  idle: "bg-amber-400",
  working: "bg-sky-500",
  ended: "bg-neutral-500",
};

export function OverlayPill() {
  const [s, setS] = useState<TraySummary>({ top: "ended", waiting: 0, idle: 0, working: 0 });
  useEffect(() => {
    const un = listen<TraySummary>("tray-summary", (e) => setS(e.payload));
    return () => {
      un.then((f) => f());
    };
  }, []);
  const count = s.top === "waiting" ? s.waiting : s.idle;
  const label = s.top === "waiting" ? "waiting" : "idle";
  return (
    <div
      data-tauri-drag-region
      onMouseDown={() => getCurrentWindow().startDragging()}
      className={`flex items-center gap-2 h-16 px-4 rounded-2xl text-white font-semibold select-none ${COLOR[s.top]}`}
    >
      <span className="w-3 h-3 rounded-full bg-white/90 animate-pulse" />
      <span>
        {count} {label}
      </span>
    </div>
  );
}
