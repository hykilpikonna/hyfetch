from __future__ import annotations
import json
from pathlib import Path
from . import AsciiArt

# Cache for loaded distros
_distros: list[AsciiArt] | None = None

def get_distros() -> list[AsciiArt]:
    global _distros
    if _distros is not None:
        return _distros
    
    _distros = []
    # Path to hyfetch/data/distros
    distro_dir = Path(__file__).parent.parent / 'data' / 'distros'
    
    if not distro_dir.exists():
        return []
        
    for f in sorted(distro_dir.glob('*.ascii')):
        try:
            content = f.read_text('utf-8')
            header_line, art = content.split('\n', 1)
            header = json.loads(header_line)
            _distros.append(AsciiArt(
                match=header['match'],
                color=str(header['color']),
                ascii=art,
                foreground=header.get('foreground'),
                background=header.get('background')
            ))
        except Exception:
            pass
            
    return _distros

def detect(name: str) -> AsciiArt | None:
    if not name:
        return None
    
    for distro in get_distros():
        if distro.matches(name):
            return distro
            
    return None
