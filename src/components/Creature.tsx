import type { AgentStatus } from "../types";

const FACE: Record<AgentStatus, string> = {
  working: "(o.o)",
  idle: "(-.-)",
  waiting: "(!.!)",
  ended: "(x.x)",
};

const RING: Record<AgentStatus, string> = {
  working: "ring-sky-500",
  idle: "ring-amber-400 animate-pulse",
  waiting: "ring-red-500 animate-bounce",
  ended: "ring-neutral-600 opacity-50",
};

export function Creature({ status, name }: { status: AgentStatus; name: string }) {
  return (
    <div
      data-state={status}
      className={`flex flex-col items-center gap-1 p-3 rounded-xl ring-2 ${RING[status]} bg-neutral-900`}
    >
      <div className="font-mono text-lg text-neutral-100">{FACE[status]}</div>
      <div className="text-xs text-neutral-400 truncate max-w-[7rem]">{name}</div>
    </div>
  );
}
