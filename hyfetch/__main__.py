from __future__ import annotations

import os

from .py import run_py
from .rs import run_rust


def run_neofetch():
    from .neofetch_util import run_neofetch_cmd
    import sys
    run_neofetch_cmd(sys.argv[1:])


if __name__ == '__main__':
    if os.environ.get('HYFETCH_PY', False):
        run_py()
    else:
        run_rust()
