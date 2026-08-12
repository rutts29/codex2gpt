import importlib.util
import json
import tempfile
import unittest
from pathlib import Path


MODULE_PATH = Path(__file__).with_name("check-deps.py")


def load_module():
    spec = importlib.util.spec_from_file_location("check_deps", MODULE_PATH)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


class CheckDepsTests(unittest.TestCase):
    def test_parse_cargo_lock_keeps_only_crates_io_packages(self):
        check_deps = load_module()
        lockfile = """
version = 4

[[package]]
name = "local"
version = "0.1.0"

[[package]]
name = "serde"
version = "1.0.228"
source = "registry+https://github.com/rust-lang/crates.io-index"

[[package]]
name = "git-dep"
version = "0.2.0"
source = "git+https://example.invalid/repo"
"""
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "Cargo.lock"
            path.write_text(lockfile)

            packages = check_deps.parse_cargo_lock(path)

        self.assertEqual(packages, [("serde", "1.0.228")])

    def test_query_osv_batch_posts_crates_io_package_versions(self):
        check_deps = load_module()
        requests = []

        def opener(request, timeout):
            requests.append((request, timeout))
            return Response(json.dumps({"results": [{}, {"vulns": [{"id": "RUSTSEC-1"}]}]}))

        findings = check_deps.query_osv_batch(
            [("serde", "1.0.228"), ("tokio", "1.52.3")],
            opener=opener,
        )

        payload = json.loads(requests[0][0].data.decode("utf-8"))
        self.assertEqual(payload["queries"][0]["package"], {"name": "serde", "ecosystem": "crates.io"})
        self.assertEqual(payload["queries"][0]["version"], "1.0.228")
        self.assertEqual(findings, [{"package": "tokio", "version": "1.52.3", "vulnerabilities": [{"id": "RUSTSEC-1"}]}])


class Response:
    def __init__(self, body):
        self.body = body.encode("utf-8")

    def __enter__(self):
        return self

    def __exit__(self, exc_type, exc, traceback):
        return False

    def read(self):
        return self.body


if __name__ == "__main__":
    unittest.main()
