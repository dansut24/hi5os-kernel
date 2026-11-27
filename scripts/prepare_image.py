#!/usr/bin/env python3
"""Pin upstream image decoder by immutable commit and Git blob hash."""
import pathlib,subprocess,hashlib
ROOT=pathlib.Path(__file__).resolve().parents[1]
OUT=ROOT/"target/hi5-image";OUT.mkdir(parents=True,exist_ok=True)
H=OUT/"stb_image.h"
if not H.exists():subprocess.run(["curl","--fail","--location","--retry","2","https://raw.githubusercontent.com/nothings/stb/31c1ad37456438565541f4919958214b6e762fb4/stb_image.h","-o",str(H)],check=True)
b=H.read_bytes();assert hashlib.sha1(b"blob "+str(len(b)).encode()+b"\0"+b).hexdigest()=="9eedabedc45b3e6fd88fae6f14a160b4d53272ec","stb_image pinned blob mismatch"
obj=OUT/"hi5image.obj"
subprocess.run(["clang","--target=x86_64-pc-windows-msvc","-ffreestanding","-fno-builtin","-fno-stack-protector","-mno-stack-arg-probe","-O2","-I"+str(ROOT/"native/tls/include"),"-I"+str(ROOT/"native/image/include"),"-I"+str(OUT),"-c",str(ROOT/"native/image/hi5_image.c"),"-o",str(obj)],check=True)
subprocess.run(["lld-link","/lib","/machine:x64","/out:"+str(OUT/"hi5image.lib"),str(obj)],check=True)
# Link the decoder independently before building the full kernel. This is a
# link-only probe, not a bootable image or a runtime decoding test.
probe=OUT/"image-link-probe.efi"
subprocess.run(["lld-link","/entry:hi5_image_decode","/subsystem:efi_application","/nodefaultlib","/machine:x64","/out:"+str(probe),str(OUT/"hi5image.lib")],check=True)
probe.unlink()
# The upstream header contains the complete dual-license notice.
license_text=H.read_text();start=license_text.rfind("ALTERNATIVE A - MIT License");assert start>=0,"Upstream license marker missing";(OUT/"STB-LICENSE.txt").write_text(license_text[start:])
print("Pinned PNG/JPEG decoder built")
