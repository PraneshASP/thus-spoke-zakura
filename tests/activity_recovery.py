"""Live regression coverage for activity recovery after auto-mining fails."""

import os
import sqlite3
import unittest
from pathlib import Path

from regtest_support import (
    RecoveryFailureReporter,
    RegtestStack,
    request_json,
    rpc,
    wait_until,
)


class RecoveryTest(unittest.TestCase):
    def test_broadcast_recovers_after_auto_mine_failure(self):
        reporter = RecoveryFailureReporter()
        try:
            stack = RegtestStack(Path(os.environ["TSZ_TEST_SERVER"]))
            reporter.attach_stack(stack)
            with stack:
                self._exercise_recovery(stack, reporter)
        except BaseException as error:
            reporter.emit_failure(error)
            raise

    def _exercise_recovery(
        self, stack: RegtestStack, reporter: RecoveryFailureReporter
    ):
        reporter.phase("broadcast")
        stack.assert_owned_containers_running()
        initial = request_json(stack.api_url, "/api/v1/activity?limit=100")
        height_before = rpc(stack.node_url, "getblockchaininfo", [])["blocks"]
        reporter.record_height("before_auto_mine", height_before)
        counts_before = stack.proxy.counts()
        reporter.phase("auto-mine")
        stack.proxy.fail_next_generate()
        pending = request_json(
            stack.api_url,
            "/api/v1/send",
            {
                "from_account": 1,
                "to_account": 2,
                "source_pool": "orchard",
                "destination_pool": "orchard",
                "amount_zatoshi": 1000000,
                "idempotency_key": "recovery-after-auto-mine-failure",
            },
            timeout=120,
        )
        reporter.record_activity(pending)
        requested_transfer = {
            "kind": "send",
            "from_account": 1,
            "to_account": 2,
            "source_pool": "orchard",
            "destination_pool": "orchard",
            "amount_zatoshi": 1000000,
        }
        self.assertEqual(
            {field: pending[field] for field in requested_transfer}, requested_transfer
        )
        self.assertEqual(pending["status"], "broadcast")
        self.assertIsNone(pending["block_hash"])
        self.assertTrue(pending["txid"])
        self.assertEqual(stack.proxy.counts(), (counts_before[0] + 1, counts_before[1]))
        height_after_auto_mine = rpc(stack.node_url, "getblockchaininfo", [])["blocks"]
        reporter.record_height("after_auto_mine", height_after_auto_mine)
        self.assertEqual(height_after_auto_mine, height_before)
        self.assertIn(pending["txid"], rpc(stack.node_url, "getrawmempool", []))
        rows = request_json(stack.api_url, "/api/v1/activity?limit=100")
        self.assertEqual(len(rows), len(initial) + 1)
        matches = [row for row in rows if row["id"] == pending["id"]]
        self.assertEqual(len(matches), 1)
        persisted_pending = matches[0]
        self.assertTrue(persisted_pending["created_at"])
        self.assertEqual(
            persisted_pending,
            dict(pending, created_at=persisted_pending["created_at"]),
        )
        self.assertEqual(
            {
                field: persisted_pending[field]
                for field in requested_transfer
            },
            requested_transfer,
        )
        pending = persisted_pending
        reporter.record_activity(pending)

        reporter.phase("direct-mine")
        # Direct RPC bypasses the server and its mine-and-sync handler.
        rpc(stack.node_url, "generate", [1])
        inclusion_height = rpc(stack.node_url, "getblockchaininfo", [])["blocks"]
        reporter.record_height("inclusion", inclusion_height)
        included = rpc(stack.node_url, "getrawtransaction", [pending["txid"], 1])
        self.assertGreaterEqual(included["confirmations"], 1)
        expected_hash = included["blockhash"]
        self.assertTrue(expected_hash)
        # A later tip must not be mistaken for this transaction's inclusion block.
        rpc(stack.node_url, "generate", [1])
        tip = rpc(stack.node_url, "getblockchaininfo", [])
        reporter.record_height("tip", tip["blocks"])
        self.assertNotEqual(expected_hash, tip["bestblockhash"])

        reporter.phase("recovery")
        # Only production background synchronization is allowed to change the row.
        def recovered():
            stack.assert_owned_containers_running()
            stack._server_running()
            try:
                status = request_json(stack.api_url, "/api/v1/status")
                rows = request_json(stack.api_url, "/api/v1/activity?limit=100")
            except RuntimeError as error:
                if str(error) != "HTTP transport failure":
                    raise
                stack._server_running()
                return None
            sync = status["wallet_sync"]
            reporter.record_height("scanned", sync.get("fully_scanned_height"))
            matches = [row for row in rows if row["id"] == pending["id"]]
            self.assertEqual(len(matches), 1)
            row = matches[0]
            if (
                sync["state"] == "ready"
                and (sync["fully_scanned_height"] or 0) >= tip["blocks"]
                and row["status"] == "confirmed"
            ):
                return row
            return None

        confirmed = wait_until(
            recovered, timeout=120, label="background activity recovery"
        )
        reporter.record_activity(confirmed)
        expected = dict(pending, status="confirmed", block_hash=expected_hash)
        self.assertEqual(
            confirmed, expected
        )  # identity, txid, timestamp and payment fields survive
        self.assertEqual(
            {field: confirmed[field] for field in requested_transfer}, requested_transfer
        )
        self.assertEqual(stack.proxy.counts(), (counts_before[0] + 1, counts_before[1]))
        rows = request_json(stack.api_url, "/api/v1/activity?limit=100")
        self.assertEqual(len(rows), len(initial) + 1)
        self.assertEqual(
            [row for row in rows if row["txid"] == pending["txid"]], [confirmed]
        )

        uri = (stack.data_dir / "tsz.db").resolve().as_uri() + "?mode=ro"
        connection = sqlite3.connect(uri, uri=True)
        try:
            stored = connection.execute(
                "SELECT id, txid, status, block_hash FROM activity WHERE txid = ?",
                (pending["txid"],),
            ).fetchall()
            self.assertEqual(
                stored,
                [(pending["id"], pending["txid"], "confirmed", expected_hash)],
            )
            mapping = connection.execute(
                "SELECT activity_id FROM idempotency WHERE key = ?",
                ("recovery-after-auto-mine-failure",),
            ).fetchall()
            self.assertEqual(mapping, [(pending["id"],)])
        finally:
            connection.close()
        reporter.phase("cleanup")


if __name__ == "__main__":
    unittest.main()
