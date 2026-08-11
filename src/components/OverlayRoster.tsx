import { useEffect, useRef } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { LogicalSize } from "@tauri-apps/api/dpi";
import { useAgents } from "../hooks/useAgents";
import type { AgentStatus } from "../types";

const DOT: Record<AgentStatus, string> = {
  waiting: "bg-red-500",
  idle: "bg-amber-400",
  working: "bg-sky-500",
  ended: "bg-neutral-500",
};

const RANK: Record<AgentStatus, number> = { waiting: 3, idle: 2, working: 1, ended: 0 };

const ROW_H = 30;
const CHROME_H = 16;
const MAX_H = 420;
const WIDTH = 240;

export function OverlayRoster() {
  const agents = useAgents();
  const rows = [...agents].sort(
    (a, b) => RANK[b.status] - RANK[a.status] || a.name.localeCompare(b.name)
  );
  const lastH = useRef(0);

  useEffect(() => {
    const h = Math.min(MAX_H, Math.max(1, rows.length) * ROW_H + CHROME_H);
    if (h === lastH.current) return;
    lastH.current = h;
    // The window follows the content: the overlay is always up, so it must
    // never be taller than what it is showing.
    getCurrentWindow().setSize(new LogicalSize(WIDTH, h)).catch(() => {});
  }, [rows.length]);

  return (
    <div
      data-tauri-drag-region
      className="h-screen w-screen overflow-y-auto rounded-xl bg-neutral-950/90 px-2 py-2 text-neutral-100 select-none"
    >
      {rows.length === 0 && (
        <div className="px-2 py-1 text-sm text-neutral-500">no sessions</div>
      )}
      {rows.map((a) => (
        <div key={a.session_id} className="flex items-center gap-2 rounded px-2 py-1 text-sm">
          <span
            data-testid="row-dot"
            data-status={a.status}
            className={`h-2.5 w-2.5 shrink-0 rounded-full ${DOT[a.status]}`}
          />
          <span data-testid="row-name" className="truncate">
            {a.name}
          </span>
        </div>
      ))}
    </div>
  );
}
