import unittest
from smoke import ROOT, REQUIRED, validate
import tomllib
import re, ipaddress

class MarkerTests(unittest.TestCase):
    def test_system_image_version_names(self):
        ver=tomllib.loads((ROOT/"Cargo.toml").read_text())["package"]["version"]
        expected=f"hi5os-v{ver}.img"
        for path in ("scripts/build.sh","scripts/smoke.py","scripts/persistence.py","scripts/network.py","scripts/routed.py","scripts/offline7zip.py","scripts/archive7zip.py",".github/workflows/kernel.yml"):
            with self.subTest(path=path):
                names=re.findall(r"hi5os-v\d+\.\d+\.\d+\.img",(ROOT/path).read_text())
                self.assertTrue(names)
                self.assertEqual(set(names),{expected})

    def test_network_fixture_addresses(self):
        network=(ROOT/"scripts/network.py").read_text()
        desktop=(ROOT/"src/desktop.rs").read_text()
        for name,source in [("network",network),("desktop",desktop)]:
            for addr in re.findall(r"(?<![\d.])(?:10|127)\.\d+\.\d+\.\d+(?![\d.])",source):
                with self.subTest(file=name,address=addr):
                    ipaddress.IPv4Address(addr)
        self.assertIn("guestfwd=tcp:10.0.2.200:8080",network)
        self.assertIn("http://10.0.2.2:{server.server_port}/hello.exe",network)
        self.assertIn("http://10.0.2.200:8080/hello.exe",network)
        self.assertIn('b"http://10.0.2.200:8080/hello.exe"',desktop)

    def test_boot_version_matches_cargo(self):
        with (ROOT / "Cargo.toml").open("rb") as f:
            version = tomllib.load(f)["package"]["version"]
        self.assertEqual(REQUIRED[0], ("H5K: BOOT v" + version).encode())
    def test_wrong_boot_version(self):
        with self.assertRaises(ValueError):
            validate(b"\n".join([b"H5K: BOOT v0.0.0", *REQUIRED[1:]]))
    def test_valid(self): validate(b'\r\n'.join(REQUIRED))
    def test_missing_handoff(self):
        with self.assertRaises(ValueError): validate(b'\n'.join(m for m in REQUIRED if m != b'H5K: EXIT_BOOT_SERVICES_OK'))
    def test_wrong_order(self):
        with self.assertRaises(ValueError): validate(b'\n'.join(reversed(REQUIRED)))
    def test_missing_memory_test(self):
        with self.assertRaises(ValueError): validate(b'\n'.join(m for m in REQUIRED if m != b'H5K: FRAME_RW_REUSE_OK'))
    def test_kernel_failure(self):
        with self.assertRaises(ValueError): validate(b'\n'.join(REQUIRED)+b'\nH5K: FAIL STACK_CHECK')
    def test_missing_desktop(self):
        with self.assertRaises(ValueError): validate(b'\n'.join(m for m in REQUIRED if m != b'H5K: DESKTOP_READY'))
    def test_panic(self):
        with self.assertRaises(ValueError): validate(b'\n'.join(REQUIRED)+b'\nH5K: PANIC')
    def test_firmware_failure(self):
        with self.assertRaises(ValueError): validate(b'\n'.join(REQUIRED)+b'\nH5K: EFI_STATUS=')

if __name__ == '__main__': unittest.main()
