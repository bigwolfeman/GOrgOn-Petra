# Embedded font assets

Every file here is embedded into the binary with `include_bytes!` from
`petra-egui/src/fonts.rs`. Nothing here is read from the filesystem at
run time, and that is the point: a frame digest that means the same thing on
two machines cannot depend on a face the operator happens to have installed.

## Faces

| File | Family | Version | Source | SHA-256 |
|---|---|---|---|---|
| `IBMPlexSans-Regular.ttf` | IBM Plex Sans | `@ibm/plex-sans@1.1.0` | [IBM/plex release `ibm-plex-sans.zip`](https://github.com/IBM/plex/releases/tag/%40ibm%2Fplex-sans%401.1.0), `fonts/complete/ttf/` | `975dcda37d80f038dcd143c22e33ca2d97a0cc5a929aace1c749153b0fe1afa5` |
| `IBMPlexSans-Medium.ttf` | IBM Plex Sans | `@ibm/plex-sans@1.1.0` | same | `331c8639d7598b2cde62a911a71db195e30cb655cd6bdf2e324a7e984955f907` |
| `IBMPlexSans-Bold.ttf` | IBM Plex Sans | `@ibm/plex-sans@1.1.0` | same | `9e6c74a889a700d707613d24548fe4ffa6bc59559a0689d2cf9e133bdcdafb2f` |
| `IBMPlexMono-Regular.ttf` | IBM Plex Mono | `@ibm/plex-mono@2.5.0` | [IBM/plex release `ibm-plex-mono.zip`](https://github.com/IBM/plex/releases/tag/%40ibm%2Fplex-mono%402.5.0), `fonts/complete/ttf/` | `7c6fbddca4b700be918f5f6183d9bd4464fa427fe435f0b480d77fe2bb8c5a43` |

776,884 bytes in total. Unsubsetted, deliberately: subsetting is a build step
that has to be re-run and re-verified whenever a string changes, and 759 KiB
buys every Latin, Greek, Cyrillic and punctuation codepoint the product might
ever draw without anyone having to think about it again.

## Licence

`OFL.txt` — SIL Open Font License 1.1, byte-identical in the Sans and Mono
distributions, so one copy covers both. Copyright © 2017 IBM Corp. with
Reserved Font Name "Plex". The OFL permits embedding in a binary without any
obligation on the binary's own licence; it forbids selling the fonts on their
own and requires the copyright notice and licence travel with them, which is
what this directory is.

## Why IBM Plex and not Inter, Geist, or Public Sans

`epaint` 0.36.1 exposes **no OpenType feature-tag API**, so `tnum` cannot be
requested. A face whose figures are proportional by default therefore draws
proportional figures in this product for ever, and a column of numbers in a
diagnostic tool that does not line up is a defect that cannot be fixed from
the theme.

Measured on the exact files in this directory, with `fontTools`:

* All four faces are `unitsPerEm = 1000` and give **every one of the ten
  digits an advance of 600 units**.
* None of the four carries a `tnum` or a `pnum` feature in `GSUB` at all —
  there is no proportional set to accidentally select, because tabular is the
  only set there is.

Inter and Geist ship proportional figures and are ruled out on that mechanic,
not on taste.

## Why no Noto Sans Symbols 2

The design note that specified this stack asked for it "to cover the
`StatusShape` glyphs that are tofu today". That reason has since expired
twice over, and the face was measured rather than assumed:

1. `StatusShape` no longer draws through a glyph at all. `paint.rs`'s
   `silhouette_points` traces the triangle and the diamond as polygons, so
   there is no codepoint left to be missing.
2. Every non-ASCII character in a string literal under `inspector/src`,
   `petra/src` and `petra-egui/src` was enumerated (47 distinct codepoints:
   em dash, ellipsis, section sign, superscripts, `·`, `×`, `→`, `√`, `≥`,
   `±`, Greek, subscript zero, and the script and emoji samples that exist
   only inside tests). `NotoSansSymbols2-Regular.ttf` carries **none** of
   them — its 2,641 codepoints are dingbats, box drawing, braille and game
   symbols, none of which this product draws. IBM Plex Sans carries every one
   of the punctuation and Greek codepoints the product renders.

Embedding it would have added 671 KiB and closed zero boxes. If a future
surface draws from those blocks, this is the note that says where to look.

## Script fallbacks are a separate question

CJK, Arabic, Devanagari and Hebrew are **not** here. See
`fonts::DESKTOP_FALLBACKS` and the agent note on the font stack: those faces
are roughly 32 MiB of Noto, they appear in this workspace only inside test
samples, and the decision about them is recorded there rather than made
silently by whoever added a file to this directory.
