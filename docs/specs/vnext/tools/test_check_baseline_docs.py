"""Tests for check_baseline_docs.py (Phase 0 baseline-document completeness)."""
import os
import sys
import unittest

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import check_baseline_docs as cbd  # noqa: E402

SRC_ROUTES = '''
    let app = Router::new()
        .route("/v1/ensure", post(h_ensure))
        .route("/healthz", get(h_health))
'''
SRC_KEYS = '''
    match key.code {
        KeyCode::Char('q') => {}
        KeyCode::Tab => {}
        KeyCode::F(5) => {}
    }
'''


class InventoryTest(unittest.TestCase):
    def test_routes_are_method_and_path(self):
        self.assertEqual(cbd.routes(SRC_ROUTES), {"POST /v1/ensure", "GET /healthz"})

    def test_keycodes_keep_their_payload(self):
        self.assertEqual(cbd.keycodes(SRC_KEYS), {"KeyCode::Char('q')", "KeyCode::Tab", "KeyCode::F(5)"})

    def test_token_counts_only_on_a_line_with_a_citation(self):
        doc = "| `GET /healthz` | 200 | src/control_api.rs:409 |\n| `POST /v1/ensure` | no citation |\n"
        self.assertEqual(cbd.missing_citations(doc, {"GET /healthz", "POST /v1/ensure"}), ["POST /v1/ensure"])


def entry(bd, status="confirmed — reproduced", evidence="`src/main.rs:75`"):
    return (f"### {bd} — title\n- **Evidence (c89f278):** {evidence}\n- **Repro:** run it\n"
            f"- **Status:** {status}\n- **Fixed by:**\n\n")


class DefectsTest(unittest.TestCase):
    def full(self, over=None):
        over = over or {}
        return "".join(entry(f"BD-{i:02d}", **over.get(i, {})) for i in range(1, 33))

    def test_complete_register_passes(self):
        self.assertEqual(cbd.check_defects(self.full(), False, lambda p, n: True), [])

    def test_missing_entry_is_reported(self):
        doc = self.full().replace("### BD-07 — title", "### BD-99 — title")
        errs = cbd.check_defects(doc, False, lambda p, n: True)
        self.assertIn("BD-07: missing entry", errs)
        self.assertIn("BD-99: not in the BD-01…BD-32 register", errs)

    def test_citation_must_exist_at_baseline(self):
        errs = cbd.check_defects(self.full(), False, lambda p, n: n < 75)
        self.assertTrue(any(e.startswith("BD-01: src/main.rs:75 does not exist") for e in errs), errs)

    def test_blocked_fails_unless_allowed(self):
        doc = self.full({32: {"status": "BLOCKED — needs R0 capture"}})
        self.assertEqual(cbd.check_defects(doc, False, lambda p, n: True), ["BD-32: still BLOCKED"])
        self.assertEqual(cbd.check_defects(doc, True, lambda p, n: True), [])

    def test_git_command_is_valid_evidence(self):
        doc = self.full({9: {"evidence": "`git ls-tree -r --name-only c89f278 | grep legacy.zip`"}})
        self.assertEqual(cbd.check_defects(doc, False, lambda p, n: True), [])

    def test_status_needs_a_reason(self):
        doc = self.full({3: {"status": "confirmed"}})
        self.assertIn("BD-03: Status must be 'confirmed|disputed|BLOCKED — <reason>'",
                      cbd.check_defects(doc, False, lambda p, n: True))


if __name__ == "__main__":
    unittest.main()
