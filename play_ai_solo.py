import os
import sys
import pygame
import time

sys.path.append(os.path.abspath(os.path.join(os.path.dirname(__file__), 'build')))
import tetrio_engine as te
from play_interactive import SHAPES, COLORS, draw_single_block, draw_piece_shape, BOARD_X, BOARD_Y, BLOCK_SIZE

WINDOW_WIDTH = 880
WINDOW_HEIGHT = 740

def run_ai_solo():
    pygame.init()
    pygame.font.init()
    screen = pygame.display.set_mode((WINDOW_WIDTH, WINDOW_HEIGHT))
    pygame.display.set_caption("TETR.io Engine - AI Solo Play Mode")
    clock = pygame.time.Clock()

    font_title = pygame.font.SysFont("Consolas", 24, bold=True)
    font_hud = pygame.font.SysFont("Consolas", 18, bold=True)
    font_action = pygame.font.SysFont("Consolas", 22, bold=True)

    player = te.TetrioPlayer(1337)

    ai_move_timer = 0
    ai_speed_ms = 150

    action_text = ""
    action_text_timer = 0

    running = True
    while running:
        dt = clock.tick(60)
        ai_move_timer += dt

        if action_text_timer > 0:
            action_text_timer -= dt

        for event in pygame.event.get():
            if event.type == pygame.QUIT:
                running = False
            elif event.type == pygame.KEYDOWN:
                if event.key == pygame.K_ESCAPE:
                    running = False
                elif event.key == pygame.K_r:
                    player.reset(1337)
                    action_text = "RESET AI GAME"
                    action_text_timer = 1000

        if not player.get_state().is_dead and ai_move_timer >= ai_speed_ms:
            ai_move_timer = 0
            placements = player.get_possible_placements()
            if placements:
                best_move = te.BeamSearchEngine.find_best_move(player, depth=3, beam_width=8)
                info = player.step(best_move)

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
                    action_text_timer = 1200

        screen.fill((15, 17, 26))

        board_rect = pygame.Rect(BOARD_X - 2, BOARD_Y - 2, BLOCK_SIZE * 10 + 4, BLOCK_SIZE * 20 + 4)
        pygame.draw.rect(screen, (80, 90, 120), board_rect, width=2)

        for y in range(20):
            for x in range(10):
                rect = pygame.Rect(BOARD_X + x * BLOCK_SIZE, BOARD_Y + y * BLOCK_SIZE, BLOCK_SIZE, BLOCK_SIZE)
                pygame.draw.rect(screen, (30, 35, 50), rect, width=1)

        board = player.get_board()
        for y in range(20):
            for x in range(10):
                cell_type = int(board.get_cell(x, y))
                if cell_type != 0:
                    cell_color = COLORS.get(cell_type, (150, 150, 160))
                    draw_single_block(surface=screen, grid_x=x, grid_y=y, color=cell_color)

        state = player.get_state()
        lbl_title = font_title.render("AI SOLO PLAY MODE", True, (255, 200, 50))
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

        lbl_c0 = font_hud.render("AI Mode Options:", True, (255, 200, 50))
        lbl_c1 = font_hud.render("R   : Reset Game", True, (200, 200, 210))
        lbl_c2 = font_hud.render("ESC : Exit", True, (200, 200, 210))

        screen.blit(lbl_c0, (40, 360))
        screen.blit(lbl_c1, (40, 390))
        screen.blit(lbl_c2, (40, 415))

        lbl_next = font_hud.render("NEXT", True, (200, 210, 230))
        screen.blit(lbl_next, (BOARD_X + BLOCK_SIZE * 10 + 40, 50))

        queue = player.get_queue(5)
        for i, piece in enumerate(queue):
            q_color = COLORS.get(int(piece), (100, 100, 100))
            lbl_q = font_hud.render(f"{i+1}. {piece.name}", True, q_color)
            screen.blit(lbl_q, (BOARD_X + BLOCK_SIZE * 10 + 40, 80 + i * 30))

        lbl_hold = font_hud.render("HOLD", True, (200, 210, 230))
        screen.blit(lbl_hold, (BOARD_X + BLOCK_SIZE * 10 + 40, 260))
        hold_piece = player.get_hold_piece()
        h_color = COLORS.get(int(hold_piece), (100, 100, 100))
        lbl_h_val = font_hud.render(f"{hold_piece.name}", True, h_color)
        screen.blit(lbl_h_val, (BOARD_X + BLOCK_SIZE * 10 + 40, 290))

        if action_text_timer > 0 and action_text:
            lbl_act = font_action.render(action_text, True, (255, 220, 50))
            screen.blit(lbl_act, (BOARD_X, BOARD_Y + BLOCK_SIZE * 20 + 15))

        if state.is_dead:
            lbl_dead = font_title.render("TOP OUT! (GAME OVER)", True, (255, 50, 50))
            screen.blit(lbl_dead, (BOARD_X, BOARD_Y + 250))

        pygame.display.flip()

    pygame.quit()

if __name__ == "__main__":
    run_ai_solo()
