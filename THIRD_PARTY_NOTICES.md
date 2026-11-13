# Third-party notices

`kauppa-rs` is MIT-licensed (see `LICENSE`). The files below come from other
projects and are distributed under their own licences.

## flag-icons 7.6.0

- Project: <https://github.com/lipis/flag-icons>
- Source: the npm registry tarball
  <https://registry.npmjs.org/flag-icons/-/flag-icons-7.6.0.tgz>
- Version: 7.6.0
- Licence: MIT
- Files, byte for byte (RR-0031):
  - `assets/flags/en.svg`, from `package/flags/4x3/gb.svg`
  - `assets/flags/fi.svg`, from `package/flags/4x3/fi.svg`

```text
The MIT License (MIT)

Copyright (c) 2013 Panayiotis Lipiridis

Permission is hereby granted, free of charge, to any person obtaining a copy of
this software and associated documentation files (the "Software"), to deal in
the Software without restriction, including without limitation the rights to
use, copy, modify, merge, publish, distribute, sublicense, and/or sell copies
of the Software, and to permit persons to whom the Software is furnished to do
so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```

## Inter 4.001, Space Grotesk 2.000, JetBrains Mono 2.211 (fonts)

- Projects: <https://github.com/rsms/inter>,
  <https://github.com/floriankarsten/space-grotesk>,
  <https://github.com/JetBrains/JetBrainsMono>
- Source: a Google Fonts download supplied by the owner (2026-11-13)
- Licence: SIL Open Font License 1.1, each family's full text beside it
- Files, byte for byte (checksums in `design-system/fonts/README.md`):
  - `design-system/fonts/Inter/Inter-VariableFont_opsz,wght.ttf`,
    licence `design-system/fonts/Inter/OFL.txt`
    (Copyright 2020 The Inter Project Authors)
  - `design-system/fonts/Space_Grotesk/SpaceGrotesk-VariableFont_wght.ttf`,
    licence `design-system/fonts/Space_Grotesk/OFL.txt`
    (Copyright 2020 The Space Grotesk Project Authors)
  - `design-system/fonts/JetBrains_Mono/JetBrainsMono-VariableFont_wght.ttf`,
    licence `design-system/fonts/JetBrains_Mono/OFL.txt`
    (Copyright 2020 The JetBrains Mono Project Authors)
- Served by the app as modified copies (RR-0034): WOFF2 files cut to a
  Latin character set, every weight kept, written by
  `design-system/fonts/build_fonts.py` from the files above. They keep the
  families' names (none declares a Reserved Font Name) and the same
  licence:
  - `assets/fonts/inter.woff2`
  - `assets/fonts/space-grotesk.woff2`
  - `assets/fonts/jetbrains-mono.woff2`
