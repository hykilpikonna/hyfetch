from __future__ import annotations

import os
import platform
import re
import shlex
import shutil
import subprocess
import sys
from dataclasses import dataclass
from pathlib import Path
from subprocess import check_output
from tempfile import TemporaryDirectory
from typing import Iterable

from .color_util import color, printc
from .constants import GLOBAL_CFG, IS_WINDOWS
from .distros import distro_detector
from .presets import ColorProfile
from .serializer import from_dict
from .types import BackendLiteral, ColorAlignMode

RE_NEOFETCH_COLOR = re.compile('\\${c[0-9]}')
SRC = Path(__file__).parent


def literal_input(prompt: str, options: Iterable[str], default: str, show_ops: bool = True) -> str:
    """
    Ask the user to provide an input among a list of options

    :param prompt: Input prompt
    :param options: Options
    :param default: Default option
    :param show_ops: Show options
    :return: Selection
    """
    options = list(options)
    lows = [o.lower() for o in options]

    if show_ops:
        op_text = '|'.join([f'&l&n{o}&L&N' if o == default else o for o in options])
        printc(f'{prompt} ({op_text})')
    else:
        printc(f'{prompt} (default: {default})')

    def find_selection(sel: str):
        if not sel:
            return None

        # Find exact match
        if sel in lows:
            return options[lows.index(sel)]

        # Find starting abbreviation
        for i, op in enumerate(lows):
            if op.startswith(sel):
                return options[i]

        return None

    selection = input('> ').lower() or default
    while not find_selection(selection):
        print(f'Invalid selection! {selection} is not one of {"|".join(options)}')
        selection = input('> ').lower() or default

    print()

    return find_selection(selection)


def term_size() -> tuple[int, int]:
    """
    Get terminal size
    :return:
    """
    try:
        return os.get_terminal_size().columns, os.get_terminal_size().lines
    except Exception:
        return 100, 20


def ascii_size(asc: str) -> tuple[int, int]:
    """
    Get distro ascii width, height ignoring color code

    :param asc: Distro ascii
    :return: Width, Height
    """
    return max(len(line) for line in re.sub(RE_NEOFETCH_COLOR, '', asc).split('\n')), len(asc.split('\n'))


def normalize_ascii(asc: str) -> str:
    """
    Make sure every line are the same width
    """
    w = ascii_size(asc)[0]
    return '\n'.join(line + ' ' * (w - ascii_size(line)[0]) for line in asc.split('\n'))


def fill_starting(asc: str) -> str:
    """
    Fill the missing starting placeholders.

    E.g. "${c1}...\n..." -> "${c1}...\n${c1}..."
    """
    new = []
    last = ''
    for line in asc.split('\n'):
        new.append(last + line)

        # Line has color placeholders
        matches = RE_NEOFETCH_COLOR.findall(line)
        if len(matches) > 0:
            # Get the last placeholder for the next line
            last = matches[-1]

    return '\n'.join(new)


@dataclass
class ColorAlignment:
    mode: ColorAlignMode

    # custom_colors[ascii color index] = unique color index in preset
    custom_colors: dict[int, int] = ()

    # Foreground/background ascii color index
    fore_back: tuple[int, int] = ()

    @classmethod
    def from_dict(cls, d: dict):
        ca = from_dict(cls, d)
        # Fixup: Keys must json serialize as str, so we convert them back to int.
        ca.custom_colors = {int(k): v for k, v in ca.custom_colors.items()}
        return ca

    def recolor_ascii(self, asc: str, preset: ColorProfile) -> str:
        """
        Use the color alignment to recolor an ascii art

        :return Colored ascii, Uncolored lines
        """
        asc = fill_starting(asc)

        if self.fore_back and self.mode in ['horizontal', 'vertical']:
            fore, back = self.fore_back

            # Replace foreground colors
            asc = asc.replace(f'${{c{fore}}}', color('&0' if GLOBAL_CFG.is_light else '&f'))
            lines = asc.split('\n')

            # Add new colors
            if self.mode == 'horizontal':
                colors = preset.with_length(len(lines))
                asc = '\n'.join([l.replace(f'${{c{back}}}', colors[i].to_ansi()) + color('&~&*') for i, l in enumerate(lines)])
            else:
                raise NotImplementedError()

            # Remove existing colors
            asc = re.sub(RE_NEOFETCH_COLOR, '', asc)

        elif self.mode in ['horizontal', 'vertical']:
            # Remove existing colors
            asc = re.sub(RE_NEOFETCH_COLOR, '', asc)
            lines = asc.split('\n')

            # Add new colors
            if self.mode == 'horizontal':
                colors = preset.with_length(len(lines))
                asc = '\n'.join([colors[i].to_ansi() + l + color('&~&*') for i, l in enumerate(lines)])
            else:
                asc = '\n'.join(preset.color_text(line) + color('&~&*') for line in lines)

        else:
            preset = preset.unique_colors()

            # Apply colors
            color_map = {ai: preset.colors[pi].to_ansi() for ai, pi in self.custom_colors.items()}
            for ascii_i, c in color_map.items():
                asc = asc.replace(f'${{c{ascii_i}}}', c)

        return asc


def if_file(f: str | Path) -> Path | None:
    """
    Return the file if the file exists, or return none. Useful for chaining 'or's
    """
    f = Path(f)
    if f.is_file():
        return f
    return None


def get_command_path() -> str:
    """
    Get the absolute path of the neofetch command

    :return: Command path
    """
    cmd_path = (if_file(SRC.parent / 'neofetch') or if_file(SRC / 'scripts/neowofetch'))

    if not cmd_path:
        printc("&cError: Neofetch script cannot be found")
        exit(127)

    return str(cmd_path)


def ensure_git_bash() -> Path:
    """
    Ensure git bash installation for windows

    :returns git bash path
    """
    if not IS_WINDOWS:
        return Path('/usr/bin/bash')

    # Bundled git bash
    git_path = (if_file(SRC / 'git/bin/bash.exe')
                or if_file("C:/Program Files/Git/bin/bash.exe")
                or if_file("C:/Program Files (x86)/Git/bin/bash.exe"))

    if not git_path.is_file():
        printc("&cError: Git Bash installation not found")
        sys.exit(127)

    return git_path


def check_windows_cmd():
    """
    Check if this script is running under cmd.exe. If so, launch an external window with git bash
    since cmd doesn't support RGB colors.
    """
    # if IS_WINDOWS:
    #     import psutil
    #     # TODO: This line does not correctly identify cmd prompts...
    #     if psutil.Process(os.getppid()).name().lower().strip() == 'cmd.exe':
    #         print("cmd.exe doesn't support RGB colors, restarting in MinTTY...")
    #         cmd = f'"{ensure_git_bash().parent.parent / "usr/bin/mintty.exe"}" -s 110,40 -e python -m hyfetch --ask-exit'
    #         os.system(cmd)
    #         sys.exit(0)


def run_neofetch_cmd(args: str, pipe: bool = False) -> str | None:
    """
    Run neofetch command
    """
    if platform.system() != 'Windows':
        bash = ['/usr/bin/env', 'bash'] if Path('/usr/bin/env').is_file() else [shutil.which('bash')]
        full_cmd = [*bash, get_command_path(), *shlex.split(args)]

    else:
        cmd = get_command_path().replace("\\", "/").replace("C:/", "/c/")
        args = args.replace('\\', '/').replace('C:/', '/c/')

        full_cmd = [ensure_git_bash(), cmd, *shlex.split(args)]

    if pipe:
        return check_output(full_cmd).decode().strip()
    else:
        subprocess.run(full_cmd)


def get_distro_ascii(distro: str | None = None) -> str:
    """
    Get the distro ascii of the current distro. Or if distro is specified, get the specific distro's
    ascii art instead.

    :return: Distro ascii
    """
    if not distro and GLOBAL_CFG.override_distro:
        distro = GLOBAL_CFG.override_distro
    if GLOBAL_CFG.debug:
        print(distro)
        print(GLOBAL_CFG)

    # Try new pure-python detection method
    det = distro_detector.detect(distro or get_distro_name())
    if det is not None:
        return normalize_ascii(det.ascii)

    if GLOBAL_CFG.debug:
        printc(f"&cError: Cannot find distro {distro}")

    # Old detection method that calls neofetch
    cmd = 'print_ascii'
    if distro:
        cmd += f' --ascii_distro {distro}'

    asc = run_neofetch_cmd(cmd, True)

    # Unescape backslashes here because backslashes are escaped in neofetch for printf
    asc = asc.replace('\\\\', '\\')

    return normalize_ascii(asc)


def get_distro_name():
    return run_neofetch_cmd('ascii_distro_name', True)


def run(asc: str, backend: BackendLiteral, args: str = ''):
    if backend == "neofetch":
        return run_neofetch(asc, args)
    if backend == "fastfetch":
        return run_fastfetch(asc, args)
    if backend == "fastfetch-old":
        return run_fastfetch(asc, args, legacy=True)
    if backend == "qwqfetch":
        return run_qwqfetch(asc, args)


def run_qwqfetch(asc: str, args: str = ''):
    """
    Run qwqfetch with colors

    :param asc: Ascii art
    :param args: Additional arguments to pass to qwqfetch
    """
    asc = asc.replace('\\', '\\\\')

    # call qwqfetch to print string
    try:
        import qwqfetch
        # distro_detector only return a bash variable
        # so we use qwqfetch builtin distro detector
        print(qwqfetch.get_ascres(asc))  
    except ImportError as e:  # module not found etc
        print("qwqfetch is not installed. Install it by executing:")  # use print to output hint directly
        print("pip install git+https://github.com/nexplorer-3e/qwqfetch")  # TODO: public repo
        exit(127)


def run_neofetch(asc: str, args: str = ''):
    """
    Run neofetch with colors

    :param asc: Ascii art
    :param args: Additional arguments to pass to neofetch
    """
    # Escape backslashes here because backslashes are escaped in neofetch for printf
    asc = asc.replace('\\', '\\\\')

    # Write temp file
    with TemporaryDirectory() as tmp_dir:
        tmp_dir = Path(tmp_dir)
        path = tmp_dir / 'ascii.txt'
        path.write_text(asc, 'utf-8')

        # Call neofetch with the temp file
        if args:
            args = ' ' + args
        run_neofetch_cmd(f'--ascii --source {path.absolute()} --ascii-colors' + args)


def fastfetch_path() -> Path | None:
    return (shutil.which('fastfetch')
            or if_file(SRC / 'fastfetch/usr/bin/fastfetch')
            or if_file(SRC / 'fastfetch/fastfetch')
            or if_file(SRC / 'fastfetch/fastfetch.exe'))


def run_fastfetch(asc: str, args: str = '', legacy: bool = False):
    """
    Run neofetch with colors

    :param asc: Ascii art
    :param args: Additional arguments to pass to fastfetch
    :param legacy: Set true when using fastfetch < 1.8.0
    """
    # Find fastfetch binary
    ff_path = fastfetch_path()
    
    if not ff_path:
        printc("&cError: fastfetch binary is not found. Please install fastfetch first.")
        exit(127)
    
    # Write temp file
    with TemporaryDirectory() as tmp_dir:
        tmp_dir = Path(tmp_dir)
        path = tmp_dir / 'ascii.txt'
        path.write_text(asc, 'utf-8')

        # Call fastfetch with the temp file
        proc = subprocess.run([ff_path, '--raw' if legacy else '--file-raw', path.absolute(), *shlex.split(args)])
        if proc.returncode == 144:
            printc("&6Error code 144 detected: Please upgrade fastfetch to >=1.8.0 or use the 'fastfetch-old' backend")


def get_fore_back(distro: str | None = None) -> tuple[int, int] | None:
    """
    Get recommended foreground-background configuration for distro, or None if the distro ascii is
    not suitable for fore-back configuration.

    :return:
    """
    if not distro and GLOBAL_CFG.override_distro:
        distro = GLOBAL_CFG.override_distro
    if not distro:
        distro = get_distro_name().lower()
    distro = distro.lower().replace(' ', '-')
    for k, v in fore_back.items():
        if distro.startswith(k.lower()):
            return v
    return None


# Foreground-background recommendation
fore_back = {
    'fedora': (2, 1),
    'kubuntu': (2, 1),
    'lubuntu': (2, 1),
    'xubuntu': (2, 1),
    'ubuntu-cinnamon': (2, 1),
    'ubuntu-kylin': (2, 1),
    'ubuntu-mate': (2, 1),
    'ubuntu-studio': (2, 1),
    'ubuntu-sway': (2, 1),
    'ultramarine': (2, 1),
}

