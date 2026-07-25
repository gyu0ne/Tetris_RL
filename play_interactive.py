import os
import sys
import pygame
import time
import random

sys.path.append(os.path.abspath(os.path.join(os.path.dirname(__file__), 'build')))
import tetrio_engine as te

# Exact Piece & Garbage Color Map (Matching TETR.io Official)
# 0: NONE, 1: I, 2: J, 3: L, 4: O, 5: S, 6: T, 7: Z, 8: GARBAGE
COLORS = {
    0: (15, 17, 26),        # Empty Background
    1: (59, 230, 210),      # Cyan (I)
    2: (51, 102, 230),      # Blue (J)
    3: (230, 140, 40),      # Orange (L)
    4: (230, 200, 40),      # Yellow (O)
    5: (60, 210, 80),       # Green (S)
    6: (180, 70, 230),      # Purple (T)
    7: (230, 60, 80),       # Red (Z)
    8: (100, 105, 120),     # Metal Dark Gray (Garbage)
}

SHAPES = {
    te.PieceType.I: {
        te.Rotation.SPAWN: [(-1, 0), (0, 0), (1, 0), (2, 0)],
        te.Rotation.RIGHT: [(1, 1), (1, 0), (1, -1), (1, -2)],
        te.Rotation.REVERSE: [(-1, -1), (0, -1), (1, -1), (2, -1)],
        te.Rotation.LEFT: [(0, 1), (0, 0), (0, -1), (0, -2)],
    },
    te.PieceType.J: {
        te.Rotation.SPAWN: [(-1, 1), (-1, 0), (0, 0), (1, 0)],
        te.Rotation.RIGHT: [(0, 1), (1, 1), (0, 0), (0, -1)],
        te.Rotation.REVERSE: [(-1, 0), (0, 0), (1, 0), (1, -1)],
        te.Rotation.LEFT: [(0, 1), (0, 0), (0, -1), (-1, -1)],
    },
    te.PieceType.L: {
        te.Rotation.SPAWN: [(1, 1), (-1, 0), (0, 0), (1, 0)],
        te.Rotation.RIGHT: [(0, 1), (0, 0), (0, -1), (1, -1)],
        te.Rotation.REVERSE: [(-1, 0), (0, 0), (1, 0), (-1, -1)],
        te.Rotation.LEFT: [(-1, 1), (0, 1), (0, 0), (0, -1)],
    },
    te.PieceType.O: {
        te.Rotation.SPAWN: [(0, 1), (1, 1), (0, 0), (1, 0)],
        te.Rotation.RIGHT: [(0, 1), (1, 1), (0, 0), (1, 0)],
        te.Rotation.REVERSE: [(0, 1), (1, 1), (0, 0), (1, 0)],
        te.Rotation.LEFT: [(0, 1), (1, 1), (0, 0), (1, 0)],
    },
    te.PieceType.S: {
        te.Rotation.SPAWN: [(0, 1), (1, 1), (-1, 0), (0, 0)],
        te.Rotation.RIGHT: [(0, 1), (0, 0), (1, 0), (1, -1)],
        te.Rotation.REVERSE: [(0, 0), (1, 0), (-1, -1), (0, -1)],
        te.Rotation.LEFT: [(-1, 1), (-1, 0), (0, 0), (0, -1)],
    },
    te.PieceType.T: {
        te.Rotation.SPAWN: [(0, 1), (-1, 0), (0, 0), (1, 0)],
        te.Rotation.RIGHT: [(0, 1), (0, 0), (1, 0), (0, -1)],
        te.Rotation.REVERSE: [(-1, 0), (0, 0), (1, 0), (0, -1)],
        te.Rotation.LEFT: [(0, 1), (-1, 0), (0, 0), (0, -1)],
    },
    te.PieceType.Z: {
        te.Rotation.SPAWN: [(-1, 1), (0, 1), (0, 0), (1, 0)],
        te.Rotation.RIGHT: [(1, 1), (0, 0), (1, 0), (0, -1)],
        te.Rotation.REVERSE: [(-1, 0), (0, 0), (0, -1), (1, -1)],
        te.Rotation.LEFT: [(0, 1), (-1, 0), (0, 0), (-1, -1)],
    },
}

GRID_LINE_COLOR = (30, 35, 50)
BLOCK_SIZE = 30
BOARD_X = 260
BOARD_Y = 50

WINDOW_WIDTH = 880
WINDOW_HEIGHT = 740

def draw_single_block(surface, grid_x, grid_y, color, is_ghost=False):
    if grid_x < 0 or grid_x >= 10 or grid_y < 0 or grid_y >= 20:
        return
    px = BOARD_X + grid_x * BLOCK_SIZE
    py = BOARD_Y + (19 - grid_y) * BLOCK_SIZE

    if is_ghost:
        rect = pygame.Rect(px + 2, py + 2, BLOCK_SIZE - 4, BLOCK_SIZE - 4)
        pygame.draw.rect(surface, (120, 130, 160), rect, width=2)
    else:
        rect = pygame.Rect(px + 1, py + 1, BLOCK_SIZE - 2, BLOCK_SIZE - 2)
        pygame.draw.rect(surface, color, rect)
        pygame.draw.rect(surface, (255, 255, 255), rect, width=1)

def draw_piece_shape(surface, piece_type, rot, grid_x, grid_y, color, is_ghost=False):
    if piece_type not in SHAPES or rot not in SHAPES[piece_type]:
        return
    for dx, dy in SHAPES[piece_type][rot]:
        draw_single_block(surface, grid_x + dx, grid_y + dy, color, is_ghost=is_ghost)

def run_tetrio_full_gui():
    pygame.init()
    pygame.font.init()
    screen = pygame.display.set_mode((WINDOW_WIDTH, WINDOW_HEIGHT))
    pygame.display.set_caption("TETR.io Engine (Dynamic 7-Bag & SRS+ Wall Kicks)")
    clock = pygame.time.Clock()

    font_title = pygame.font.SysFont("Consolas", 24, bold=True)
    font_hud = pygame.font.SysFont("Consolas", 18, bold=True)
    font_action = pygame.font.SysFont("Consolas", 22, bold=True)

    # Dynamic random seed for each new game!
    init_seed = random.randint(1, 1000000000)
    player = te.TetrioPlayer(init_seed)

    DAS_MS = 133
    GRAVITY_MS = 800
    LOCK_DELAY_MS = 500

    curr_piece = player.get_current_piece()
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

    action_text = ""
    action_text_timer = 0

    def reset_active_piece():
        nonlocal curr_piece, curr_x, curr_y, curr_rot, last_move, last_kick_idx, lock_timer, lock_resets
        curr_piece = player.get_current_piece()
        curr_x = 4 if curr_piece == te.PieceType.O else 3
        curr_y = 19
        curr_rot = te.Rotation.SPAWN
        last_move = te.MoveType.NORMAL
        last_kick_idx = 0
        lock_timer = None
        lock_resets = 0

    reset_active_piece()

    running = True
    while running:
        dt = clock.tick(60)
        now_ms = time.time() * 1000

        if action_text_timer > 0:
            action_text_timer -= dt

        board = player.get_board()

        for event in pygame.event.get():
            if event.type == pygame.QUIT:
                running = False
            elif event.type == pygame.KEYDOWN:
                if event.key == pygame.K_ESCAPE:
                    running = False
                elif event.key == pygame.K_r:
                    new_seed = random.randint(1, 1000000000)
                    player.reset(new_seed)
                    reset_active_piece()
                    action_text = "RESET NEW SEED GAME"
                    action_text_timer = 1000

                if player.get_state().is_dead:
                    continue

                # Left Move (J)
                if event.key == pygame.K_j:
                    j_pressed_time = now_ms
                    if board.can_place(curr_piece, curr_rot, curr_x - 1, curr_y):
                        curr_x -= 1
                        last_move = te.MoveType.NORMAL
                        if lock_resets < 15:
                            lock_timer = now_ms; lock_resets += 1

                # Right Move (O)
                elif event.key == pygame.K_o:
                    o_pressed_time = now_ms
                    if board.can_place(curr_piece, curr_rot, curr_x + 1, curr_y):
                        curr_x += 1
                        last_move = te.MoveType.NORMAL
                        if lock_resets < 15:
                            lock_timer = now_ms; lock_resets += 1

                # Soft Drop (I)
                elif event.key == pygame.K_i:
                    if board.can_place(curr_piece, curr_rot, curr_x, curr_y - 1):
                        curr_y -= 1
                        last_move = te.MoveType.NORMAL

                # CW Rotate (W) with Full SRS+ Wall Kicks
                elif event.key == pygame.K_w:
                    next_rot = te.Rotation((int(curr_rot) + 1) % 4)
                    res = te.SRSPlus.try_rotate(board, curr_piece, curr_rot, next_rot, curr_x, curr_y)
                    if res.success:
                        curr_x = res.new_x
                        curr_y = res.new_y
                        curr_rot = res.new_rot
                        last_move = te.MoveType.CW
                        last_kick_idx = res.kick_index
                        if lock_resets < 15:
                            lock_timer = now_ms; lock_resets += 1

                # CCW Rotate (Q) with Full SRS+ Wall Kicks
                elif event.key == pygame.K_q:
                    next_rot = te.Rotation((int(curr_rot) + 3) % 4)
                    res = te.SRSPlus.try_rotate(board, curr_piece, curr_rot, next_rot, curr_x, curr_y)
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
                    if player.hold():
                        reset_active_piece()

                # Hard Drop (Space)
                elif event.key == pygame.K_SPACE:
                    drop_y = curr_y
                    while board.can_place(curr_piece, curr_rot, curr_x, drop_y - 1):
                        drop_y -= 1

                    placement = te.Placement(curr_x, drop_y, curr_rot, False, te.SpinType.NONE, last_move, last_kick_idx)
                    info = player.step(placement)

                    if info.spin != te.SpinType.NONE:
                        spin_name = "T-SPIN MINI " if info.spin == te.SpinType.MINI else "T-SPIN "
                        lines_map = {1: "SINGLE", 2: "DOUBLE", 3: "TRIPLE"}
                        action_text = f"{spin_name}{lines_map.get(info.lines_cleared, '')}"
                    elif info.lines_cleared == 4:
                        action_text = "QUAD (TETRIS)!"
                    elif info.lines_cleared > 0:
                        lines_map = {1: "SINGLE", 2: "DOUBLE", 3: "TRIPLE"}
                        action_text = f"{lines_map.get(info.lines_cleared, '')}"
                    else:
                        action_text = ""

                    if info.b2b_bonus > 0:
                        action_text += f" (B2B Lv.{info.b2b_level_after})"
                    if info.all_clear:
                        action_text = "PERFECT CLEAR (+10)!"

                    if action_text:
                        action_text_timer = 1500

                    reset_active_piece()

            elif event.type == pygame.KEYUP:
                if event.key == pygame.K_j:
                    j_pressed_time = 0
                elif event.key == pygame.K_o:
                    o_pressed_time = 0

        # Auto-Repeat & Gravity
        keys = pygame.key.get_pressed()
        if not player.get_state().is_dead:
            if keys[pygame.K_j] and j_pressed_time > 0 and (now_ms - j_pressed_time >= DAS_MS):
                while board.can_place(curr_piece, curr_rot, curr_x - 1, curr_y):
                    curr_x -= 1
            if keys[pygame.K_o] and o_pressed_time > 0 and (now_ms - o_pressed_time >= DAS_MS):
                while board.can_place(curr_piece, curr_rot, curr_x + 1, curr_y):
                    curr_x += 1
            if keys[pygame.K_i]:
                if board.can_place(curr_piece, curr_rot, curr_x, curr_y - 1):
                    curr_y -= 1

            if now_ms - gravity_timer >= GRAVITY_MS:
                gravity_timer = now_ms
                if board.can_place(curr_piece, curr_rot, curr_x, curr_y - 1):
                    curr_y -= 1

            is_grounded = not board.can_place(curr_piece, curr_rot, curr_x, curr_y - 1)
            if is_grounded:
                if lock_timer is None:
                    lock_timer = now_ms
                elif now_ms - lock_timer >= LOCK_DELAY_MS:
                    placement = te.Placement(curr_x, curr_y, curr_rot, False, te.SpinType.NONE, last_move, last_kick_idx)
                    info = player.step(placement)
                    reset_active_piece()
            else:
                lock_timer = None

        # Render Frame
        screen.fill((15, 17, 26))

        # Board Frame & Grid Lines
        board_rect = pygame.Rect(BOARD_X - 2, BOARD_Y - 2, BLOCK_SIZE * 10 + 4, BLOCK_SIZE * 20 + 4)
        pygame.draw.rect(screen, (80, 90, 120), board_rect, width=2)

        for y in range(20):
            for x in range(10):
                rect = pygame.Rect(BOARD_X + x * BLOCK_SIZE, BOARD_Y + y * BLOCK_SIZE, BLOCK_SIZE, BLOCK_SIZE)
                pygame.draw.rect(screen, GRID_LINE_COLOR, rect, width=1)

        # Draw Per-Piece & Garbage Colored Locked Blocks
        for y in range(20):
            for x in range(10):
                cell_type = int(board.get_cell(x, y))
                if cell_type != 0:
                    cell_color = COLORS.get(cell_type, (150, 150, 160))
                    draw_single_block(surface=screen, grid_x=x, grid_y=y, color=cell_color)

        # Active & Ghost Piece Rendering
        if not player.get_state().is_dead and curr_piece != te.PieceType.NONE:
            ghost_y = curr_y
            while board.can_place(curr_piece, curr_rot, curr_x, ghost_y - 1):
                ghost_y -= 1

            piece_color = COLORS.get(int(curr_piece), (200, 200, 200))
            draw_piece_shape(screen, curr_piece, curr_rot, curr_x, ghost_y, piece_color, is_ghost=True)
            draw_piece_shape(screen, curr_piece, curr_rot, curr_x, curr_y, piece_color, is_ghost=False)

        # HUD Stats
        state = player.get_state()
        lbl_title = font_title.render("TETR.io Engine (Dynamic 7-Bag Shuffling)", True, (240, 240, 250))
        screen.blit(lbl_title, (40, 15))

        lbl_lines = font_hud.render(f"Lines : {state.total_lines_cleared}", True, (200, 210, 230))
        lbl_attack = font_hud.render(f"Attack: {state.total_attack_sent}", True, (255, 200, 50))
        lbl_b2b = font_hud.render(f"B2B Lv: {state.b2b_level}", True, (255, 100, 200))
        lbl_combo = font_hud.render(f"Combo : {state.combo}", True, (100, 220, 255))
        lbl_pieces = font_hud.render(f"Pieces: {state.total_pieces_placed}", True, (180, 180, 190))

        screen.blit(lbl_lines, (40, 60))
        screen.blit(lbl_attack, (40, 90))
        screen.blit(lbl_b2b, (40, 120))
        screen.blit(lbl_combo, (40, 150))
        screen.blit(lbl_pieces, (40, 180))

        # Controls Guide
        lbl_c0 = font_hud.render("Customized Controls:", True, (255, 200, 50))
        lbl_c1 = font_hud.render("Left / Right     : J / O", True, (200, 200, 210))
        lbl_c2 = font_hud.render("Soft Drop        : I", True, (200, 200, 210))
        lbl_c3 = font_hud.render("Hard Drop        : Space", True, (200, 200, 210))
        lbl_c4 = font_hud.render("Hold             : D", True, (200, 200, 210))
        lbl_c5 = font_hud.render("Clockwise (+90)  : W", True, (200, 200, 210))
        lbl_c6 = font_hud.render("Counter-CW (-90) : Q", True, (200, 200, 210))
        lbl_c7 = font_hud.render("Reset Game       : R", True, (200, 200, 210))

        screen.blit(lbl_c0, (40, 340))
        screen.blit(lbl_c1, (40, 370))
        screen.blit(lbl_c2, (40, 395))
        screen.blit(lbl_c3, (40, 420))
        screen.blit(lbl_c4, (40, 445))
        screen.blit(lbl_c5, (40, 470))
        screen.blit(lbl_c6, (40, 495))
        screen.blit(lbl_c7, (40, 520))

        # Next Queue
        lbl_next = font_hud.render("NEXT", True, (200, 210, 230))
        screen.blit(lbl_next, (BOARD_X + BLOCK_SIZE * 10 + 40, 50))

        queue = player.get_queue(5)
        for i, piece in enumerate(queue):
            q_color = COLORS.get(int(piece), (100, 100, 100))
            lbl_q = font_hud.render(f"{i+1}. {piece.name}", True, q_color)
            screen.blit(lbl_q, (BOARD_X + BLOCK_SIZE * 10 + 40, 80 + i * 30))

        # Hold Piece
        lbl_hold = font_hud.render("HOLD", True, (200, 210, 230))
        screen.blit(lbl_hold, (BOARD_X + BLOCK_SIZE * 10 + 40, 260))
        hold_piece = player.get_hold_piece()
        h_color = COLORS.get(int(hold_piece), (100, 100, 100))
        lbl_h_val = font_hud.render(f"{hold_piece.name}", True, h_color)
        screen.blit(lbl_h_val, (BOARD_X + BLOCK_SIZE * 10 + 40, 290))

        # Action Banner
        if action_text_timer > 0 and action_text:
            lbl_act = font_action.render(action_text, True, (255, 220, 50))
            screen.blit(lbl_act, (BOARD_X, BOARD_Y + BLOCK_SIZE * 20 + 15))

        if state.is_dead:
            lbl_dead = font_title.render("TOP OUT! (GAME OVER)", True, (255, 50, 50))
            screen.blit(lbl_dead, (BOARD_X, BOARD_Y + 250))

        pygame.display.flip()

    pygame.quit()

if __name__ == "__main__":
    run_tetrio_full_gui()
