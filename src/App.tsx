import { useState } from "react";
import { useAgents } from "./hooks/useAgents";
import { RosterView } from "./components/RosterView";
import { TamagotchiView } from "./components/TamagotchiView";

export default function App() {
  const agents = useAgents();
  const [view, setView] = useState<"roster" | "tama">("roster");
  return (
    <div className="min-h-screen bg-neutral-950 text-neutral-100 p-4">
      <div className="flex gap-2 mb-4">
        <button
          onClick={() => setView("roster")}
          className={`px-3 py-1 rounded ${view === "roster" ? "bg-sky-600" : "bg-neutral-800"}`}
        >
          Roster
        </button>
        <button
          onClick={() => setView("tama")}
          className={`px-3 py-1 rounded ${view === "tama" ? "bg-sky-600" : "bg-neutral-800"}`}
        >
          Tamagotchi
        </button>
      </div>
      {view === "roster" ? <RosterView agents={agents} /> : <TamagotchiView agents={agents} />}
    </div>
  );
}
