const WIDTH = 10;
const HEIGHT = 20;
const ui = Object.fromEntries([...document.querySelectorAll("[id]")].map((element) => [element.id, element]));
const boards = { human: buildBoard(ui["human-board"]), model: buildBoard(ui["model-board"]) };
const edgeQueue = [];
const held = new Set();
let paused = true;
let pending = false;

const keyMap = new Map([
  ["ArrowLeft", "left"], ["ArrowRight", "right"], ["ArrowDown", "soft_drop"],
  ["Space", "hard_drop"], ["KeyZ", "rotate_counterclockwise"],
  ["KeyX", "rotate_clockwise"], ["KeyA", "rotate_half"], ["KeyC", "hold"],
]);

function buildBoard(element) {
  const cells = [];
  const fragment = document.createDocumentFragment();
  for (let index = 0; index < WIDTH * HEIGHT; index += 1) {
    const cell = document.createElement("span");
    cell.className = "cell";
    cell.setAttribute("aria-hidden", "true");
    fragment.append(cell);
    cells.push(cell);
  }
  element.append(fragment);
  return cells;
}

window.addEventListener("keydown", (event) => {
  if (event.target instanceof HTMLInputElement) return;
  if ([...keyMap.keys(), "KeyP", "KeyN", "KeyR"].includes(event.code)) event.preventDefault();
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
ui.pause.addEventListener("click", togglePause);
ui["single-step"].addEventListener("click", stepOnce);
ui.reset.addEventListener("click", reset);
ui.rematch.addEventListener("click", reset);

function releaseAll() {
  for (const code of held) edgeQueue.push({ button: keyMap.get(code), kind: "release" });
  held.clear();
}
function togglePause() {
  paused = !paused;
  ui.pause.textContent = paused ? "계속" : "일시정지";
  ui.pause.setAttribute("aria-pressed", String(paused));
  if (paused) releaseAll();
}
async function request(path, payload) {
  const response = await fetch(path, {
    method: "POST", headers: { "Content-Type": "application/json" }, body: JSON.stringify(payload),
  });
  const body = await response.json();
  if (!response.ok) throw new Error(body.error || `HTTP ${response.status}`);
  return body;
}
async function stepOnce() {
  if (pending) return;
  pending = true;
  const edges = edgeQueue.splice(0);
  try {
    render(await request("/api/frame", { edges }));
    setConnection("엔진 연결됨", "ready");
  } catch (error) {
    edgeQueue.unshift(...edges);
    showError(error);
  } finally { pending = false; }
}
async function reset() {
  const seed = Number(ui.seed.value);
  if (!Number.isSafeInteger(seed) || seed < 0) return showError(new Error("시드는 0 이상의 안전한 정수여야 합니다."));
  releaseAll(); edgeQueue.length = 0; paused = true; ui.pause.textContent = "계속";
  ui.pause.setAttribute("aria-pressed", "true");
  ui.reset.disabled = true; ui.reset.dataset.state = "loading";
  try {
    render(await request("/api/reset", { seed }));
    ui.reset.dataset.state = "success"; ui.error.hidden = true;
  } catch (error) { ui.reset.dataset.state = "error"; showError(error); }
  finally { ui.reset.disabled = false; window.setTimeout(() => { ui.reset.dataset.state = "default"; }, 500); }
}
function render(state) {
  renderPlayer("human", state.human); renderPlayer("model", state.model);
  ui.frame.textContent = state.frame.toLocaleString("ko-KR");
  ui["bot-next"].textContent = state.next_bot_frame.toLocaleString("ko-KR");
  ui.candidates.textContent = state.last_decision.candidate_count || "—";
  ui.inference.textContent = state.last_decision.inference_ms == null ? "—" : `${state.last_decision.inference_ms.toFixed(2)} ms`;
  ui.update.textContent = state.training_update ?? "—";
  ui.parameters.textContent = state.parameters.toLocaleString("ko-KR");
  ui["model-cadence"].textContent = `${state.frames_per_placement} frame / placement`;
  ui.seed.value = state.seed; ui.checkpoint.textContent = state.checkpoint; ui.checkpoint.title = state.checkpoint;
  const labels = { human_win: "승리", model_win: "패배", draw: "무승부" };
  ui.result.hidden = state.result === "ongoing";
  ui["result-label"].textContent = labels[state.result] || "";
  if (state.result !== "ongoing") {
    paused = true;
    ui.pause.setAttribute("aria-pressed", "true");
  }
}
function renderPlayer(name, player) {
  const cells = boards[name];
  for (const cell of cells) cell.className = "cell";
  for (let y = 0; y < HEIGHT; y += 1) for (let x = 0; x < WIDTH; x += 1) {
    const mask = 1 << x; if (((player.board_rows[y] || 0) & mask) === 0) continue;
    getCell(cells, x, y).classList.add(((player.garbage_rows[y] || 0) & mask) ? "garbage" : "locked");
  }
  if (player.active) for (const [x, y] of player.active.cells) {
    const cell = getCell(cells, x, y); if (cell) cell.classList.add("active", `piece-${player.active.kind.toLowerCase()}`);
  }
  ui[`${name}-pieces`].textContent = player.pieces_placed.toLocaleString("ko-KR");
  ui[`${name}-sent`].textContent = player.sent_lines.toLocaleString("ko-KR");
  ui[`${name}-hold`].textContent = player.hold || "—";
  ui[`${name}-garbage`].textContent = `${player.ready_garbage} / ${player.pending_garbage}`;
  ui[`${name}-next`].replaceChildren(...player.preview.slice(0, 5).map((piece) => Object.assign(document.createElement("li"), { textContent: piece })));
}
function getCell(cells, x, y) { if (x < 0 || x >= WIDTH || y < 0 || y >= HEIGHT) return null; return cells[(HEIGHT - 1 - y) * WIDTH + x]; }
function setConnection(label, state) { ui.connection.dataset.state = state; ui["connection-label"].textContent = label; }
function showError(error) { paused = true; setConnection("연결 오류", "error"); ui.error.textContent = `요청 실패: ${error.message}`; ui.error.hidden = false; }

window.setInterval(() => { if (!paused) stepOnce(); }, 1000 / 60);
fetch("/api/state").then((response) => response.json()).then((state) => { render(state); setConnection("엔진 연결됨", "ready"); }).catch(showError);
