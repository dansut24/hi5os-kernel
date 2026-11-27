#!/usr/bin/env python3
"""Pinned QuickJS-NG WASI module; no npm install or package scripts."""
import hashlib, pathlib, subprocess, tarfile, shutil, tomllib, os, sys
ROOT=pathlib.Path(__file__).resolve().parents[1]
OUT=ROOT/"target/hi5-js"; OUT.mkdir(parents=True,exist_ok=True)
PKG=OUT/"quickjs-wasi-3.7.0.tgz"
URL="https://registry.npmjs.org/quickjs-wasi/-/quickjs-wasi-3.7.0.tgz"
if not PKG.exists(): subprocess.run(["curl","--fail","--location","--retry","2",URL,"-o",str(PKG)],check=True)
assert hashlib.sha256(PKG.read_bytes()).hexdigest()=="bf1cc7779f8ba05e7d77bba96464c78b3121197fd15e2ff8c00c15f2a3f26036","QuickJS package pin mismatch"
notices=OUT/"notices"; notices.mkdir(exist_ok=True)
with tarfile.open(PKG) as f:
    wasm=f.extractfile("package/quickjs.wasm").read()
    assert hashlib.sha256(wasm).hexdigest()=="c0bd8f296e18cf074969f6401b9ecb72609a1323818ec2b9bf7a8f8d740d1363","QuickJS WASM pin mismatch"
    (OUT/"quickjs.wasm").write_bytes(wasm)
    (notices/"quickjs-wasi-LICENSE.txt").write_bytes(f.extractfile("package/LICENSE").read())
for name,url in [
    ("quickjs-ng-LICENSE.txt","https://raw.githubusercontent.com/quickjs-ng/quickjs/d7215b0ad3fc2c562e8317fd9bca058fd4863804/LICENSE"),
]:
    dest=notices/name
    if not dest.exists():subprocess.run(["curl","--fail","--location","--retry","2",url,"-o",str(dest)],check=True)
(notices/"SOURCES.txt").write_text("QuickJS-WASI 3.7.0: https://github.com/vercel-labs/quickjs-wasi/tree/2d8504c47bbb22bced32ec3c9e93e8c51ed73cac\nQuickJS-NG: https://github.com/quickjs-ng/quickjs/tree/d7215b0ad3fc2c562e8317fd9bca058fd4863804\nWASI SDK 32: https://github.com/WebAssembly/wasi-sdk/releases/tag/wasi-sdk-32\nWASI libc notices: https://github.com/WebAssembly/wasi-libc/blob/main/LICENSE\nWasmi and dependencies: Cargo.lock\n")
if "--notices" in sys.argv:
    reg=pathlib.Path(os.environ.get("CARGO_HOME",str(pathlib.Path.home()/".cargo")))/"registry/src"
    for pkg in tomllib.loads((ROOT/"Cargo.lock").read_text())["package"]:
        if "source" not in pkg:continue
        for src in reg.glob("*/"+pkg["name"]+"-"+pkg["version"]):
            for lic in src.iterdir():
                if lic.is_file() and lic.name.upper().startswith(("LICENSE","COPYING","NOTICE")):
                    shutil.copyfile(lic,notices/(pkg["name"]+"-"+pkg["version"]+"-"+lic.name+".txt"))
print("Pinned QuickJS WASI module:",len(wasm),"bytes")
