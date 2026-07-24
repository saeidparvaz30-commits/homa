import { mkdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";

const dir = process.env.HOMA_SESSIONS_DIR || join(process.cwd(), ".homa-replay");
mkdirSync(dir, { recursive: true });

const pid = process.pid; // a live pid so the agent is not classified as Ended
const write = (status) =>
  writeFileSync(
    join(dir, `${pid}.json`),
    JSON.stringify({
      pid,
      sessionId: "replay-1",
      cwd: "C:\\demo\\myrepo",
      name: "replay-agent",
      status,
      startedAt: Date.now(),
      statusUpdatedAt: Date.now(),
    }),
  );

const seq = ["busy", "idle", "waiting", "busy"];
let i = 0;
console.log("Replay dir:", dir);
console.log("Set HOMA_SESSIONS_DIR to this path and run the app to watch the cycle.");
write("busy");
setInterval(() => {
  const s = seq[i++ % seq.length];
  write(s);
  console.log("status ->", s);
}, 4000);
