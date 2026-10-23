"""Rasterize the same simple vector primitives at 4x for crisp Windows icons."""
from pathlib import Path
from PIL import Image, ImageDraw

root = Path(__file__).resolve().parents[1]
scale = 4
im = Image.new('RGBA', (1024, 1024))
d = ImageDraw.Draw(im)
def points(coords): return [(x*scale,y*scale) for x,y in coords]
def line(coords, color, width):
    d.line(points(coords), fill=color, width=width*scale, joint='curve')
    for x,y in (coords[0],coords[-1]):
        r=width*scale/2; d.ellipse((x*scale-r,y*scale-r,x*scale+r,y*scale+r),fill=color)
d.rounded_rectangle((32,32,992,992),216,fill='#294d52')
d.rounded_rectangle((228,256,764,808),64,fill='#c6bda4')
d.rounded_rectangle((284,188,796,760),52,fill='#f5ecd6')
line([(85,48),(85,189)],'#d7c9a8',4)
for y,x in [(86,169),(108,152),(130,161)]:line([(110,y),(x,y)],'#698080',7)
d.polygon(points([(154,145),(192,145),(192,210),(173,198),(154,210)]),fill='#cc8058')
line([(163,165),(170,172),(184,156)],'#fff8e8',5)
im=im.resize((256,256),Image.Resampling.LANCZOS)
im.save(root/'assets/paperdesk.png')
im.save(root/'assets/paperdesk.ico',sizes=[(16,16),(24,24),(32,32),(48,48),(64,64),(128,128),(256,256)])
