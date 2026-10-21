# The licence texts that ship with Vocument

Every binary that Vocument hands out carries the licences of the components
that are inside it. The texts of those licences lie in this directory, so
that the notice of a binary can be assembled without a network connection
and without a guess.

The provenance of every text is named here. A licence text that cannot be
named is a **finding**: it is not written into the notice.

| File | Licence | Where the text was taken from |
|---|---|---|
| `AGPL-3.0-or-later.txt` | AGPL-3.0-or-later | the `COPYING` of MuPDF 1.28.5, the file that ships with MuPDF. It is word-identical with the text of the GNU project (SHA-256 `0d96a4ff68ad6d4b6f1f30f713b18d5184912ba8dd389f86aa7710db079abcb0`) |
| `MPL-2.0.txt` | MPL-2.0 | the SPDX licence list as it is packaged in the crate `spdx` 0.10.8 (`src/text/licenses/MPL-2.0`) — the licence of UniFFI, which is bound into the library that is shipped |
| `Apache-2.0.txt` | Apache-2.0 | the SPDX licence list (`spdx` 0.10.8, `src/text/licenses/Apache-2.0`) |
| `BSD-2-Clause.txt` | BSD-2-Clause | the SPDX licence list (`spdx` 0.10.8, `src/text/licenses/BSD-2-Clause`) |
| `BSD-3-Clause.txt` | BSD-3-Clause | the SPDX licence list (`spdx` 0.10.8, `src/text/licenses/BSD-3-Clause`) |
| `ISC.txt` | ISC | the SPDX licence list (`spdx` 0.10.8, `src/text/licenses/ISC`) |
| `MIT.txt` | MIT | the SPDX licence list (`spdx` 0.10.8, `src/text/licenses/MIT`) |
| `Zlib.txt` | Zlib | the SPDX licence list (`spdx` 0.10.8, `src/text/licenses/Zlib`) |
| `OFL-1.1.txt` | OFL-1.1 | the SPDX licence list (`spdx` 0.10.8, `src/text/licenses/OFL-1.1`) — the licence of the fonts of Noto, URW, SIL Charis and Source Han |
| `Unicode-3.0.txt` | Unicode-3.0 | the SPDX licence list (`spdx` 0.10.8, `src/text/licenses/Unicode-3.0`) |
| `FTL.txt` | FTL (FreeType) or GPLv2 — the project uses the **FTL** | the `LICENSE.TXT` of the FreeType source in the MuPDF tree — the file that ships with FreeType itself |
| `IJG.txt` | IJG (the terms of the Independent JPEG Group) | the `README` of the libjpeg source in the MuPDF tree — the file that ships with libjpeg itself |

The texts of the font families that ship inside the PDF engine are the files
of the fonts themselves: `resources/fonts/noto/COPYING`,
`resources/fonts/urw/OFL.txt`, `resources/fonts/sil/OFL.txt` and
`resources/fonts/han/LICENSE.txt` in the source of MuPDF 1.28.5, and the
`resources/fonts/droid/NOTICE` for the two fonts of the Droid family (the
Apache-2.0). They carry the copyright of the family and the reserved font
names, and they are read from the source tree when the notice is assembled.

`scripts/generate-third-party-notices.sh` assembles the notice of a release
from these texts and from the inventory of `THIRD_PARTY.md`.
