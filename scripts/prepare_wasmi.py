#!/usr/bin/env python3
"""Prepare pinned Wasmi 0.31.2 with a small host-only fuel checkpoint hook.
The hook does not replenish fuel, grant guest capabilities or resume a trapped VM.
"""
import hashlib, pathlib, tarfile, subprocess, shutil
ROOT=pathlib.Path(__file__).resolve().parents[1]
OUT=ROOT/"target/hi5-wasmi";OUT.mkdir(parents=True,exist_ok=True)
PKG=OUT/"wasmi-0.31.2.crate"
PIN="77a8281d1d660cdf54c76a3efa9ddd0c270cada1383a995db3ccb43d166456c7"
if not PKG.exists():
    subprocess.run(["curl","--fail","--location","--retry","2","https://static.crates.io/crates/wasmi/wasmi-0.31.2.crate","-o",str(PKG)],check=True)
if hashlib.sha256(PKG.read_bytes()).hexdigest()!=PIN:raise RuntimeError("Wasmi crate checksum mismatch")
SRC=OUT/"wasmi-0.31.2"
if SRC.exists():shutil.rmtree(SRC)
with tarfile.open(PKG) as f:
    for entry in f.getmembers():
        path=pathlib.PurePosixPath(entry.name)
        if path.is_absolute() or ".." in path.parts or not path.parts or path.parts[0]!="wasmi-0.31.2":raise RuntimeError("Invalid Wasmi archive path")
        dest=OUT/path
        if entry.isdir():dest.mkdir(parents=True,exist_ok=True)
        elif entry.isfile():dest.parent.mkdir(parents=True,exist_ok=True);dest.write_bytes(f.extractfile(entry).read())
        else:raise RuntimeError("Unsupported Wasmi archive entry")
def replace_once(path,old,new):
    text=path.read_text()
    if text.count(old)!=1:raise RuntimeError(f"Wasmi patch anchor mismatch: {path}")
    path.write_text(text.replace(old,new))
store=SRC/"src/store.rs"
replace_once(store,"    total: u64,\n}\n\nimpl Fuel {","    total: u64,\n    hi5_poll: Option<fn() -> bool>,\n    hi5_next_poll: u64,\n}\n\nimpl Fuel {\n    pub(crate) fn hi5_poll_checkpoint(&mut self) -> Result<(), TrapCode> {\n        let consumed = self.fuel_consumed();\n        if consumed >= self.hi5_next_poll {\n            self.hi5_next_poll = consumed.saturating_add(65_536);\n            if self.hi5_poll.is_some_and(|poll| poll()) { return Err(TrapCode::OutOfFuel); }\n        }\n        Ok(())\n    }")
replace_once(store,"    pub fn add_fuel(&mut self, delta: u64) -> Result<(), FuelError> {","    /// Register a host-only cooperative cancellation checkpoint. No fuel is added.\n    pub fn set_hi5_fuel_poll(&mut self, poll: fn() -> bool) {\n        self.inner.fuel.hi5_poll = Some(poll);\n        self.inner.fuel.hi5_next_poll = 0;\n    }\n\n    pub fn add_fuel(&mut self, delta: u64) -> Result<(), FuelError> {")
replace_once(SRC/"src/engine/executor.rs","        self.ctx.fuel_mut().consume_fuel(block_fuel.to_u64())?;","        self.ctx.fuel_mut().hi5_poll_checkpoint()?;\n        self.ctx.fuel_mut().consume_fuel(block_fuel.to_u64())?;")
notices=ROOT/"target/hi5-js/notices";notices.mkdir(parents=True,exist_ok=True)
for f in SRC.iterdir():
    if f.is_file() and f.name.upper().startswith(("LICENSE","COPYING","NOTICE")):shutil.copyfile(f,notices/("wasmi-0.31.2-"+f.name+".txt"))
# The crates.io tarball does not contain the upstream workspace-level licenses.
for name,digest in [
    ("LICENSE-APACHE","3d118918fc831c13ea55d6a92f80a4c5ee06cd45fb8ca64ba6e87212f8641cef"),
    ("LICENSE-MIT","aad39b911044a685b258fe50edb168596ad7172c225fdb8229f9f853eeed45ca"),
]:
    dest=notices/("wasmi-0.31.2-"+name+".txt")
    if not dest.exists():
        subprocess.run(["curl","--fail","--location","--retry","2","https://raw.githubusercontent.com/wasmi-labs/wasmi/0218dfc74b4c4a83261d46d90ac83fb513fd6b3f/"+name,"-o",str(dest)],check=True)
    if hashlib.sha256(dest.read_bytes()).hexdigest()!=digest:raise RuntimeError("Wasmi license checksum mismatch")
(notices/"wasmi-0.31.2-HI5-PATCH.txt").write_text("Source: https://static.crates.io/crates/wasmi/wasmi-0.31.2.crate\nSHA-256: "+PIN+"\nLocal modification: host-only callback at fuel checkpoints (at most once per 65536 consumed fuel units), implemented by scripts/prepare_wasmi.py. Original fuel charges remain unchanged. No guest API or added permissions.\n")
print("Pinned Wasmi cooperative fuel checkpoint ready")
