const WIDTH = 10;
const HEIGHT = 20;
const board = document.querySelector("#board");
const cells = [];
const edgeQueue = [];
const held = new Set();
let paused = false;
let requestInFlight = false;

const keyMap = new Map([
  ["ArrowLeft", "left"],
  ["ArrowRight", "right"],
  ["ArrowDown", "soft_drop"],
  ["Space", "hard_drop"],
  ["KeyZ", "rotate_counterclockwise"],
  ["KeyX", "rotate_clockwise"],
  ["KeyA", "rotate_half"],
  ["KeyC", "hold"],
]);

for (let visualY = HEIGHT - 1; visualY >= 0; visualY -= 1) {
  for (let x = 0; x < WIDTH; x += 1) {
    const cell = document.createElement("div");
    cell.className = "cell";
    cell.dataset.x = String(x);
    cell.dataset.y = String(visualY);
    board.append(cell);
    cells.push(cell);
  }
}

window.addEventListener("keydown", (event) => {
  if ([...keyMap.keys(), "KeyP", "KeyN", "KeyR"].includes(event.code)) {
    event.preventDefault();
  }
  if (event.code === "KeyP" && !event.repeat) togglePause();
  if (event.code === "KeyN" && !event.repeat) stepOnce();
  if (event.code === "KeyR" && !event.repeat) reset();
  const button = keyMap.get(event.code);
  if (!button || held.has(event.code)) return;
  held.add(event.code);
  edgeQueue.push({ button, kind: "press" });
});

window.addEventListener("keyup", (event) => {
  const button = keyMap.get(event.code);
  if (!button || !held.delete(event.code)) return;
  edgeQueue.push({ button, kind: "release" });
});

window.addEventListener("blur", releaseAll);
document.querySelector("#pause").addEventListener("click", togglePause);
document.querySelector("#single-step").addEventListener("click", stepOnce);
document.querySelector("#reset").addEventListener("click", reset);

function releaseAll() {
  for (const code of held) {
    edgeQueue.push({ button: keyMap.get(code), kind: "release" });
  }
  held.clear();
}

function togglePause() {
  paused = !paused;
  document.querySelector("#pause").textContent = paused ? "계속" : "일시정지";
  if (paused) releaseAll();
}

async function stepOnce() {
  if (requestInFlight) return;
  requestInFlight = true;
  const edges = edgeQueue.splice(0);
  try {
    const response = await fetch("/api/step", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ edges }),
    });
    const state = await response.json();
    if (!response.ok) throw new Error(state.error || "step failed");
    render(state);
    setConnection("엔진 연결됨", true);
  } catch (error) {
    edgeQueue.unshift(...edges);
    setConnection(error.message, false);
  } finally {
    requestInFlight = false;
  }
}

async function reset() {
  releaseAll();
  edgeQueue.length = 0;
  try {
    const response = await fetch("/api/reset", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ seed: 1 }),
    });
    const state = await response.json();
    if (!response.ok) throw new Error(state.error || "reset failed");
    render(state);
  } catch (error) {
    setConnection(error.message, false);
  }
}

function render(state) {
  for (const cell of cells) cell.className = "cell";
  for (let y = 0; y < HEIGHT; y += 1) {
    const boardRow = state.board_rows[y];
    const garbageRow = state.garbage_rows[y];
    for (let x = 0; x < WIDTH; x += 1) {
      const mask = 1 << x;
      if ((boardRow & mask) === 0) continue;
      getCell(x, y)?.classList.add((garbageRow & mask) !== 0 ? "garbage" : "locked");
    }
  }
  if (state.active) {
    for (const [x, y] of state.active.cells) {
      getCell(x, y)?.classList.add("active", `piece-${state.active.kind.toLowerCase()}`);
    }
  }
  setText("frame", state.frame);
  setText("pieces", state.pieces_placed);
  setText("hold", state.hold || "—");
  setText("preview", state.preview.join("  "));
  setText("last-event", state.last_event);
  setText("top-out", state.top_out ? `TOP OUT · ${state.top_out}` : "");
  const timing = state.timing;
  setText("lock", timing ? `${timing.lock_elapsed_frames} / 30` : "—");
  setText("resets", timing ? `${timing.lock_resets_used} / 15` : "—");
  setText("phase", timing ? timing.fall_fraction_micros : "—");
  setText("last-action", timing ? timing.last_action : "—");
}

function getCell(x, y) {
  if (x < 0 || x >= WIDTH || y < 0 || y >= HEIGHT) return null;
  const visualRow = HEIGHT - 1 - y;
  return cells[visualRow * WIDTH + x];
}

function setText(id, value) {
  document.querySelector(`#${id}`).textContent = String(value);
}

function setConnection(message, ok) {
  const element = document.querySelector("#connection");
  element.textContent = message;
  element.classList.toggle("ok", ok);
}

setInterval(() => {
  if (!paused) stepOnce();
}, 1000 / 60);

fetch("/api/state")
  .then((response) => response.json())
  .then((state) => {
    render(state);
    setConnection("엔진 연결됨", true);
  })
  .catch((error) => setConnection(error.message, false));
