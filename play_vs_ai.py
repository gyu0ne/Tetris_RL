import os
import sys
import pygame
import time
import random

sys.path.append(os.path.abspath(os.path.join(os.path.dirname(__file__), 'build')))
import tetrio_engine as te
from play_interactive import SHAPES, COLORS, draw_single_block, draw_piece_shape

BLOCK_SIZE = 24
WINDOW_WIDTH = 1000
WINDOW_HEIGHT = 720

P1_BOARD_X = 140
P2_BOARD_X = 600
BOARD_Y = 70

def draw_player_side(screen, player, board_x, font_hud, is_ai=False, active_piece=None, active_pos=None):
    board_rect = pygame.Rect(board_x - 2, BOARD_Y - 2, BLOCK_SIZE * 10 + 4, BLOCK_SIZE * 20 + 4)
    pygame.draw.rect(screen, (80, 90, 120), board_rect, width=2)

    for y in range(20):
        for x in range(10):
            rect = pygame.Rect(board_x + x * BLOCK_SIZE, BOARD_Y + y * BLOCK_SIZE, BLOCK_SIZE, BLOCK_SIZE)
            pygame.draw.rect(screen, (30, 35, 50), rect, width=1)

    board = player.get_board()
    for y in range(20):
        for x in range(10):
            cell_type = int(board.get_cell(x, y))
            if cell_type != 0:
                px = board_x + x * BLOCK_SIZE
                py = BOARD_Y + (19 - y) * BLOCK_SIZE
                rect = pygame.Rect(px + 1, py + 1, BLOCK_SIZE - 2, BLOCK_SIZE - 2)
                color = COLORS.get(cell_type, (150, 150, 160))
                pygame.draw.rect(screen, color, rect)
                pygame.draw.rect(screen, (255, 255, 255), rect, width=1)

    if not is_ai and active_piece and active_pos and not player.get_state().is_dead:
        curr_piece, curr_rot, curr_x, curr_y = active_piece[0], active_piece[1], active_pos[0], active_pos[1]
        if curr_piece != te.PieceType.NONE:
            ghost_y = curr_y
            while board.can_place(curr_piece, curr_rot, curr_x, ghost_y - 1):
                ghost_y -= 1

            piece_color = COLORS.get(int(curr_piece), (200, 200, 200))

            if curr_piece in SHAPES and curr_rot in SHAPES[curr_piece]:
                for dx, dy in SHAPES[curr_piece][curr_rot]:
                    gx, gy = curr_x + dx, ghost_y + dy
                    if 0 <= gx < 10 and 0 <= gy < 20:
                        px = board_x + gx * BLOCK_SIZE
                        py = BOARD_Y + (19 - gy) * BLOCK_SIZE
                        rect = pygame.Rect(px + 2, py + 2, BLOCK_SIZE - 4, BLOCK_SIZE - 4)
                        pygame.draw.rect(screen, (120, 130, 160), rect, width=2)

            if curr_piece in SHAPES and curr_rot in SHAPES[curr_piece]:
                for dx, dy in SHAPES[curr_piece][curr_rot]:
                    ax, ay = curr_x + dx, curr_y + dy
                    if 0 <= ax < 10 and 0 <= ay < 20:
                        px = board_x + ax * BLOCK_SIZE
                        py = BOARD_Y + (19 - ay) * BLOCK_SIZE
                        rect = pygame.Rect(px + 1, py + 1, BLOCK_SIZE - 2, BLOCK_SIZE - 2)
                        pygame.draw.rect(screen, piece_color, rect)
                        pygame.draw.rect(screen, (255, 255, 255), rect, width=1)

    state = player.get_state()
    lbl_title = font_hud.render("AI BOT" if is_ai else "HUMAN PLAYER", True, (255, 200, 50) if is_ai else (100, 220, 255))
    lbl_attack = font_hud.render(f"Attack: {state.total_attack_sent}", True, (240, 240, 250))
    lbl_b2b = font_hud.render(f"B2B Lv: {state.b2b_level}", True, (255, 100, 200))
    lbl_combo = font_hud.render(f"Combo : {state.combo}", True, (100, 220, 255))

    screen.blit(lbl_title, (board_x, BOARD_Y - 35))
    screen.blit(lbl_attack, (board_x + BLOCK_SIZE * 10 + 15, BOARD_Y))
    screen.blit(lbl_b2b, (board_x + BLOCK_SIZE * 10 + 15, BOARD_Y + 25))
    screen.blit(lbl_combo, (board_x + BLOCK_SIZE * 10 + 15, BOARD_Y + 50))

    lbl_hold = font_hud.render(f"HOLD: {player.get_hold_piece().name}", True, (200, 200, 220))
    screen.blit(lbl_hold, (board_x + BLOCK_SIZE * 10 + 15, BOARD_Y + 90))

    queue = player.get_queue(3)
    lbl_next_hdr = font_hud.render("NEXT:", True, (200, 200, 220))
    screen.blit(lbl_next_hdr, (board_x + BLOCK_SIZE * 10 + 15, BOARD_Y + 125))
    for i, piece in enumerate(queue):
        lbl_q = font_hud.render(f"{i+1}. {piece.name}", True, COLORS.get(int(piece), (150, 150, 150)))
        screen.blit(lbl_q, (board_x + BLOCK_SIZE * 10 + 15, BOARD_Y + 150 + i * 22))

def run_vs_ai_full():
    pygame.init()
    pygame.font.init()
    screen = pygame.display.set_mode((WINDOW_WIDTH, WINDOW_HEIGHT))
    pygame.display.set_caption("TETR.io 1v1 Battle - Full Mechanics (Dynamic 7-Bag)")
    clock = pygame.time.Clock()

    font_title = pygame.font.SysFont("Consolas", 24, bold=True)
    font_hud = pygame.font.SysFont("Consolas", 15, bold=True)
    font_banner = pygame.font.SysFont("Consolas", 28, bold=True)

    seed1 = random.randint(1, 1000000000)
    seed2 = random.randint(1, 1000000000)
    game = te.TetrioVsGame(seed1, seed2)
    p1 = game.get_p1()
    p2 = game.get_p2()

    DAS_MS = 133
    GRAVITY_MS = 800
    LOCK_DELAY_MS = 500

    curr_piece = p1.get_current_piece()
    curr_x = 4 if curr_piece == te.PieceType.O else 3
    curr_y = 19
    curr_rot = te.Rotation.SPAWN
    last_move = te.MoveType.NORMAL
    last_kick_idx = 0

    j_pressed_time = 0
    o_pressed_time = 0
    gravity_timer = time.time() * 1000
    lock_timer = None
    lock_resets = 0

    def reset_human_piece():
        nonlocal curr_piece, curr_x, curr_y, curr_rot, last_move, last_kick_idx, lock_timer, lock_resets
        curr_piece = p1.get_current_piece()
        curr_x = 4 if curr_piece == te.PieceType.O else 3
        curr_y = 19
        curr_rot = te.Rotation.SPAWN
        last_move = te.MoveType.NORMAL
        last_kick_idx = 0
        lock_timer = None
        lock_resets = 0

    reset_human_piece()

    ai_move_timer = 0
    ai_speed_ms = 300

    running = True
    while running:
        dt = clock.tick(60)
        now_ms = time.time() * 1000
        ai_move_timer += dt

        board_p1 = p1.get_board()

        for event in pygame.event.get():
            if event.type == pygame.QUIT:
                running = False
            elif event.type == pygame.KEYDOWN:
                if event.key == pygame.K_ESCAPE:
                    running = False
                elif event.key == pygame.K_r:
                    s1 = random.randint(1, 1000000000)
                    s2 = random.randint(1, 1000000000)
                    game.reset(s1, s2)
                    reset_human_piece()

                if p1.get_state().is_dead or game.is_game_over():
                    continue

                # Left Move (J)
                if event.key == pygame.K_j:
                    j_pressed_time = now_ms
                    if board_p1.can_place(curr_piece, curr_rot, curr_x - 1, curr_y):
                        curr_x -= 1
                        last_move = te.MoveType.NORMAL
                        if lock_resets < 15:
                            lock_timer = now_ms; lock_resets += 1

                # Right Move (O)
                elif event.key == pygame.K_o:
                    o_pressed_time = now_ms
                    if board_p1.can_place(curr_piece, curr_rot, curr_x + 1, curr_y):
                        curr_x += 1
                        last_move = te.MoveType.NORMAL
                        if lock_resets < 15:
                            lock_timer = now_ms; lock_resets += 1

                # Soft Drop (I)
                elif event.key == pygame.K_i:
                    if board_p1.can_place(curr_piece, curr_rot, curr_x, curr_y - 1):
                        curr_y -= 1
                        last_move = te.MoveType.NORMAL

                # CW Rotate (W) with SRS+ Wall Kicks
                elif event.key == pygame.K_w:
                    next_rot = te.Rotation((int(curr_rot) + 1) % 4)
                    res = te.SRSPlus.try_rotate(board_p1, curr_piece, curr_rot, next_rot, curr_x, curr_y)
                    if res.success:
                        curr_x = res.new_x
                        curr_y = res.new_y
                        curr_rot = res.new_rot
                        last_move = te.MoveType.CW
                        last_kick_idx = res.kick_index
                        if lock_resets < 15:
                            lock_timer = now_ms; lock_resets += 1

                # CCW Rotate (Q) with SRS+ Wall Kicks
                elif event.key == pygame.K_q:
                    next_rot = te.Rotation((int(curr_rot) + 3) % 4)
                    res = te.SRSPlus.try_rotate(board_p1, curr_piece, curr_rot, next_rot, curr_x, curr_y)
                    if res.success:
                        curr_x = res.new_x
                        curr_y = res.new_y
                        curr_rot = res.new_rot
                        last_move = te.MoveType.CCW
                        last_kick_idx = res.kick_index
                        if lock_resets < 15:
                            lock_timer = now_ms; lock_resets += 1

                # Hold (D)
                elif event.key == pygame.K_d:
                    if p1.hold():
                        reset_human_piece()

                # Hard Drop (Space)
                elif event.key == pygame.K_SPACE:
                    drop_y = curr_y
                    while board_p1.can_place(curr_piece, curr_rot, curr_x, drop_y - 1):
                        drop_y -= 1

                    placement = te.Placement(curr_x, drop_y, curr_rot, False, te.SpinType.NONE, last_move, last_kick_idx)
                    game.step_p1(placement)
                    reset_human_piece()

            elif event.type == pygame.KEYUP:
                if event.key == pygame.K_j:
                    j_pressed_time = 0
                elif event.key == pygame.K_o:
                    o_pressed_time = 0

        # Auto-Repeat & Gravity
        keys = pygame.key.get_pressed()
        if not p1.get_state().is_dead and not game.is_game_over():
            if keys[pygame.K_j] and j_pressed_time > 0 and (now_ms - j_pressed_time >= DAS_MS):
                while board_p1.can_place(curr_piece, curr_rot, curr_x - 1, curr_y):
                    curr_x -= 1
            if keys[pygame.K_o] and o_pressed_time > 0 and (now_ms - o_pressed_time >= DAS_MS):
                while board_p1.can_place(curr_piece, curr_rot, curr_x + 1, curr_y):
                    curr_x += 1
            if keys[pygame.K_i]:
                if board_p1.can_place(curr_piece, curr_rot, curr_x, curr_y - 1):
                    curr_y -= 1

            if now_ms - gravity_timer >= GRAVITY_MS:
                gravity_timer = now_ms
                if board_p1.can_place(curr_piece, curr_rot, curr_x, curr_y - 1):
                    curr_y -= 1

            if not board_p1.can_place(curr_piece, curr_rot, curr_x, curr_y - 1):
                if lock_timer is None:
                    lock_timer = now_ms
                elif now_ms - lock_timer >= LOCK_DELAY_MS:
                    placement = te.Placement(curr_x, curr_y, curr_rot, False, te.SpinType.NONE, last_move, last_kick_idx)
                    game.step_p1(placement)
                    reset_human_piece()
            else:
                lock_timer = None

        # AI Move Logic
        if not game.is_game_over() and ai_move_timer >= ai_speed_ms:
            ai_move_timer = 0
            p2_placements = p2.get_possible_placements()
            if p2_placements:
                best_action = te.BeamSearchEngine.find_best_move(p2, depth=3, beam_width=8)
                game.step_p2(best_action)

        # Render Screen
        screen.fill((15, 17, 26))

        lbl_header = font_title.render("TETR.IO 1VS1 BATTLE SIMULATOR", True, (240, 240, 250))
        screen.blit(lbl_header, (WINDOW_WIDTH // 2 - 200, 20))

        draw_player_side(
            screen, p1, P1_BOARD_X, font_hud, is_ai=False,
            active_piece=(curr_piece, curr_rot), active_pos=(curr_x, curr_y)
        )

        draw_player_side(screen, p2, P2_BOARD_X, font_hud, is_ai=True)

        lbl_ctrl = font_hud.render("KEYS: Left/Right (J/O) | SoftDrop (I) | HardDrop (Space) | Hold (D) | Rotate CW/CCW (W/Q)", True, (150, 160, 180))
        screen.blit(lbl_ctrl, (40, WINDOW_HEIGHT - 35))

        winner = game.get_winner()
        if winner != 0:
            winner_str = "HUMAN PLAYER WINS!" if winner == 1 else "BOT AI WINS!"
            color = (100, 220, 255) if winner == 1 else (255, 100, 100)
            lbl_win = font_banner.render(winner_str, True, color)
            screen.blit(lbl_win, (WINDOW_WIDTH // 2 - 130, WINDOW_HEIGHT // 2 - 20))

        pygame.display.flip()

    pygame.quit()

if __name__ == "__main__":
    run_vs_ai_full()
