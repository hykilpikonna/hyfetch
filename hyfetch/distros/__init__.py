from __future__ import annotations

import string


class AsciiArt:
    name: str
    match: str
    color: str
    ascii: str
    foreground: list[int]
    background: int | None

    def __init__(self, match: str, color: str, ascii: str, name: str | None = None, 
                 foreground: list[int] | None = None, background: int | None = None):
        self.match = match
        self.color = color
        self.ascii = ascii
        self.name = name or self.get_friendly_name()
        self.foreground = foreground or []
        self.background = background

    def get_friendly_name(self) -> str:
        return self.match.split("|")[0].strip(string.punctuation + '* ') \
            .replace('"', '').replace('*', '')

    def matches(self, name: str) -> bool:
        name = name.lower()
        for m in self.match.split('|'):
            m = m.strip()
            stripped = m.strip('*\'"').lower()

            if '"*"' in m:
                prefix, suffix = stripped.split('"', 1)
                if name.startswith(prefix) and name.endswith(suffix):
                    return True
                continue

            # Exact matches
            if '*' not in m:
                if name == stripped:
                    return True
                continue

            # Both sides are *
            if m.startswith('*') and m.endswith('*'):
                if stripped in name:
                    return True
                continue

            # Ends with *
            if m.endswith('*'):
                if name.startswith(stripped):
                    return True
                continue

            # Starts with *
            if m.startswith('*'):
                if name.endswith(stripped):
                    return True
                continue
        return False
