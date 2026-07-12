try:
    from typing import Literal
except ImportError:
    from typing_extensions import Literal

AnsiMode = Literal['default', 'ansi', '8bit', 'rgb']
LightDark = Literal['light', 'dark']
BackendLiteral = Literal["qwqfetch", "neofetch", "fastfetch", "fastfetch-old"]
ColorAlignMode = Literal['horizontal', 'vertical', 'custom']
ColorSpacing = Literal['equal', 'weighted']
