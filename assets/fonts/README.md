# Bundled Chinese font

`NotoSansSC.ttf` is a weight-400 static instance of the Noto Sans SC variable TrueType font from Google Fonts, renamed to Paperdesk Sans SC:
https://github.com/google/fonts/tree/main/ofl/notosanssc

License: SIL Open Font License 1.1, reproduced in `OFL.txt`.

Downloaded source: `https://raw.githubusercontent.com/google/fonts/main/ofl/notosanssc/NotoSansSC%5Bwght%5D.ttf`.

The font is embedded in the application, subset per exported PDF. PDF Unicode mappings prefer the actual printed characters over visually identical Unicode radical aliases.

Reproduce the static instance with fontTools and `python scripts/prepare_pdf_font.py upstream-variable.ttf`. The font remains under the OFL license. Python/fontTools are build-time maintenance tools only.
