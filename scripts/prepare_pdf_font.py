"""Optional build-time maintenance: instantiate the upstream variable font at weight 400.
The shipped application embeds the prepared TTF and needs no Python/fontTools at runtime.
"""
import argparse
import sys
from pathlib import Path

root = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(root / "tools" / "pythonlibs"))
from fontTools.ttLib import TTFont
from fontTools.varLib.instancer import instantiateVariableFont

parser = argparse.ArgumentParser()
parser.add_argument("source", type=Path)
parser.add_argument("--output", type=Path, default=root / "assets" / "fonts" / "NotoSansSC.ttf")
args = parser.parse_args()
font = TTFont(args.source)
font = instantiateVariableFont(font, {"wght": 400}, inplace=True)
for record in font["name"].names:
    value = {1: "Paperdesk Sans SC", 2: "Regular", 3: "PaperdeskSansSC-Regular-1.0", 4: "Paperdesk Sans SC Regular", 6: "PaperdeskSansSC-Regular", 16: "Paperdesk Sans SC", 17: "Regular"}.get(record.nameID)
    if value:
        record.string = value.encode(record.getEncoding(), errors="replace")
font.save(args.output)
print(args.output)
