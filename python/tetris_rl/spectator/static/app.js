const ui = {
  active: document.querySelector("#active"),
  board: document.querySelector("#board"),
  boardMessage: document.querySelector("#board-message"),
  boardWrap: document.querySelector("#board-wrap"),
  candidates: document.querySelector("#candidates"),
  connection: document.querySelector("#connection"),
  connectionLabel: document.querySelector("#connection-label"),
  dataset: document.querySelector("#dataset"),
  error: document.querySelector("#error-banner"),
  footerSeed: document.querySelector("#footer-seed"),
  footerState: document.querySelector("#footer-state"),
  hold: document.querySelector("#hold"),
  inferenceMs: document.querySelector("#inference-ms"),
  parameters: document.querySelector("#parameters"),
  pieces: document.querySelector("#pieces"),
  play: document.querySelector("#play"),
  preview: document.querySelector("#preview"),
  reset: document.querySelector("#reset"),
  revision: document.querySelector("#revision"),
  scoreSpread: document.querySelector("#score-spread"),
  seed: document.querySelector("#seed"),
  selectedIndex: document.querySelector("#selected-index"),
  selectedScore: document.querySelector("#selected-score"),
  speed: document.querySelector("#speed"),
  step: document.querySelector("#step"),
};

let playing = false;
let pending = false;

function buildBoard() {
  const fragment = document.createDocumentFragment();
  for (let index = 0; index < 200; index += 1) {
    const cell = document.createElement("span");
    cell.className = "cell";
    cell.setAttribute("aria-hidden", "true");
    fragment.append(cell);
  }
  ui.board.append(fragment);
}

async function request(path, payload) {
  const options = payload === undefined ? {} : {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(payload),
  };
  const response = await fetch(path, options);
  const body = await response.json();
  if (!response.ok) throw new Error(body.error || `HTTP ${response.status}`);
  return body;
}

function render(state) {
  ui.connection.dataset.state = "ready";
  ui.connectionLabel.textContent = "모델 연결됨";
  ui.boardWrap.dataset.state = state.top_out ? "error" : "ready";
  ui.boardMessage.hidden = !state.top_out;
  ui.pieces.textContent = state.pieces_placed.toLocaleString("ko-KR");
  ui.candidates.textContent = state.last_decision.candidate_count || "—";
  ui.active.textContent = state.active;
  ui.hold.textContent = state.hold || "—";
  ui.parameters.textContent = state.parameters.toLocaleString("ko-KR");
  ui.revision.textContent = state.engine_revision || "—";
  ui.revision.title = state.engine_revision || "";
  ui.dataset.textContent = state.dataset_id || "—";
  ui.dataset.title = state.dataset_id || "";
  ui.selectedIndex.textContent = formatOptional(state.last_decision.selected_index, 0);
  ui.selectedScore.textContent = formatOptional(state.last_decision.selected_score, 4);
  ui.scoreSpread.textContent = formatOptional(state.last_decision.score_spread, 4);
  ui.inferenceMs.textContent = state.last_decision.inference_ms == null ? "—" : `${state.last_decision.inference_ms.toFixed(2)} ms`;
  ui.seed.value = state.seed;
  ui.footerSeed.textContent = `seed ${state.seed}`;
  ui.footerState.textContent = state.top_out ? "TOP OUT" : playing ? "재생 중" : "대기";

  const cells = ui.board.children;
  for (let displayRow = 0; displayRow < 20; displayRow += 1) {
    const engineRow = 19 - displayRow;
    const occupied = state.board_rows[engineRow] || 0;
    const garbage = state.garbage_rows[engineRow] || 0;
    for (let column = 0; column < 10; column += 1) {
      const cell = cells[displayRow * 10 + column];
      const mask = 1 << column;
      cell.dataset.filled = String((occupied & mask) !== 0);
      cell.dataset.garbage = String((garbage & mask) !== 0);
    }
  }
  ui.preview.replaceChildren(...state.preview.map((piece) => {
    const item = document.createElement("li");
    item.textContent = piece;
    return item;
  }));
  ui.boardWrap.dataset.flash = "false";
  requestAnimationFrame(() => { ui.boardWrap.dataset.flash = "true"; });
}

function formatOptional(value, digits) {
  if (value == null) return "—";
  return typeof value === "number" && digits > 0 ? value.toFixed(digits) : String(value);
}

function showError(error) {
  playing = false;
  ui.play.textContent = "재생";
  ui.play.setAttribute("aria-pressed", "false");
  ui.connection.dataset.state = "error";
  ui.connectionLabel.textContent = "연결 오류";
  ui.error.textContent = `요청이 실패했습니다. ${error.message}`;
  ui.error.hidden = false;
  ui.footerState.textContent = "오류";
}

async function advance(count = 1) {
  if (pending) return;
  pending = true;
  ui.step.disabled = true;
  ui.step.dataset.state = "loading";
  try {
    const state = await request("/api/step", { count });
    render(state);
    ui.error.hidden = true;
    if (state.top_out) {
      playing = false;
      ui.play.setAttribute("aria-pressed", "false");
    }
  } catch (error) {
    showError(error);
  } finally {
    pending = false;
    ui.step.disabled = false;
    ui.step.dataset.state = "default";
  }
}

async function playback() {
  if (!playing) return;
  await advance(Number(ui.speed.value) === 0 ? 25 : 1);
  if (!playing) {
    ui.play.textContent = "재생";
    ui.play.setAttribute("aria-pressed", "false");
    return;
  }
  const delay = Number(ui.speed.value);
  window.setTimeout(playback, delay || 0);
}

ui.play.addEventListener("click", () => {
  playing = !playing;
  ui.play.textContent = playing ? "일시정지" : "재생";
  ui.play.setAttribute("aria-pressed", String(playing));
  ui.footerState.textContent = playing ? "재생 중" : "대기";
  if (playing) playback();
});
ui.step.addEventListener("click", () => advance());
ui.reset.addEventListener("click", async () => {
  const seed = Number(ui.seed.value);
  if (!Number.isSafeInteger(seed) || seed < 0) {
    showError(new Error("시드는 0 이상의 안전한 정수여야 합니다."));
    return;
  }
  playing = false;
  ui.play.textContent = "재생";
  ui.play.setAttribute("aria-pressed", "false");
  ui.reset.disabled = true;
  ui.reset.dataset.state = "loading";
  try {
    render(await request("/api/reset", { seed }));
    ui.reset.dataset.state = "success";
    ui.error.hidden = true;
  } catch (error) {
    ui.reset.dataset.state = "error";
    showError(error);
  } finally {
    ui.reset.disabled = false;
    window.setTimeout(() => { ui.reset.dataset.state = "default"; }, 500);
  }
});

buildBoard();
request("/api/state").then(render).catch(showError);
