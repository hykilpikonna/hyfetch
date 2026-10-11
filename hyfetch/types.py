try:
    from typing import Literal
except ImportError:
    from typing_extensions import Literal

AnsiMode = Literal['default', 'ansi', '8bit', 'rgb']
LightDark = Literal['light', 'dark']
BackendLiteral = Literal["neofetch", "fastfetch"]
ColorAlignMode = Literal['horizontal', 'vertical', 'custom', 'random']
ColorSpacing = Literal['equal', 'weighted']
