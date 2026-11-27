#!/usr/bin/env python3
"""Extract a named Hi5Office file from a powered-off Hi5OS data image, read-only."""
import argparse,pathlib,struct,zlib
HEADER=b"HI5OS-DATA-V1-7D1F6C0284A9E3B5"
def read_slot(path,slot,docx=False):
    if not 0<=slot<4:raise ValueError("Slot must be 1–4")
    base=4162+slot*576
    banks=(base+64,base+320) if docx else (base,base+32)
    span=256 if docx else 32
    magic=b"H5DOCX01" if docx else b"H5OFF001"
    valid=[]
    with pathlib.Path(path).open("rb") as f:
        h=f.read(512)
        if h!=HEADER.ljust(512,b"\0"):raise ValueError("Not a Hi5OS data disk")
        for bank in banks:
            f.seek(bank*512);h=f.read(512)
            if len(h)!=512 or h[:8]!=magic or zlib.crc32(h[:508])!=struct.unpack_from("<I",h,508)[0]:continue
            gen,n,crc=struct.unpack_from("<III",h,8)
            if not gen or not 0<n<=(span-1)*512:continue
            data=f.read(n)
            if len(data)==n and zlib.crc32(data)==crc:valid.append((gen,data))
    if not valid:raise ValueError("No valid saved document")
    return max(valid,key=lambda v:v[0])[1]
if __name__=="__main__":
    p=argparse.ArgumentParser(description=__doc__);p.add_argument("image",type=pathlib.Path);p.add_argument("slot",type=int,choices=range(1,5));p.add_argument("--docx",action="store_true");p.add_argument("-o","--output",type=pathlib.Path);a=p.parse_args()
    out=a.output or pathlib.Path(f"Document {a.slot}."+("docx" if a.docx else "h5doc"))
    data=read_slot(a.image,a.slot-1,a.docx)
    with out.open("xb") as f:f.write(data)
    print(out)
