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


class CitationLineTest(unittest.TestCase):
    SRC = {"src/main.rs": "fn main() {\n    KeyCode::Tab => next(),\n    KeyCode::Char('q') | KeyCode::Esc => quit(),\n}\n",
           "src/control_api.rs": "let app = Router::new()\n    .route(\"/healthz\", get(h))\n"}

    def test_row_passes_when_a_cited_line_holds_one_of_its_tokens(self):
        doc = "| `KeyCode::Tab` | main | next | src/main.rs:2 |\n| `KeyCode::Esc` / `KeyCode::Char('q')` | main | quit | src/main.rs:3, src/main.rs:99 |\n"
        self.assertEqual(cbd.misplaced_citations(doc, self.SRC), [])

    def test_row_fails_when_no_cited_line_holds_its_tokens(self):
        doc = "| `KeyCode::Tab` | main | next | src/main.rs:3 |\n"
        self.assertEqual(cbd.misplaced_citations(doc, self.SRC),
                         ["`KeyCode::Tab`: none of src/main.rs:3 contains it"])

    def test_route_rows_are_checked_by_path(self):
        ok = "| `GET /healthz` | none | 200 | empty | src/control_api.rs:2 |\n"
        bad = "| `GET /healthz` | none | 200 | empty | src/control_api.rs:1 |\n"
        self.assertEqual(cbd.misplaced_citations(ok, self.SRC), [])
        self.assertEqual(cbd.misplaced_citations(bad, self.SRC),
                         ["`GET /healthz`: none of src/control_api.rs:1 contains it"])


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
        self.assertIn("BD-99: not in the requirements.md §5 register", errs)

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



class RegisterTest(unittest.TestCase):
    REQ = (
        "| ID | Defect | Fixed by |\n|---|---|---|\n"
        "| BD-01 | Malformed config (`main.rs:75`) | REQ-CFG-003 |\n"
        "| BD-02 | Buffers (`control_api.rs:388`) | REQ-PRX-002 |\n"
        "| BD-33 | Decimal sizes misread | REQ-ORC-001 |\n"
        "Text mentioning BD-99 outside a table row is ignored.\n"
    )

    def test_register_ids_come_from_the_requirements_table(self):
        self.assertEqual(cbd.register_ids(self.REQ), ["BD-01", "BD-02", "BD-33"])

    def test_defects_are_checked_against_the_given_register(self):
        doc = entry("BD-01") + entry("BD-02")
        errs = cbd.check_defects(doc, False, lambda p, n: True, ["BD-01", "BD-02", "BD-33"])
        self.assertEqual(errs, ["BD-33: missing entry"])
        extra = cbd.check_defects(doc + entry("BD-07"), False, lambda p, n: True, ["BD-01", "BD-02"])
        self.assertEqual(extra, ["BD-07: not in the requirements.md §5 register"])

if __name__ == "__main__":
    unittest.main()
