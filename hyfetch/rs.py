from __future__ import annotations

import os
import platform
import subprocess
import sys

from .color_util import printc
from .constants import SRC
from .py import run_py


def run_rust():
    reason = None

    # Rust 1.76+ do not support windows < 10
    if platform.system() == 'Windows' and sys.getwindowsversion().major < 10:
        reason = '&cWindows < 10 detected, falling back to python version since rust 1.76+ do not support it.'

    # Rust 1.74+ do not support macOS < 10.12
    elif platform.system() == 'Darwin':
        mac_ver = platform.mac_ver()[0]
        if mac_ver and tuple(map(int, mac_ver.split('.'))) < (10, 12):
            reason = f'&cmacOS {mac_ver} detected, falling back to python version since rust 1.74+ do not support it.'

    # Find the rust executable
    pd = SRC / 'rust' / ('hyfetch.exe' if platform.system() == 'Windows' else 'hyfetch')
    if not reason and not pd.exists():
        reason = '&cThe executable for hyfetch v2 (rust) is not found, falling back to legacy v1.99.∞ (python).'

    if reason:
        if 'HYFETCH_DONT_WARN_RUST' not in os.environ:
            printc(f'{reason}\n'
                   'You can add environment variable HYFETCH_DONT_WARN_RUST=1 to suppress this warning.\n')
        run_py()
        return

    # Run the rust executable, passing in all arguments
    os.execv(str(pd), [str(pd), *sys.argv[1:]])


if __name__ == '__main__':
    try:
        run_rust()
    except KeyboardInterrupt:
        printc('&cThe program is interrupted by ^C, exiting...')
        exit(0)
