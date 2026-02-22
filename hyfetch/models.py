from __future__ import annotations

from dataclasses import dataclass, field

from .constants import CONFIG_PATH
from .neofetch_util import ColorAlignment
from .presets import ColorProfile
from .serializer import json_stringify, from_dict
from .types import AnsiMode, LightDark, BackendLiteral


def build_hex_color_profile(hex_colors: list[str]) -> ColorProfile:
    for color in hex_colors:
        if not (
            color.startswith('#')
            and len(color) in [4, 7]
            and all(c in '0123456789abcdefABCDEF' for c in color[1:])
        ):
            raise ValueError(f'invalid hex color: {color}')
    return ColorProfile(hex_colors)


@dataclass
class Config:
    preset: str
    mode: AnsiMode
    light_dark: LightDark = 'dark'
    lightness: float | None = None
    color_align: ColorAlignment = field(default_factory=lambda: ColorAlignment('horizontal'))
    backend: BackendLiteral = "neofetch"
    args: str | None = None
    distro: str | None = None
    pride_month_shown: list[int] = field(default_factory=list)  # This is deprecated, see issue #136
    pride_month_disable: bool = False
    custom_ascii_path: str | None = None
    custom_presets: dict[str, list[str]] | None = None

    @classmethod
    def from_dict(cls, d: dict):
        d['color_align'] = ColorAlignment.from_dict(d['color_align'])
        return from_dict(cls, d)

    def save(self):
        CONFIG_PATH.parent.mkdir(exist_ok=True, parents=True)
        CONFIG_PATH.write_text(json_stringify(self, indent=4), 'utf-8')

    def custom_preset_profiles(self) -> dict[str, ColorProfile]:
        profiles: dict[str, ColorProfile] = {}
        if self.custom_presets:
            for preset_name, colors in self.custom_presets.items():
                try:
                    profiles[preset_name] = build_hex_color_profile(colors)
                except ValueError as err:
                    raise ValueError(
                        f'failed to validate custom preset key "{preset_name}": {err}'
                    ) from err
        return profiles
