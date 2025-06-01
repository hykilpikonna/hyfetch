import math
from time import sleep

from hyfetch import presets
from hyfetch.color_util import RGB, color, printc
from hyfetch.constants import IS_WINDOWS
from hyfetch.neofetch_util import term_size
from hyfetch.presets import PRESETS


def key_pressed():
    if IS_WINDOWS:
        import msvcrt
        return msvcrt.kbhit()  # Non-blocking check for key press
    else:
        import select
        import sys
        return select.select([sys.stdin], [], [], 0) == ([sys.stdin], [], [])


def start_animation():
    text = r"""
.======================================================.
| .  .              .__       .     .  .       , .   | |
| |__| _.._ ._   .  [__)._.* _| _   |\/| _ ._ -+-|_  | |
| |  |(_][_)[_)\_|  |   [  |(_](/,  |  |(_)[ ) | [ ) * |
|        |  |  ._|                                     |
'======================================================'""".strip("\n")
    text_lines = text.split("\n")
    text_height = len(text_lines)
    text_width = len(text_lines[0])

    notice = "Press enter to continue"

    speed = 2
    frame_delay = 1 / 25

    colors: list[RGB] = []
    frame = 0

    w, h = term_size()
    blocks = 9
    block_width = w // blocks

    text_start_y = h // 2 - text_height // 2
    text_end_y = text_start_y + text_height
    text_start_x = w // 2 - text_width // 2
    text_end_x = text_start_x + text_width

    notice_start_x = w - len(notice) - 1
    notice_end_x = w - 1
    notice_y = h - 1

    # Add everything in PRESETS to colors
    colors = [c for preset in PRESETS.values() for c in preset.colors]

    black = RGB(0, 0, 0)
    fg = RGB.from_hex("#FFE09B")

    def draw_frame():
        buf = ""

        # Loop over the height
        for y in range(h):
            # Print the starting color
            buf += colors[((frame + y) // block_width) % len(colors)].to_ansi_rgb(foreground=False)
            buf += fg.to_ansi_8bit(foreground=True)

            # Loop over the width
            x = 0
            while x < w:
                idx = frame + x + y + int(math.sin(y + 0.5 * frame) * 2)
                y_text = text_start_y <= y < text_end_y

                border = 1 + int(not (y == text_start_y or y == text_end_y - 1))

                # If it's a switching point
                if idx % block_width == 0 or x == text_start_x - border or x == text_end_x + border \
                        or x == notice_start_x - 1 or x == notice_end_x + 1:
                    # Print the color at the current frame
                    c = colors[(idx // block_width) % len(colors)]
                    if (y_text and text_start_x - border <= x < text_end_x + border) \
                            or (y == notice_y and notice_start_x - 1 <= x < notice_end_x + 1):
                        # buf += c.set_light(0.3).to_ansi_8bit(foreground=False)
                        buf += c.overlay(black, 0.5).to_ansi_8bit(foreground=False)
                    else:
                        buf += c.to_ansi_8bit(foreground=False)

                # If text should be printed, print text
                if y_text and text_start_x <= x < text_end_x:
                    # Add white background
                    buf += text_lines[y - text_start_y][x - text_start_x]
                elif y == notice_y and notice_start_x <= x < notice_end_x:
                    buf += notice[x - notice_start_x]
                else:
                    buf += ' '

                x += 1

            # New line if it isn't the last line
            if y != h - 1:
                buf += color('&r\n')

        print(buf, end='', flush=True)

    try:
        while 1:
            # Clear the screen
            print("\033[2J\033[H", end="")
            draw_frame()
            frame += speed
            sleep(frame_delay)

            if key_pressed():
                break
    except KeyboardInterrupt:
        pass

    # Clear the screen
    printc("&r")
    print("\033[2J\033[H", end="", flush=True)


if __name__ == '__main__':
    start_animation()

