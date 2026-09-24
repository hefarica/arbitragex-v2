import tempfile
import unittest
from pathlib import Path
from forensic_integrity.capture import validate_url,redact,save_capture
from forensic_integrity.inventory import parse_maps
class UtilityTests(unittest.TestCase):
    def test_https_public(self):self.assertEqual(validate_url('https://example.org/path'),'https://example.org/path')
    def test_http_not_silent(self):
        with self.assertRaises(ValueError):validate_url('http://example.org/path')
    def test_local_explicit(self):self.assertEqual(validate_url('http://127.0.0.1:8080/status',True),'http://127.0.0.1:8080/status')
    def test_url_credentials_refused(self):
        with self.assertRaises(ValueError):validate_url('https://user:password@example.org')
    def test_query_credentials_refused(self):
        with self.assertRaises(ValueError):validate_url('https://example.org?api_key=secret')
    def test_redaction(self):self.assertEqual(redact({'api_key':'secret','value':'0'}),{'api_key':'[REDACTED]','value':'0'})
    def test_capture_never_overwrites(self):
        with tempfile.TemporaryDirectory() as tmp:
            p=Path(tmp)/'evidence.json';save_capture(p,{'test':1})
            with self.assertRaises(FileExistsError):save_capture(p,{'test':2})
    def test_bad_csv_width_retained(self):
        with tempfile.TemporaryDirectory() as tmp:
            p=Path(tmp)/'map.txt';p.write_text('## HOJA 1\n```csv\na,b\n1,2,3\n```\n')
            r=parse_maps([p]);self.assertEqual(len(r['parse_problems']),1);self.assertEqual(r['rows'][0]['raw'],'1,2,3')
    def test_undefined_dependency_reported(self):
        with tempfile.TemporaryDirectory() as tmp:
            p=Path(tmp)/'map.txt';p.write_text('## X\n| Field_ID | Input_Fields |\n|---|---|\n| F001 | P999 |\n')
            r=parse_maps([p]);self.assertEqual(r['undefined_dependencies'][0]['dependency'],'P999')
if __name__=='__main__':unittest.main()
