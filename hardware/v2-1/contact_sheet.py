from pathlib import Path
from PIL import Image, ImageDraw, ImageFont

HERE=Path(__file__).resolve().parent
font_path='/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf'
title=ImageFont.truetype(font_path,36)
label=ImageFont.truetype(font_path,21)
small=ImageFont.truetype(font_path,18)
canvas=Image.new('RGB',(1800,1510),(28,33,40))
draw=ImageDraw.Draw(canvas)
draw.text((40,25),'HOOK / V2.1',font=title,fill=(248,248,245))
draw.text((40,78),'Referencia refinada · Naranja mate · 64 × 100 × 34 mm · Blender',font=label,fill=(188,197,205))
items=[('front','01 / Frente'),('hero','02 / Volumen'),('back','03 / Dorso'),('rear-three-quarter','04 / Rejilla y puerto'),('top','05 / Botón superior'),('exploded','06 / Carcasa separada')]
for index,(file,text) in enumerate(items):
    x=20+(index%3)*595
    y=125+(index//3)*660
    im=Image.open(HERE/'renders'/f'{file}.png').convert('RGB')
    im.thumbnail((575,620),Image.Resampling.LANCZOS)
    canvas.paste(im,(x+(575-im.width)//2,y))
    draw.text((x+12,y+625),text,font=label,fill=(235,238,241))
draw.text((40,1470),'Muestra visual. Alojamientos y cierre sin tornillos pendientes de refinamiento.',font=small,fill=(188,197,205))
canvas.save(HERE/'renders'/'overview.png')
