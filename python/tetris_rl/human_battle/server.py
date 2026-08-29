from __future__ import annotations

import argparse
import json
from http import HTTPStatus
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from urllib.parse import urlsplit

import torch

from tetris_rl.human_battle.controller import HumanBattleController

STATIC_DIRECTORY = Path(__file__).with_name("static")
ROOT_TOKENS = Path(__file__).resolve().parents[3] / "tokens.css"
STATIC_FILES = {
    "/": (STATIC_DIRECTORY / "index.html", "text/html; charset=utf-8"),
    "/app.js": (STATIC_DIRECTORY / "app.js", "text/javascript; charset=utf-8"),
    "/style.css": (STATIC_DIRECTORY / "style.css", "text/css; charset=utf-8"),
    "/tokens.css": (ROOT_TOKENS, "text/css; charset=utf-8"),
}


def main() -> None:
    parser = argparse.ArgumentParser(description="Play real-time Tetris against a checkpoint")
    parser.add_argument(
        "--checkpoint",
        type=Path,
        default=Path("checkpoints/versus-selfplay-r3/latest-model.pt"),
    )
    parser.add_argument("--seed", type=int, default=1)
    parser.add_argument("--frames-per-placement", type=int, default=12)
    parser.add_argument("--threads", type=int, default=2)
    parser.add_argument("--bind", default="0.0.0.0:8789")
    parser.add_argument("--allow-observed", action="store_true")
    args = parser.parse_args()
    if args.seed < 0 or args.threads <= 0 or args.frames_per_placement <= 0:
        raise ValueError("seed must be nonnegative; threads and cadence must be positive")
    host, separator, port_text = args.bind.rpartition(":")
    if not separator or not host:
        raise ValueError("bind must use host:port")

    torch.set_num_threads(args.threads)
    controller = HumanBattleController(
        args.checkpoint,
        args.seed,
        frames_per_placement=args.frames_per_placement,
        allow_observed=args.allow_observed,
    )
    server = ThreadingHTTPServer((host, int(port_text)), handler(controller))
    print(f"Human versus model: http://127.0.0.1:{port_text}", flush=True)
    server.serve_forever()


def handler(controller: HumanBattleController) -> type[BaseHTTPRequestHandler]:
    class HumanBattleHandler(BaseHTTPRequestHandler):
        def do_GET(self) -> None:
            path = urlsplit(self.path).path
            if path == "/api/state":
                self._send_json(controller.state())
                return
            static = STATIC_FILES.get(path)
            if static is None:
                self._send_json({"error": "경로를 찾을 수 없습니다."}, HTTPStatus.NOT_FOUND)
                return
            file_path, content_type = static
            try:
                body = file_path.read_bytes()
            except OSError as error:
                self._send_json(
                    {"error": f"정적 파일을 읽지 못했습니다: {error}"},
                    HTTPStatus.INTERNAL_SERVER_ERROR,
                )
                return
            self.send_response(HTTPStatus.OK)
            self.send_header("Content-Type", content_type)
            self.send_header("Content-Length", str(len(body)))
            self.send_header("Cache-Control", "no-store")
            self.end_headers()
            self.wfile.write(body)

        def do_POST(self) -> None:
            path = urlsplit(self.path).path
            try:
                payload = self._read_json()
                if path == "/api/frame":
                    state = controller.step(payload.get("edges", []))
                elif path == "/api/reset":
                    state = controller.reset(int(payload.get("seed", 1)))
                else:
                    self._send_json({"error": "경로를 찾을 수 없습니다."}, HTTPStatus.NOT_FOUND)
                    return
            except (TypeError, ValueError, RuntimeError) as error:
                self._send_json({"error": str(error)}, HTTPStatus.BAD_REQUEST)
                return
            self._send_json(state)

        def log_message(self, message: str, *args: object) -> None:
            print(f"human-battle: {message % args}", flush=True)

        def _read_json(self) -> dict[str, object]:
            length = int(self.headers.get("Content-Length", "0"))
            if length > 16_384:
                raise ValueError("request body is too large")
            if length == 0:
                return {}
            payload = json.loads(self.rfile.read(length))
            if not isinstance(payload, dict):
                raise ValueError("request body must be a JSON object")
            return payload

        def _send_json(
            self, payload: dict[str, object], status: HTTPStatus = HTTPStatus.OK
        ) -> None:
            body = json.dumps(payload, ensure_ascii=False, sort_keys=True).encode("utf-8")
            self.send_response(status)
            self.send_header("Content-Type", "application/json; charset=utf-8")
            self.send_header("Content-Length", str(len(body)))
            self.send_header("Cache-Control", "no-store")
            self.end_headers()
            self.wfile.write(body)

    return HumanBattleHandler


if __name__ == "__main__":
    main()
