import unittest
from smoke import REQUIRED, validate

class MarkerTests(unittest.TestCase):
    def test_valid(self): validate(b'\r\n'.join(REQUIRED))
    def test_missing_handoff(self):
        with self.assertRaises(ValueError): validate(b'\n'.join(m for m in REQUIRED if m != b'H5K: EXIT_BOOT_SERVICES_OK'))
    def test_wrong_order(self):
        with self.assertRaises(ValueError): validate(b'\n'.join(reversed(REQUIRED)))
    def test_panic(self):
        with self.assertRaises(ValueError): validate(b'\n'.join(REQUIRED)+b'\nH5K: PANIC')
    def test_firmware_failure(self):
        with self.assertRaises(ValueError): validate(b'\n'.join(REQUIRED)+b'\nH5K: EFI_STATUS=')

if __name__ == '__main__': unittest.main()
