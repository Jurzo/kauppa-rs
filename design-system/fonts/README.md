# Fonts

The three families the design names, as upright variable fonts. One file per family covers every weight the type styles use.

| Family | File | Version | Axes | Used at | Size | sha256 |
|---|---|---|---|---|---|---|
| Inter | `Inter/Inter-VariableFont_opsz,wght.ttf` | 4.001 | `wght` 100–900, `opsz` 14–32 | 400, 500, 600 | 874,708 B | `0be2399ea925f1f83ff974764761da9860ec50742ed29a5d4c1ffd0c5c7ac3a8` |
| Space Grotesk | `Space_Grotesk/SpaceGrotesk-VariableFont_wght.ttf` | 2.000 | `wght` 300–700 | 600, 700 | 134,112 B | `697a7905c71f7c61d071648bc73449541796af865b51c86a672795f3b452b142` |
| JetBrains Mono | `JetBrains_Mono/JetBrainsMono-VariableFont_wght.ttf` | 2.211 | `wght` 100–800 | 500 | 189,028 B | `192a3158a926768d1fa520164f0fb0f40a6ad085435aa854e43eb14cb40d1aab` |

## Source and licence

The files are byte for byte from the archive `Inter,JetBrains_Mono,Space_Grotesk.zip` (a Google Fonts download, supplied by the owner on 2026-11-13). Left out of the archive: the italic variable fonts and the `static/` single-weight files, which the design doesn't use, and the archive's `README.txt` files.

Each family is under the SIL Open Font License 1.1; its `OFL.txt` sits beside the font and must travel with it. None of the three declares a Reserved Font Name. See also `THIRD_PARTY_NOTICES.md` at the repository root.

## How the app serves them

These are source files; the app serves nothing from this directory. RR-0034 decided how the fonts reach the browser:

- `build_fonts.py` (added with MB-0054) writes one WOFF2 file per family into `assets/fonts/`: the Latin subset, every weight kept, about 173 kB together instead of 1.2 MB. It uses `fonttools` and `brotli` at pinned versions and is run by hand when a font changes, not by CI or `cargo build`.
- Subset ranges: `U+0000-00FF, U+0131, U+0152-0153, U+0160-0161, U+017D-017E, U+02BB-02BC, U+02C6, U+02DA, U+02DC, U+0304, U+0308, U+0329, U+2000-206F, U+20AC, U+2122, U+2191, U+2193, U+2212, U+2215, U+FEFF, U+FFFD`, all layout features kept. Text outside them shows in the system font.
- The outputs are committed and the test suite pins their size and sha256. `assets/app.css` declares them with `@font-face` and `font-display: swap`.
- Another format, character set or family needs a new Resolution Record.
