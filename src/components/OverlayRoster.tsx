import { useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useAgents } from "../hooks/useAgents";
import type { AgentState, AgentStatus } from "../types";

const DOT: Record<AgentStatus, string> = {
  waiting: "bg-red-500",
  limited: "bg-purple-500",
  idle: "bg-amber-400",
  working: "bg-sky-500",
  ended: "bg-neutral-500",
};

const RANK: Record<AgentStatus, number> = {
  waiting: 4,
  limited: 3,
  idle: 2,
  working: 1,
  ended: 0,
};

const fmtReset = (ms: number) => {
  const d = new Date(ms);
  const h24 = d.getHours();
  const h = h24 % 12 === 0 ? 12 : h24 % 12;
  const m = String(d.getMinutes()).padStart(2, "0");
  return `${h}:${m}${h24 < 12 ? "am" : "pm"}`;
};

export function OverlayRoster() {
  const agents = useAgents();
  const rows = [...agents].sort(
    (a, b) => RANK[b.status] - RANK[a.status] || a.name.localeCompare(b.name)
  );

  const [editing, setEditing] = useState<string | null>(null);
  const [draft, setDraft] = useState("");

  const beginEdit = (sessionId: string, current: string) => {
    setEditing(sessionId);
    setDraft(current);
  };

  const commit = (cwd: string) => {
    if (editing === null) return;
    setEditing(null);
    invoke("set_alias", { cwd, name: draft }).catch(() => {});
  };

  const cancel = () => setEditing(null);

  const clickTimer = useRef<ReturnType<typeof setTimeout> | null>(null);

  // A single click focuses the session's terminal, but only after the
  // double-click window has passed so renaming never yanks focus away.
  const clickFocus = (a: AgentState) => {
    if (editing !== null) return;
    if (clickTimer.current) clearTimeout(clickTimer.current);
    clickTimer.current = setTimeout(() => {
      invoke("focus_session", {
        pid: a.pid,
        cwd: a.cwd,
        sessionId: a.session_id,
      }).catch(() => {});
    }, 250);
  };

  return (
    <div className="flex h-screen w-screen flex-col rounded-xl bg-neutral-950/90 px-2 py-1 text-neutral-100 select-none">
      <div className="flex h-6 w-full shrink-0 items-center">
        <div data-tauri-drag-region className="h-full flex-1 cursor-grab" />
        <button
          aria-label="minimize"
          onClick={() => invoke("hide_overlay").catch(() => {})}
          className="px-2 text-neutral-400 hover:text-neutral-100"
        >
          &#8211;
        </button>
      </div>
      <div className="min-h-0 flex-1 overflow-y-auto">
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
          {editing === a.session_id ? (
            <input
              autoFocus
              value={draft}
              onChange={(e) => setDraft(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter") commit(a.cwd);
                if (e.key === "Escape") cancel();
              }}
              onBlur={() => commit(a.cwd)}
              className="min-w-0 flex-1 rounded border border-sky-500 bg-neutral-900 px-1 text-sm outline-none"
            />
          ) : (
            <span
              data-testid="row-name"
              onClick={() => clickFocus(a)}
              onDoubleClick={() => {
                if (clickTimer.current) clearTimeout(clickTimer.current);
                beginEdit(a.session_id, a.name);
              }}
              className="truncate"
            >
              {a.name}
            </span>
          )}
          {a.status === "limited" && (
            <span className="ml-auto shrink-0 text-xs text-neutral-400">
              {a.resume_fired
                ? "resuming"
                : a.limited_until
                  ? `resets ${fmtReset(a.limited_until)}`
                  : "limited"}
            </span>
          )}
        </div>
      ))}
      </div>
    </div>
  );
}
