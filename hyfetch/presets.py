from __future__ import annotations

from typing import Iterable

from .color_util import RGB
from .constants import GLOBAL_CFG
from .types import LightDark, ColorSpacing


def remove_duplicates(seq: Iterable) -> list:
    """
    Remove duplicate items from a sequence while preserving the order
    """
    seen = set()
    seen_add = seen.add
    return [x for x in seq if not (x in seen or seen_add(x))]


class ColorProfile:
    raw: list[str]
    colors: list[RGB]
    spacing: ColorSpacing = 'equal'

    def __init__(self, colors: list[str] | list[RGB]):
        if isinstance(colors[0], str):
            self.raw = colors
            self.colors = [RGB.from_hex(c) for c in colors]
        else:
            self.colors = colors

    def with_weights(self, weights: list[int]) -> list[RGB]:
        """
        Map colors based on weights

        :param weights: Weights of each color (weights[i] = how many times color[i] appears)
        :return:
        """
        return [c for i, w in enumerate(weights) for c in [self.colors[i]] * w]

    def with_length(self, length: int) -> list[RGB]:
        """
        Spread to a specific length of text

        :param length: Length of text
        :return: List of RGBs of the length
        """
        preset_len = len(self.colors)
        center_i = preset_len // 2

        # How many copies of each color should be displayed at least?
        repeats = length // preset_len
        weights = [repeats] * preset_len

        # How many extra space left?
        extras = length % preset_len

        # If there is an even space left, extend the center by one space
        if extras % 2 == 1:
            extras -= 1
            weights[center_i] += 1

        # Add weight to border until there's no space left (extras must be even at this point)
        border_i = 0
        while extras > 0:
            extras -= 2
            weights[border_i] += 1
            weights[-(border_i + 1)] += 1
            border_i += 1

        return self.with_weights(weights)

    def color_text(self, txt: str, foreground: bool = True, space_only: bool = False) -> str:
        """
        Color a text

        :param txt: Text
        :param foreground: Whether the foreground text show the color or the background block
        :param space_only: Whether to only color spaces
        :return: Colored text
        """
        colors = self.with_length(len(txt))
        result = ''
        for i, t in enumerate(txt):
            if space_only and t != ' ':
                if i > 0 and txt[i - 1] == ' ':
                    result += '\033[39;49m'
                result += t
            else:
                result += colors[i].to_ansi(foreground=foreground) + t

        result += '\033[39;49m'
        return result

    def lighten(self, multiplier: float) -> ColorProfile:
        """
        Lighten the color profile by a multiplier

        :param multiplier: Multiplier
        :return: Lightened color profile (original isn't modified)
        """
        return ColorProfile([c.lighten(multiplier) for c in self.colors])

    def set_light_raw(self, light: float, at_least: bool | None = None, at_most: bool | None = None) -> 'ColorProfile':
        """
        Set HSL lightness value

        :param light: Lightness value (0-1)
        :param at_least: Set the lightness to at least this value (no change if greater)
        :param at_most: Set the lightness to at most this value (no change if lesser)
        :return: New color profile (original isn't modified)
        """
        return ColorProfile([c.set_light(light, at_least, at_most) for c in self.colors])

    def set_light_dl(self, light: float, term: LightDark | None = None):
        """
        Set HSL lightness value with respect to dark/light terminals

        :param light: Lightness value (0-1)
        :param term: Terminal color (can be "dark" or "light")
        :return: New color profile (original isn't modified)
        """
        if GLOBAL_CFG.use_overlay:
            return self.overlay_dl(light, term)

        term = term or GLOBAL_CFG.light_dark()
        assert term.lower() in ['light', 'dark']
        at_least, at_most = (True, None) if term.lower() == 'dark' else (None, True)
        return self.set_light_raw(light, at_least, at_most)

    def overlay_raw(self, color: RGB, alpha: float) -> 'ColorProfile':
        """
        Overlay a color on top of the color profile

        :param color: Color to overlay
        :param alpha: Alpha value (0-1)
        :return: New color profile (original isn't modified)
        """
        return ColorProfile([c.overlay(color, alpha) for c in self.colors])

    def overlay_dl(self, light: float, term: LightDark | None = None):
        """
        Same as set_light_dl except that this function uses RGB overlaying instead of HSL lightness change
        """
        term = term or GLOBAL_CFG.light_dark()
        assert term.lower() in ['light', 'dark']

        # If it's light bg, overlay black, else overlay white
        overlay_color = RGB.from_hex('#000000' if term.lower() == 'light' else '#FFFFFF')
        return self.overlay_raw(overlay_color, abs(light - 0.5) * 2)

    def set_light_dl_def(self, term: LightDark | None = None):
        """
        Set default lightness with respect to dark/light terminals

        :param term: Terminal color (can be "dark" or "light")
        :return: New color profile (original isn't modified)
        """
        return self.set_light_dl(GLOBAL_CFG.default_lightness(term), term)

    def unique_colors(self) -> ColorProfile:
        """
        Create another color profile with only the unique colors
        """
        return ColorProfile(remove_duplicates(self.colors))


PRESETS: dict[str, ColorProfile] = {
    'rainbow': ColorProfile([
        '#E50000',
        '#FF8D00',
        '#FFEE00',
        '#028121',
        '#004CFF',
        '#770088'
    ]),

    'transgender': ColorProfile([
        '#55CDFD',
        '#F6AAB7',
        '#FFFFFF',
        '#F6AAB7',
        '#55CDFD'
    ]),

    'nonbinary': ColorProfile([
        '#FCF431',
        '#FCFCFC',
        '#9D59D2',
        '#282828'
    ]),

    # xenogender sourced from https://commons.wikimedia.org/wiki/File:Xenogender_pride_flag.svg
    'xenogender': ColorProfile([
        '#FF6692',
        '#FF9A98',
        '#FFB883',
        '#FBFFA8',
        '#85BCFF',
        '#9D85FF',
        '#A510FF'
    ]),

    'agender': ColorProfile([
        '#000000',
        '#BABABA',
        '#FFFFFF',
        '#BAF484',
        '#FFFFFF',
        '#BABABA',
        '#000000'
    ]),

    'queer': ColorProfile([
        '#B57FDD',
        '#FFFFFF',
        '#49821E'
    ]),

    'genderfluid': ColorProfile([
        '#FE76A2',
        '#FFFFFF',
        '#BF12D7',
        '#000000',
        '#303CBE'
    ]),

    'bisexual': ColorProfile([
        '#D60270',
        '#9B4F96',
        '#0038A8'
    ]),

    'pansexual': ColorProfile([
        '#FF1C8D',
        '#FFD700',
        '#1AB3FF'
    ]),

    'polysexual': ColorProfile([
        '#F714BA',
        '#01D66A',
        '#1594F6',
    ]),

    # omnisexual sorced from https://www.flagcolorcodes.com/omnisexual
    'omnisexual': ColorProfile([
        '#FE9ACE',
        '#FF53BF',
        '#200044',
        '#6760FE',
        '#8EA6FF',
    ]),

    'omniromantic': ColorProfile([
        '#FEC8E4',
        '#FDA1DB',
        '#89739A',
        '#ABA7FE',
        '#BFCEFF',
    ]),

    # gay men sourced from https://www.flagcolorcodes.com/gay-men
    'gay-men': ColorProfile([
        '#078D70',
        '#98E8C1',
        '#FFFFFF',
        '#7BADE2',
        '#3D1A78'
    ]),

    'lesbian': ColorProfile([
        '#D62800',
        '#FF9B56',
        '#FFFFFF',
        '#D462A6',
        '#A40062'
    ]),

    # abrosexual used colorpicker to source from
    # https://fyeahaltpride.tumblr.com/post/151704251345/could-you-guys-possibly-make-an-abrosexual-pride
    'abrosexual': ColorProfile([
        '#46D294',
        '#A3E9CA',
        '#FFFFFF',
        '#F78BB3',
        '#EE1766',
    ]),

    'asexual': ColorProfile([
        '#000000',
        '#A4A4A4',
        '#FFFFFF',
        '#810081'
    ]),

    'aromantic': ColorProfile([
        '#3BA740',
        '#A8D47A',
        '#FFFFFF',
        '#ABABAB',
        '#000000'
    ]),
    
    # https://orientation.fandom.com/wiki/Fictosexual
    'fictosexual': ColorProfile(["#000000", "#C4C4C4", "#A349A5", "#C4C4C4", "#000000"]),

    # aroace1 sourced from https://flag.library.lgbt/flags/aroace/
    'aroace1': ColorProfile([
        '#E28C00',
        '#ECCD00',
        '#FFFFFF',
        '#62AEDC',
        '#203856'
    ]),

    'aroace2': ColorProfile([
        '#000000',
        '#810081',
        '#A4A4A4',
        '#FFFFFF',
        '#A8D47A',
        '#3BA740'
    ]),

    'aroace3': ColorProfile([
        '#3BA740',
        '#A8D47A',
        '#FFFFFF',
        '#ABABAB',
        '#000000',
        '#A4A4A4',
        '#FFFFFF',
        '#810081'
    ]),

    # below sourced from https://www.flagcolorcodes.com/flags/pride
    # goto f"https://www.flagcolorcodes.com/{preset}" for info
    # todo: sane sorting
    'autosexual': ColorProfile([
        '#99D9EA',
        '#7F7F7F'
    ]),

    'intergender': ColorProfile([
        # todo: use weighted spacing
        '#900DC2',
        '#900DC2',
        '#FFE54F',
        '#900DC2',
        '#900DC2',
    ]),

    'greygender': ColorProfile([
        '#B3B3B3',
        '#B3B3B3',
        '#FFFFFF',
        '#062383',
        '#062383',
        '#FFFFFF',
        '#535353',
        '#535353',
    ]),

    'akiosexual': ColorProfile([
        '#F9485E',
        '#FEA06A',
        '#FEF44C',
        '#FFFFFF',
        '#000000',
    ]),

    # bigender sourced from https://www.flagcolorcodes.com/bigender
    'bigender': ColorProfile([
        '#C479A2',
        '#EDA5CD',
        '#D6C7E8',
        '#FFFFFF',
        '#D6C7E8',
        '#9AC7E8',
        '#6D82D1',
    ]),

    # demigender yellow sourced from https://lgbtqia.fandom.com/f/p/4400000000000041031
    # other colors sourced from demiboy and demigirl flags
    'demigender': ColorProfile([
        '#7F7F7F',
        '#C4C4C4',
        '#FBFF75',
        '#FFFFFF',
        '#FBFF75',
        '#C4C4C4',
        '#7F7F7F',
    ]),

    # demiboy sourced from https://www.flagcolorcodes.com/demiboy
    'demiboy': ColorProfile([
        '#7F7F7F',
        '#C4C4C4',
        '#9DD7EA',
        '#FFFFFF',
        '#9DD7EA',
        '#C4C4C4',
        '#7F7F7F',
    ]),

    # demigirl sourced from https://www.flagcolorcodes.com/demigirl
    'demigirl': ColorProfile([
        '#7F7F7F',
        '#C4C4C4',
        '#FDADC8',
        '#FFFFFF',
        '#FDADC8',
        '#C4C4C4',
        '#7F7F7F',
    ]),

    'transmasculine': ColorProfile([
        '#FF8ABD',
        '#CDF5FE',
        '#9AEBFF',
        '#74DFFF',
        '#9AEBFF',
        '#CDF5FE',
        '#FF8ABD',
    ]),

    # transfeminine used colorpicker to source from https://www.deviantart.com/pride-flags/art/Trans-Woman-Transfeminine-1-543925985
    # linked from https://gender.fandom.com/wiki/Transfeminine
    'transfeminine': ColorProfile([
        '#73DEFF',
        '#FFE2EE',
        '#FFB5D6',
        '#FF8DC0',
        '#FFB5D6',
        '#FFE2EE',
        '#73DEFF',
    ]),

    # genderfaun sourced from https://www.flagcolorcodes.com/genderfaun
    'genderfaun': ColorProfile([
        '#FCD689',
        '#FFF09B',
        '#FAF9CD',
        '#FFFFFF',
        '#8EDED9',
        '#8CACDE',
        '#9782EC',
    ]),

    'demifaun': ColorProfile([
        '#7F7F7F',
        '#7F7F7F',
        '#C6C6C6',
        '#C6C6C6',
        '#FCC688',
        '#FFF19C',
        '#FFFFFF',
        '#8DE0D5',
        '#9682EC',
        '#C6C6C6',
        '#C6C6C6',
        '#7F7F7F',
        '#7F7F7F',
    ]),

    # genderfae sourced from https://www.flagcolorcodes.com/genderfae
    'genderfae': ColorProfile([
        '#97C3A5',
        '#C3DEAE',
        '#F9FACD',
        '#FFFFFF',
        '#FCA2C4',
        '#DB8AE4',
        '#A97EDD',
    ]),

    # demifae used colorpicker to source form https://www.deviantart.com/pride-flags/art/Demifae-870194777
    'demifae': ColorProfile([
        '#7F7F7F',
        '#7F7F7F',
        '#C5C5C5',
        '#C5C5C5',
        '#97C3A4',
        '#C4DEAE',
        '#FFFFFF',
        '#FCA2C5',
        '#AB7EDF',
        '#C5C5C5',
        '#C5C5C5',
        '#7F7F7F',
        '#7F7F7F',
    ]),

    'neutrois': ColorProfile([
        '#FFFFFF',
        '#1F9F00',
        '#000000'
    ]),

    'biromantic1': ColorProfile([
        '#8869A5',
        '#D8A7D8',
        '#FFFFFF',
        '#FDB18D',
        '#151638',
    ]),

    'biromantic2': ColorProfile([
        '#740194',
        '#AEB1AA',
        '#FFFFFF',
        '#AEB1AA',
        '#740194',
    ]),

    'autoromantic': ColorProfile([  # symbol interpreted
        '#99D9EA',
        '#99D9EA',
        '#3DA542',
        '#7F7F7F',
        '#7F7F7F',
    ]),

    # i didn't expect this one to work. cool!
    'boyflux2': ColorProfile(ColorProfile([
        '#E48AE4',
        '#9A81B4',
        '#55BFAB',
        '#FFFFFF',
        '#A8A8A8',
        '#81D5EF',
        '#69ABE5',
        '#5276D4',
    ]).with_weights([1, 1, 1, 1, 1, 5, 5, 5])),

    # sourced from https://commons.wikimedia.org/wiki/File:Girlflux_Pride_Flag.jpg
    "girlflux": ColorProfile([
        "f9e6d7",
        "f2526c",
        "bf0311",
        "e9c587",
        "bf0311",
        "f2526c",
        "f9e6d7",
    ]),

    # sourced from https://www.deviantart.com/pride-flags/art/Genderflux-1-543925589
    "genderflux": ColorProfile([
        "f47694",
        "f2a2b9",
        "cecece",
        "7ce0f7",
        "3ecdf9",
        "fff48d",
    ]),
    
    # https://lgbtqia.wiki/wiki/Gendernull
    'nullflux': ColorProfile([
        '#0B0C0E', '#A28DB9', '#E1D4EF', '#F0E6DD', '#665858',
    ]),
    
    'hypergender': ColorProfile([
        "#EFEFEF", "#FFFFFF", "#FBFF75", "#000000", "#FBFF75", "#FFFFFF", "#EFEFEF",
    ]),
    'hyperboy': ColorProfile([
        "#EFEFEF", "#FFFFFF", "#74D7FE", "#000000", "#74D7FE", "#FFFFFF", "#EFEFEF",
    ]),
    'hypergirl': ColorProfile([
        "#EFEFEF", "#FFFFFF", "#FC76D3", "#000000", "#FC76D3", "#FFFFFF", "#EFEFEF",
    ]),
    'hyperandrogyne': ColorProfile([
        "#EFEFEF", "#FFFFFF", "#BB83FF", "#000000", "#BB83FF", "#FFFFFF", "#EFEFEF",
    ]),
    'hyperneutrois': ColorProfile([
        "#EFEFEF", "#FFFFFF", "#BAFA74", "#000000", "#BAFA74", "#FFFFFF", "#EFEFEF",
    ]),

    "finsexual": ColorProfile([
        "#B18EDF",
        "#D7B1E2",
        "#F7CDE9",
        "#F39FCE",
        "#EA7BB3",
    ]),

    'unlabeled1': ColorProfile([
        '#EAF8E4',
        '#FDFDFB',
        '#E1EFF7',
        '#F4E2C4'
    ]),

    'unlabeled2': ColorProfile([
        '#250548',
        '#FFFFFF',
        '#F7DCDA',
        '#EC9BEE',
        '#9541FA',
        '#7D2557'
    ]),

    'pangender': ColorProfile([
        '#FFF798',
        '#FEDDCD',
        '#FFEBFB',
        '#FFFFFF',
        '#FFEBFB',
        '#FEDDCD',
        '#FFF798',
    ]),

    'pangender.contrast': ColorProfile([
        '#ffe87f',
        '#fcbaa6',
        '#fbc9f3',
        '#FFFFFF',
        '#fbc9f3',
        '#fcbaa6',
        '#ffe87f',
    ]),

    'gendernonconforming1': ColorProfile(
        ColorProfile([
            '#50284d',
            '#96467b',
            '#5c96f7',
            '#ffe6f7',
            '#5c96f7',
            '#96467b',
            '#50284d'
        ]).with_weights([
            4, 1, 1, 1, 1, 1, 4
        ])
    ),

    'gendernonconforming2': ColorProfile([
        '#50284d',
        '#96467b',
        '#5c96f7',
        '#ffe6f7',
        '#5c96f7',
        '#96467b',
        '#50284d'
    ]),

    'femboy': ColorProfile([
        "#d260a5",
        "#e4afcd",
        "#fefefe",
        "#57cef8",
        "#fefefe",
        "#e4afcd",
        "#d260a5"
    ]),

    'tomboy': ColorProfile([
        "#2f3fb9",
        "#613a03",
        "#fefefe",
        "#f1a9b7",
        "#fefefe",
        "#613a03",
        "#2f3fb9"
    ]),

    'gynesexual': ColorProfile([
        "#F4A9B7",
        "#903F2B",
        "#5B953B",
    ]),

    'androsexual': ColorProfile([
        "#01CCFF",
        "#603524",
        "#B799DE",
    ]),

    # gendervoid and related flags sourced from: https://gender.fandom.com/wiki/Gendervoid
    'gendervoid': ColorProfile([
        "#081149",
        "#4B484B",
        "#000000",
        "#4B484B",
        "#081149"
    ]),

    'voidgirl': ColorProfile([
        "#180827",
        "#7A5A8B",
        "#E09BED",
        "#7A5A8B",
        "#180827"
    ]),

    'voidboy': ColorProfile([
        "#0B130C",
        "#547655",
        "#66B969",
        "#547655",
        "#0B130C"
    ]),

    # used https://twitter.com/foxbrained/status/1667621855518236674/photo/1 as source and colorpicked
    'nonhuman-unity': ColorProfile([
        "#177B49",
        "#FFFFFF",
        "#593C90"
    ]),

    # used https://pluralpedia.org/w/Plurality#/media/File:Plural-Flag-1.jpg as source and colorpicked
    'plural': ColorProfile([
        "#2D0625",
        "#543475",
        "#7675C3",
        "#89C7B0",
        "#F3EDBD",
    ]),

    # sampled from https://es.m.wikipedia.org/wiki/Archivo:Fraysexual_flag.jpg
    'fraysexual': ColorProfile([
        '#226CB5',
        '#94E7DD',
        '#FFFFFF',
        '#636363',
    ]),

    # Queer Subcultures
    # sourced from https://commons.wikimedia.org/wiki/File:Bear_Brotherhood_flag.svg
    'bear': ColorProfile([
        '#623804',
        '#D56300',
        '#FEDD63',
        '#FEE6B8',
        '#FFFFFF',
        '#555555',
    ]),

    # colorpicked from https://commons.wikimedia.org/wiki/File:Butch_Flag.png
    'butch': ColorProfile([
        '#D72800',
        '#F17623',
        '#FF9C56',
        '#FFFDF6',
        '#FFCE89',
        '#FEAF02',
        '#A37000',
    ]),

    # colorpicked from https://commons.wikimedia.org/wiki/File:Leather,_Latex,_and_BDSM_pride_-_Light.svg
    'leather': ColorProfile([
        '#000000',
        '#252580',
        '#000000',
        '#252580',
        '#FFFFFF',
        '#252580',
        '#000000',
        '#252580',
        '#000000',
    ]),

    # colorpicked from https://commons.wikimedia.org/wiki/File:Official_Otter_Pride_Flag_by_Bearbackgear.jpg
    'otter': ColorProfile([
        '#263881',
        '#5C9DC9',
        '#FFFFFF',
        '#3A291D',
        '#5C9DC9',
        '#263881',
    ]),

    # colorpicked from https://commons.wikimedia.org/wiki/File:Twink_Pride_Flag_(proposed).svg
    'twink': ColorProfile([
        '#FFB2FF',
        '#FFFFFF',
        '#FFFF81',
    ]),
    
    # https://en.wikipedia.org/wiki/File:FatFetishFlag.png
    'adipophilia': ColorProfile(["#000000", "#E16180", "#FFF9BE", "#603E41", "#000000"]),

    'kenochoric': ColorProfile([
        '#000000',
        '#2E1569',
        '#824DB7',
        '#C7A1D6',
    ]),

    'veldian': ColorProfile([
        '#D182A8',
        '#FAF6E0',
        '#69ACBE',
        '#5D448F',
        '#3A113E',
    ]),

    'solian': ColorProfile([
        '#FFF8ED',
        '#FFE7A8',
        '#F1B870',
        '#A56058',
        '#46281E',
    ]),

    'lunian': ColorProfile([
        '#2F0E62',
        '#6F41B1',
        '#889FDF',
        '#7DDFD5',
        '#D2F2E2',
    ]),

    # Start of Extras by Jaida Corvera
    # polyamorous flag colors pulled from https://polyamproud.com/flag
    'polyam': ColorProfile([
        "#FFFFFF",
        "#FCBF00",
        "#009FE3",
        "#E50051",
        "#340C46",
    ]),

    'sapphic': ColorProfile([
        "#FD8BA8",
        "#FBF2FF",
        "#C76BC5",
        "#FDD768",
        "#C76BC5",
        "#FBF2FF",
        "#FD8BA8"
    ]),

    'androgyne': ColorProfile([
        "#FE007F",
        "#9832FF",
        "#00B8E7",
    ]),

    'interprogress': ColorProfile([
        "#FFD800",
        "#7902AA",
        "#FFFFFF",
        "#FFAFC8",
        "#74D7EE",
        "#613915",
        "#000000",
        '#E50000',
        '#FF8D00',
        '#FFEE00',
        '#028121',
        '#004CFF',
        '#770088'
    ]),

    'progress': ColorProfile([
        "#FFFFFF",
        "#FFAFC8",
        "#74D7EE",
        "#613915",
        "#000000",
        '#E50000',
        '#FF8D00',
        '#FFEE00',
        '#028121',
        '#004CFF',
        '#770088'
    ]),

    'intersex': ColorProfile([
        "#FFD800",
        "#FFD800",
        "#7902AA",
        "#FFD800",
        "#FFD800"
    ]),

    'old-polyam': ColorProfile([
        "#0000FF",
        "#FF0000",
        "#FFFF00",
        "#FF0000",
        "#000000"
    ]),

    'equal-rights': ColorProfile([
        "#0000FF",
        "#0000FF",
        "#FFFF00",
        "#0000FF",
        "#0000FF",
        "#FFFF00",
        "#0000FF",
        "#0000FF"
    ]),

    'drag': ColorProfile([
        "#CC67FF",
        "#FFFFFF",
        "#FFA3E3",
        "#FFFFFF",
        "#3366FF"
    ]),

    # Pronoun Flags

    'pronounfluid': ColorProfile([
        "#ffb3f9",
        "#ffffff",
        "#d1fdcb",
        "#c7b0ff",
        "#000000",
        "#b8ccff"
    ]),

    'pronounflux': ColorProfile([
        "#fdb3f8",
        "#b6ccfa",
        "#18ddd3",
        "#64ff89",
        "#ff7690",
        "#ffffff"
    ]),

    'exipronoun': ColorProfile([
        "#1c3d34",
        "#ffffff",
        "#321848",
        "#000000"
    ]),

    'neopronoun': ColorProfile([
        "#bcec64",
        "#ffffff",
        "#38077a"
    ]),

    'neofluid': ColorProfile([
        "#ffeca0",
        "#ffffff",
        "#ffeca0",
        "#38087a",
        "#bcec64"
    ]),

    'genderqueer': ColorProfile([
        "#b57edc",
        "#b57edc",
        "#ffffff",
        "#ffffff",
        "#4a8123",
        "#4a8123"
    ]),

    'cisgender': ColorProfile([
        "#D70270",
        "#0038A7"
    ]),

    # colors from Gilbert Baker's original 1978 flag design
    # used https://gilbertbaker.com/rainbow-flag-color-meanings/ as source and colorpicked
    'baker': ColorProfile([
        '#F23D9E',
        '#F80A24',
        '#F78022',
        '#F9E81F',
        '#1E972E',
        '#1B86BC',
        '#243897',
        '#6F0A82',
    ]),

    # this is 4 all the dogs, from zombpawcoins on tumblr!
    'caninekin': ColorProfile([
        '#2d2822',
        '#543d25',
        '#9c754d',
        '#e8dac2',
        '#cfad8c',
        '#b77b55',
        '#954e31'
    ]),

    # Meme flags
    'beiyang': ColorProfile([
        '#DF1B12',
        '#FFC600',
        '#01639D',
        '#FFFFFF',
        '#000000',
    ]),

    'burger': ColorProfile([
        '#F3A26A',
        '#498701',
        '#FD1C13',
        '#7D3829',
        '#F3A26A',
    ]),

    'throatlozenges': ColorProfile([
        "#2759DA",
        "#03940D",
        "#F5F100",
        "#F59B00",
        "#B71212"
    ]),

    'band': ColorProfile([
        "#2670c0",
        "#f5bd00",
        "#dc0045",
        "#e0608e"
    ]),
    
    # Adding libragender flags https://lgbtqia.wiki/wiki/Libragender
    # Sourced from https://lgbtqia.wiki/wiki/Libragender
    'libragender': ColorProfile([
        "#000000",
        "#808080",
        "#92D8E9",
        "#FFF544",
        "#FFB0CA",
        "#808080",
        "#000000"
    ]),
    
    # Sourced from https://lgbtqia.wiki/wiki/Librafeminine
    'librafeminine': ColorProfile([
        "#000000",
        "#A3A3A3",
        "#FFFFFF",
        "#C6568F",
        "#FFFFFF",
        "#A3A3A3",
        "#000000"
    ]),
    
    # Sourced from https://lgbtqia.wiki/wiki/Libramasculine
    'libramasculine': ColorProfile([
        "#000000",
        "#A3A3A3",
        "#FFFFFF",
        "#56C5C5",
        "#FFFFFF",
        "#A3A3A3",
        "#000000"
    ]),
    
    # Sourced from https://lgbtqia.wiki/wiki/Librandrogyne
    'libraandrogyne': ColorProfile([
        "#000000",
        "#A3A3A3",
        "#FFFFFF",
        "#9186B1",
        "#FFFFFF",
        "#A3A3A3",
        "#000000"
    ]),
    
    # Sourced from https://lgbtqia.wiki/wiki/Libranonbinary
    'libranonbinary': ColorProfile([
        "#000000",
        "#A3A3A3",
        "#FFFFFF",
        "#FFF987",
        "#FFFFFF",
        "#A3A3A3",
        "#000000"
    ]),

    # Adding Fluidflux flags - ObsoleteDev
    # Sourced from https://gender.fandom.com/wiki/Fluidflux?file=FC90B24D-CA36-4FE2-A752-C9ABFC65E332.jpeg

    'fluidflux A': ColorProfile([
        "#ff115f",
        "#a34aa3",
        "#00a4e7",
        "#ffdf00",
        "#000000",
        "#ffed71",
        "#85daff",
        "#dbadda",
        "#fe8db1"
    ]),

    'fluidflux B': ColorProfile([
        "#c6d1d2",
        "#f47b9d",
        "#f09f9b",
        "#e3f09e",
        "#75eeea",
        "#52d2ed",
        "#c6d1d2"
    ]),

    'Cenelian': ColorProfile([
        "#ffe7b6",
        "#93554a",
        "#52203a",
        "#7e4a93",
        "#99afd6"
    ]),

    'Transneutral': ColorProfile([
        "#74dfff",
        "#fffdb3",
        "#fffc75",
        "#fff200",
        "#fffc75",
        "#fffdb3",
        "#fe8cbf"
    ]),
}
