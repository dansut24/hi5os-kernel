#!/usr/bin/env python3
"""Guest-independent Hi5FS transaction tests using the actual Rust filesystem module."""
import pathlib, subprocess, tempfile
ROOT=pathlib.Path(__file__).resolve().parents[1]
with tempfile.TemporaryDirectory(prefix="hi5fs-host-") as td:
    d=pathlib.Path(td)
    main=d/"main.rs"
    main.write_text('extern crate alloc;\n#[path="'+str(ROOT/"src/hi5fs.rs")+'"]mod hi5fs;\nfn main(){assert!(hi5fs::self_test());println!("PASS: Hi5FS transaction, directory, checksum, corruption and write interruption model");}\n')
    subprocess.run(["rustc","--edition=2021","-O",str(main),"-o",str(d/"test")],check=True)
    subprocess.run([str(d/"test")],check=True)
