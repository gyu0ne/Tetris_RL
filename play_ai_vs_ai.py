import os
import sys
import pygame
import torch

sys.path.append(os.path.abspath(os.path.join(os.path.dirname(__file__), 'build')))
import tetrio_engine as te
from play_interactive import COLORS

BLOCK_SIZE = 24
WINDOW_WIDTH = 1000
WINDOW_HEIGHT = 700

P1_BOARD_X = 140
P2_BOARD_X = 600
BOARD_Y = 80

def draw_player_board(screen, player, board_x, font_hud, title_name="AI BOT"):
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

    state = player.get_state()
    lbl_title = font_hud.render(title_name, True, (255, 200, 50))
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

def run_ai_vs_ai():
    pygame.init()
    pygame.font.init()
    screen = pygame.display.set_mode((WINDOW_WIDTH, WINDOW_HEIGHT))
    pygame.display.set_caption("TETR.io 1v1 Battle - AI vs AI Competition Mode")
    clock = pygame.time.Clock()

    font_title = pygame.font.SysFont("Consolas", 24, bold=True)
    font_hud = pygame.font.SysFont("Consolas", 15, bold=True)
    font_banner = pygame.font.SysFont("Consolas", 28, bold=True)

    game = te.TetrioVsGame(1337, 4242)

    ai_move_timer = 0
    ai_speed_ms = 180

    running = True
    while running:
        delta = clock.tick(60)
        ai_move_timer += delta

        p1 = game.get_p1()
        p2 = game.get_p2()

        for event in pygame.event.get():
            if event.type == pygame.QUIT:
                running = False
            elif event.type == pygame.KEYDOWN:
                if event.key == pygame.K_ESCAPE:
                    running = False
                elif event.key == pygame.K_r:
                    game.reset(1337, 4242)

        if not game.is_game_over() and ai_move_timer >= ai_speed_ms:
            ai_move_timer = 0

            placements1 = p1.get_possible_placements()
            if placements1:
                best_action1 = te.BeamSearchEngine.find_best_move(p1, depth=3, beam_width=8)
                game.step_p1(best_action1)

            placements2 = p2.get_possible_placements()
            if placements2:
                best_action2 = te.BeamSearchEngine.find_best_move(p2, depth=3, beam_width=8)
                game.step_p2(best_action2)

        screen.fill((15, 17, 26))

        lbl_header = font_title.render("TETR.IO AI VS AI BATTLE COMPETITION", True, (240, 240, 250))
        screen.blit(lbl_header, (WINDOW_WIDTH // 2 - 220, 20))

        draw_player_board(screen, p1, P1_BOARD_X, font_hud, title_name="AI BOT 1 (SADRL)")
        draw_player_board(screen, p2, P2_BOARD_X, font_hud, title_name="AI BOT 2 (BASELINE)")

        lbl_ctrl = font_hud.render("Press R: Reset Game | ESC: Exit", True, (150, 160, 180))
        screen.blit(lbl_ctrl, (WINDOW_WIDTH // 2 - 120, WINDOW_HEIGHT - 35))

        winner = game.get_winner()
        if winner != 0:
            winner_str = "AI BOT 1 WINS!" if winner == 1 else "AI BOT 2 WINS!"
            color = (100, 220, 255) if winner == 1 else (255, 100, 100)
            lbl_win = font_banner.render(winner_str, True, color)
            screen.blit(lbl_win, (WINDOW_WIDTH // 2 - 120, WINDOW_HEIGHT // 2 - 20))

        pygame.display.flip()

    pygame.quit()

if __name__ == "__main__":
    run_ai_vs_ai()
