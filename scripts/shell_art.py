#!/usr/bin/env python3
"""Original Hi5OS Aurora scene. Deterministic, offline, no external assets.
The approved concept is a reference, not a baked-in fake desktop.
"""
from pathlib import Path
import struct, zlib
from math import sin, cos, exp, pi
import random
W,H=960,540
rng=random.Random(5021)
def rgb(c): return tuple(max(0,min(255,round(t))) for t in c)
def mix(a,b,t): return tuple(x+(y-x)*t for x,y in zip(a,b))
def noise(x,y): return (sin(x*49+y*13)+sin(x*113-y*73)+sin(x*239+y*183))/3
pixels=bytearray(W*H*3)
class Raster:
    def __getitem__(self,xy):
        x,y=xy;i=(y*W+x)*3;return tuple(pixels[i:i+3])
    def __setitem__(self,xy,c):
        x,y=xy;i=(y*W+x)*3;pixels[i:i+3]=bytes(c)
p=Raster()
for j in range(H):
    y=j/(H-1)
    for i in range(W):
        x=i/(W-1)
        base=mix((3,10,29),(8,57,100),min(1,y*1.5))
        # Multiple folded curtains, brightest at their lower edge.
        for phase,col,amp in [(0,(8,223,208),1.0),(0.9,(45,95,240),.65),(1.7,(86,35,199),.4)]:
            edge=.43-.26*x+.16*sin(x*5.9+phase)+.045*sin(x*11+phase)
            delta=y-edge
            curtain=exp(-abs(delta)/(.035 if delta>0 else .105))
            fold=.68+.21*sin(x*57+y*19+phase)+.11*sin(x*177-y*30)
            a=max(0,min(.92,curtain*fold*amp))
            base=mix(base,col,a)
        p[i,j]=rgb(base)
for _ in range(900):
    x=rng.randrange(W);y=rng.randrange(int(H*.48));v=rng.randrange(65,170)
    if rng.random()<.12: p[x,y]=(v//2,v,v)
# Mountain profiles, jagged snow slopes and shadows.
shore=int(H*.72)
for n,(base,peaks,shade,snow) in enumerate([
    (.64,[(.07,.53),(.26,.45),(.41,.55),(.57,.44),(.79,.50),(.94,.40)],(13,40,77),(58,116,167)),
    (.68,[(.04,.61),(.20,.48),(.34,.58),(.45,.38),(.59,.55),(.72,.49),(.86,.60)],(6,26,55),(75,151,205)),
    (.71,[(.02,.63),(.13,.54),(.25,.61),(.36,.58),(.55,.65),(.78,.59),(.96,.63)],(3,17,38),(28,77,119))
]):
    pts=[(0,base),*peaks,(1,base)]
    ridge=[]
    idx=0
    for i in range(W):
        x=i/(W-1)
        while idx+1<len(pts)-1 and x>pts[idx+1][0]: idx+=1
        a,b=pts[idx],pts[idx+1]; t=(x-a[0])/(b[0]-a[0])
        y=a[1]+(b[1]-a[1])*t + .012*sin(x*83+n)+.006*sin(x*231)+.003*sin(x*611)
        ridge.append(int(y*H))
    for i,top in enumerate(ridge):
        for j in range(max(0,top),shore):
            z=(j-top)/max(1,shore-top); x=i/W;y=j/H
            facet=.5+.5*sin(x*97+y*67+n)
            detail=noise(x,y)*.1
            snowline=max(0,1-z*2.8+detail)*(.3+.7*facet)
            if n==2: snowline*=.32
            p[i,j]=rgb(mix(shade,snow,min(1,snowline)))
# Reflect the actual landscape and sky; horizontal lake ripples.
for j in range(shore,H):
    t=(j-shore)/(H-shore)
    sy=max(0,shore-1-int(t*shore*.85))
    for i in range(W):
        sx=max(0,min(W-1,i+int(3*sin(j*.8+i*.035)+9*t*sin(j*.24))))
        a=.52*(1-t*.78)
        v=mix((2,19,39),p[sx,sy],a)
        pulse=exp(-((i/W-.48)/.19)**2)*(.5+.5*sin(j*.72))
        v=mix(v,(3,148,180),.16*pulse*(1-t))
        p[i,j]=rgb(v)
for x in range(W): p[x,shore]=(16,94,127)
out=Path("src/generated");out.mkdir(parents=True,exist_ok=True)
# RGB565 keeps the immutable artwork below one MiB and avoids a decoder at boot.
raw=bytearray()
for i in range(0,len(pixels),3):
    r,g,b=pixels[i:i+3]
    v=((r>>3)<<11)|((g>>2)<<5)|(b>>3);raw.extend(v.to_bytes(2,"little"))
(out/"aurora565.bin").write_bytes(raw)
Path("target").mkdir(exist_ok=True)
def chunk(kind,payload):
    return struct.pack(">I",len(payload))+kind+payload+struct.pack(">I",zlib.crc32(kind+payload))
scan=b"".join(b"\0"+pixels[j*W*3:(j+1)*W*3] for j in range(H))
Path("target/hi5os-aurora-preview.png").write_bytes(b"\x89PNG\r\n\x1a\n"+chunk(b"IHDR",struct.pack(">IIBBBBB",W,H,8,2,0,0,0))+chunk(b"IDAT",zlib.compress(scan))+chunk(b"IEND",b""))
assert len(raw)==W*H*2
