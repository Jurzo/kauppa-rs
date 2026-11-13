"""Builds the font files kauppalista.app serves (MB-0054, RR-0034).

Reads the variable TTFs beside this script and writes one WOFF2 file per
family into assets/fonts/: the Latin subset below, every weight kept, all
layout features kept. Run it by hand when a font or the subset changes; it
is not part of CI or of `cargo build`.

    python3 -m venv /tmp/fonts-venv
    /tmp/fonts-venv/bin/pip install fonttools==4.65.0 brotli==1.2.1
    /tmp/fonts-venv/bin/python design-system/fonts/build_fonts.py

The test suite pins each output's size and sha256
(`routes::assets::tests::brand_fonts`): after a run that changes a file,
update those constants in the same commit. Another format, character set or
family needs a Resolution Record (RR-0034 constraint 3).
"""
import hashlib
import pathlib
import sys

import brotli
import fontTools
from fontTools import subset
from fontTools.ttLib import TTFont

# The versions the committed files were built with.
FONTTOOLS = "4.65.0"
BROTLI = "1.2.1"

# Google Fonts' `latin` set, plus Š š Ž ž for Finnish loan words. The same
# list is the `unicode-range` of the @font-face rules in assets/app.css.
UNICODES = (
    "U+0000-00FF,U+0131,U+0152-0153,U+0160-0161,U+017D-017E,U+02BB-02BC,"
    "U+02C6,U+02DA,U+02DC,U+0304,U+0308,U+0329,U+2000-206F,U+20AC,U+2122,"
    "U+2191,U+2193,U+2212,U+2215,U+FEFF,U+FFFD"
)

HERE = pathlib.Path(__file__).resolve().parent
OUT = HERE.parent.parent / "assets" / "fonts"

# source TTF (under design-system/fonts/) -> served file (under assets/fonts/)
FONTS = {
    "Inter/Inter-VariableFont_opsz,wght.ttf": "inter.woff2",
    "Space_Grotesk/SpaceGrotesk-VariableFont_wght.ttf": "space-grotesk.woff2",
    "JetBrains_Mono/JetBrainsMono-VariableFont_wght.ttf": "jetbrains-mono.woff2",
}


def build(source: pathlib.Path, target: pathlib.Path) -> None:
    options = subset.Options()
    options.layout_features = ["*"]
    options.unicodes = subset.parse_unicodes(UNICODES)
    # Keep the source's timestamps, so the same input gives the same bytes.
    font = TTFont(source, recalcTimestamp=False)
    subsetter = subset.Subsetter(options)
    subsetter.populate(unicodes=options.unicodes)
    subsetter.subset(font)
    font.flavor = "woff2"
    font.save(target)


def main() -> int:
    got = (fontTools.version, brotli.__version__)
    if got != (FONTTOOLS, BROTLI):
        print(
            f"fonttools {got[0]} and brotli {got[1]} found; "
            f"the committed files need fonttools {FONTTOOLS} and brotli {BROTLI}",
            file=sys.stderr,
        )
        return 1
    OUT.mkdir(parents=True, exist_ok=True)
    for source, name in FONTS.items():
        target = OUT / name
        build(HERE / source, target)
        data = target.read_bytes()
        print(f"{name}\t{len(data)}\t{hashlib.sha256(data).hexdigest()}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
