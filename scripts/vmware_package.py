#!/usr/bin/env python3
"""Build a VMware system-only upgrade from a qualified raw FAT32 UEFI image.
Do not attach, read or modify any user disk. Output format matches CI #101.
"""
import argparse, hashlib, json, os, pathlib, shutil, struct, uuid, zlib
SECTOR=512
TOTAL=262144
START=2048
COUNT=131072
END=START+COUNT-1
DISK_GUID=uuid.UUID("9d8c7cc2-60aa-49fb-a478-4a22ffaa55fe")
PART_GUID=uuid.UUID("64c94bed-6eae-45b6-a99b-50dd904a188b")
ESP=uuid.UUID("c12a7328-f81f-11d2-ba4b-00a0c93ec93b")
def crc(b):return zlib.crc32(b)&0xffffffff
def sha(path):
    with path.open("rb") as f:return hashlib.file_digest(f,"sha256").hexdigest()
def gpt(current,other,table,entries):
    h=bytearray(512)
    struct.pack_into("<8sIIIIQQQQ16sQIII",h,0,b"EFI PART",0x10000,92,0,0,current,other,34,TOTAL-34,DISK_GUID.bytes_le,table,128,128,crc(entries))
    struct.pack_into("<I",h,16,crc(h[:92]));return h
def make(raw,out):
    if raw.stat().st_size!=COUNT*SECTOR:raise ValueError("Expected 64 MiB system image")
    out.mkdir(parents=True,exist_ok=False)
    image=out/"Hi5OS-system-flat.vmdk"
    with raw.open("rb") as src,image.open("xb+") as dst:
        boot=bytearray(src.read(512))
        if boot[510:512]!=b"\x55\xaa" or boot[82:90]!=b"FAT32   " or struct.unpack_from("<H",boot,11)[0]!=512:raise ValueError("Expected FAT32 system volume")
        backup=struct.unpack_from("<H",boot,50)[0]
        if not 0<backup<struct.unpack_from("<H",boot,14)[0]:raise ValueError("Invalid FAT32 backup boot sector")
        dst.truncate(TOTAL*SECTOR)
        mbr=bytearray(512);struct.pack_into("<B3sB3sII",mbr,446,0,b"\x00\x02\x00",0xee,b"\xff\xff\xff",1,TOTAL-1);mbr[510:]=b"\x55\xaa";dst.write(mbr)
        entries=bytearray(128*128);struct.pack_into("<16s16sQQQ",entries,0,ESP.bytes_le,PART_GUID.bytes_le,START,END,0);name="Hi5OS EFI".encode("utf-16le");entries[56:56+len(name)]=name
        dst.seek(SECTOR);dst.write(gpt(1,TOTAL-1,2,entries));dst.write(entries)
        dst.seek((TOTAL-33)*SECTOR);dst.write(entries);dst.write(gpt(TOTAL-1,1,TOTAL-33,entries))
        dst.seek(START*SECTOR);src.seek(0);shutil.copyfileobj(src,dst,1024*1024)
        # FAT BPB hidden-sector fields match the enclosing partition.
        for lba in (0,backup):
            dst.seek((START+lba)*SECTOR+28);dst.write(struct.pack("<I",START))
        dst.flush()
        # Verify both GPT tables and exact payload, allowing only the two BPB patches.
        for lba,table in ((1,2),(TOTAL-1,TOTAL-33)):
            dst.seek(lba*SECTOR);h=bytearray(dst.read(512));v=struct.unpack_from("<I",h,16)[0];struct.pack_into("<I",h,16,0)
            assert h[:8]==b"EFI PART" and crc(h[:92])==v
            dst.seek(table*SECTOR);assert dst.read(len(entries))==entries
        src.seek(0);dst.seek(START*SECTOR)
        for off in range(0,COUNT*SECTOR,1024*1024):
            expected=bytearray(src.read(1024*1024))
            for pos in (28,backup*SECTOR+28):
                if off<=pos<off+len(expected):struct.pack_into("<I",expected,pos-off,START)
            assert dst.read(len(expected))==expected
    digest=sha(image)
    desc=out/"Hi5OS-system.vmdk"
    desc.write_text(f'''# Disk DescriptorFile
version=1
encoding="UTF-8"
CID={digest[:8]}
parentCID=ffffffff
createType="monolithicFlat"

RW {TOTAL} FLAT "Hi5OS-system-flat.vmdk" 0

ddb.adapterType = "ide"
ddb.geometry.cylinders = "260"
ddb.geometry.heads = "16"
ddb.geometry.sectors = "63"
ddb.virtualHWVersion = "17"
''',encoding="ascii")
    manifest={"schema":1,"build":os.environ.get("GITHUB_RUN_NUMBER","local"),"commit":os.environ.get("GITHUB_SHA","local"),"raw_sha256":sha(raw),"files":{p.name:sha(p) for p in (desc,image)}}
    (out/"manifest.json").write_text(json.dumps(manifest,indent=2)+"\n")
    (out/"README-Upgrade.txt").write_text("Power off (not suspend), consolidate snapshots, exit VMware Workstation, back up the entire VM folder. Replace ONLY Hi5OS-system.vmdk and Hi5OS-system-flat.vmdk together. Keep VMX and data disks. Optional Windows PowerShell helper: .\\Upgrade-Hi5OS.ps1 -VmFolder 'C:\\VMs\\Hi5OS' . Manual backup remains required. Restore BOTH system files if startup fails. SHA-256 detects corruption; this is not a signed native update.\n")
    print("PASS: VMware GPT, FAT32 offsets and byte-exact system payload verified",flush=True)
if __name__=="__main__":
    p=argparse.ArgumentParser();p.add_argument("raw",type=pathlib.Path);p.add_argument("out",type=pathlib.Path);a=p.parse_args();make(a.raw,a.out)
