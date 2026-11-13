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

## Not served yet

These are source files. The app serves nothing from this directory, and `assets/` doesn't take font files yet: that needs a content type in `crates/kauppa-web/build.rs`, a place in AGENTS.md's assets rule, and choices about format and size (TTF as it is, or WOFF2, possibly cut down to the characters Finnish and English need; Inter alone is 875 kB here). Those are decided in MB-0054's consult. Until then the font stacks fall through to system fonts.
