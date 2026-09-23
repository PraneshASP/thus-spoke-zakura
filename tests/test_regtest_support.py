import json
import os
import subprocess
import tempfile
import threading
import unittest
from pathlib import Path
from unittest.mock import patch
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from urllib.error import HTTPError
from urllib.request import Request, urlopen

from regtest_support import GenerateFaultProxy, RegtestStack, request_json, rpc, wait_until


class RecordingRpcServer:
    def __enter__(self):
        owner = self

        class Handler(BaseHTTPRequestHandler):
            def do_POST(self):  # noqa: N802 - HTTP handler API
                size = int(self.headers["Content-Length"])
                request = json.loads(self.rfile.read(size))
                with owner.lock:
                    owner.requests.append(request)
                if request["method"] == "upstream_error":
                    status = 503
                    response = {
                        "jsonrpc": "2.0",
                        "id": request["id"],
                        "result": None,
                        "error": {"code": -8, "message": "upstream failure"},
                    }
                elif request["method"] == "rpc_error":
                    status = 200
                    response = {
                        "jsonrpc": "2.0",
                        "id": request["id"],
                        "result": None,
                        "error": {"code": -1, "message": "RPC failure"},
                    }
                elif request["method"] == "missing_result":
                    status = 200
                    response = {"jsonrpc": "2.0", "id": request["id"]}
                elif request["method"] == "mismatched_id":
                    status = 200
                    response = {
                        "jsonrpc": "2.0",
                        "id": "different-id",
                        "result": {"unexpected": True},
                    }
                else:
                    status = 200
                    response = {
                        "jsonrpc": "2.0",
                        "id": request["id"],
                        "result": {
                            "method": request["method"],
                            "params": request["params"],
                        },
                    }
                encoded = json.dumps(response).encode()
                self.send_response(status)
                self.send_header("Content-Type", "application/json")
                self.send_header("Content-Length", str(len(encoded)))
                self.end_headers()
                self.wfile.write(encoded)

            def log_message(self, format, *args):
                return

        self.lock = threading.Lock()
        self.requests = []
        self.httpd = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
        self.thread = threading.Thread(target=self.httpd.serve_forever, daemon=True)
        self.thread.start()
        self.url = f"http://127.0.0.1:{self.httpd.server_port}"
        return self

    def __exit__(self, exc_type, exc, traceback):
        self.httpd.shutdown()
        self.httpd.server_close()
        self.thread.join(timeout=2)


class JsonEndpoint:
    def __enter__(self):
        owner = self

        class Handler(BaseHTTPRequestHandler):
            def respond(self, body):
                size = int(self.headers.get("Content-Length", "0"))
                raw_body = self.rfile.read(size)
                owner.requests.append(
                    (self.command, self.path, self.headers.get("Content-Length"), raw_body)
                )
                if self.path == "/failure":
                    status, encoded = 418, b'{"message":"private detail"}'
                elif self.path == "/bad-json":
                    status, encoded = 200, b"not json"
                else:
                    status = 200
                    encoded = json.dumps(
                        {
                            "method": self.command,
                            "path": self.path,
                            "body": json.loads(raw_body) if raw_body else None,
                        }
                    ).encode()
                self.send_response(status)
                self.send_header("Content-Type", "application/json")
                self.send_header("Content-Length", str(len(encoded)))
                self.end_headers()
                self.wfile.write(encoded)

            def do_GET(self):  # noqa: N802 - HTTP handler API
                self.respond(b"")

            def do_POST(self):  # noqa: N802 - HTTP handler API
                self.respond(b"")

            def log_message(self, format, *args):
                return

        self.requests = []
        self.httpd = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
        self.thread = threading.Thread(target=self.httpd.serve_forever, daemon=True)
        self.thread.start()
        self.url = f"http://127.0.0.1:{self.httpd.server_port}"
        return self

    def __exit__(self, exc_type, exc, traceback):
        self.httpd.shutdown()
        self.httpd.server_close()
        self.thread.join(timeout=2)


def post_json(url, payload):
    request = Request(
        url,
        data=json.dumps(payload).encode(),
        headers={"Content-Type": "application/json"},
        method="POST",
    )
    try:
        with urlopen(request, timeout=2) as response:
            return response.status, json.loads(response.read())
    except HTTPError as error:
        try:
            return error.code, json.loads(error.read())
        finally:
            error.close()


class GenerateFaultProxyTests(unittest.TestCase):
    def test_forwards_unarmed_generate_and_regular_rpc_unchanged(self):
        with RecordingRpcServer() as upstream, GenerateFaultProxy(upstream.url) as proxy:
            regular = {"jsonrpc": "2.0", "id": "regular", "method": "getblockchaininfo", "params": []}
            generate = {"jsonrpc": "2.0", "id": "generate", "method": "generate", "params": [1]}

            self.assertEqual(post_json(proxy.url, regular), (200, {"jsonrpc": "2.0", "id": "regular", "result": {"method": "getblockchaininfo", "params": []}}))
            self.assertEqual(post_json(proxy.url, generate), (200, {"jsonrpc": "2.0", "id": "generate", "result": {"method": "generate", "params": [1]}}))
            self.assertEqual(upstream.requests, [regular, generate])

    def test_armed_proxy_rejects_only_generate_once_with_matching_id(self):
        with RecordingRpcServer() as upstream, GenerateFaultProxy(upstream.url) as proxy:
            proxy.fail_next_generate()
            with self.assertRaisesRegex(RuntimeError, "already armed"):
                proxy.fail_next_generate()
            read = {"jsonrpc": "2.0", "id": "read", "method": "getblockchaininfo", "params": []}
            generate = {"jsonrpc": "2.0", "id": "fault-id", "method": "generate", "params": [1]}

            self.assertEqual(post_json(proxy.url, read)[0], 200)
            status, fault = post_json(proxy.url, generate)

            self.assertEqual(status, 500)
            self.assertEqual(fault["jsonrpc"], "2.0")
            self.assertEqual(fault["id"], "fault-id")
            self.assertIsNone(fault["result"])
            self.assertEqual(fault["error"]["code"], -32603)
            self.assertEqual(post_json(proxy.url, generate)[0], 200)
            self.assertEqual(upstream.requests, [read, generate])
            self.assertEqual(proxy.counts(), (1, 1))

    def test_preserves_upstream_errors_and_returns_explicit_transport_failure(self):
        with RecordingRpcServer() as upstream, GenerateFaultProxy(upstream.url) as proxy:
            request = {"jsonrpc": "2.0", "id": "upstream-id", "method": "upstream_error", "params": []}
            self.assertEqual(
                post_json(proxy.url, request),
                (503, {"jsonrpc": "2.0", "id": "upstream-id", "result": None, "error": {"code": -8, "message": "upstream failure"}}),
            )

        with GenerateFaultProxy("http://127.0.0.1:1") as proxy:
            status, response = post_json(
                proxy.url,
                {"jsonrpc": "2.0", "id": "transport-id", "method": "getblockchaininfo", "params": []},
            )

        self.assertEqual(status, 502)
        self.assertEqual(response["id"], "transport-id")
        self.assertIsNone(response["result"])
        self.assertEqual(response["error"]["code"], -32000)

    def test_independent_proxies_do_not_share_their_armed_fault(self):
        with RecordingRpcServer() as upstream:
            with GenerateFaultProxy(upstream.url) as first, GenerateFaultProxy(upstream.url) as second:
                first.fail_next_generate()
                request = {"jsonrpc": "2.0", "id": "generate", "method": "generate", "params": [1]}
                self.assertEqual(post_json(second.url, request)[0], 200)
                self.assertEqual(post_json(first.url, request)[0], 500)

                self.assertEqual(first.counts(), (1, 0))
                self.assertEqual(second.counts(), (0, 1))
                self.assertEqual(upstream.requests, [request])

    def test_armed_fault_ignores_generate_with_other_params(self):
        with RecordingRpcServer() as upstream, GenerateFaultProxy(upstream.url) as proxy:
            proxy.fail_next_generate()
            other_generate = {"jsonrpc": "2.0", "id": "two", "method": "generate", "params": [2]}
            target_generate = {"jsonrpc": "2.0", "id": "one", "method": "generate", "params": [1]}
            self.assertEqual(post_json(proxy.url, other_generate)[0], 200)
            self.assertEqual(post_json(proxy.url, target_generate)[0], 500)
            self.assertEqual(upstream.requests, [other_generate])
            self.assertEqual(proxy.counts(), (1, 1))


class WaitUntilTests(unittest.TestCase):
    def test_returns_first_truthy_result(self):
        self.assertEqual(wait_until(lambda: {"state": "ready"}, timeout=0.1, label="ready"), {"state": "ready"})

    def test_deadline_is_labeled_and_assertion_failures_propagate(self):
        with self.assertRaisesRegex(TimeoutError, "never-ready"):
            wait_until(lambda: False, timeout=0.02, label="never-ready")

        def invalid_schema():
            raise AssertionError("unexpected schema")

        with self.assertRaisesRegex(AssertionError, "unexpected schema"):
            wait_until(invalid_schema, timeout=0.1, label="schema")


class HttpHelperTests(unittest.TestCase):
    def test_request_json_uses_get_without_body_and_post_with_json(self):
        with JsonEndpoint() as endpoint:
            self.assertEqual(
                request_json(endpoint.url, "/read"),
                {"method": "GET", "path": "/read", "body": None},
            )
            self.assertEqual(
                request_json(endpoint.url, "/write", {"height": 1}),
                {"method": "POST", "path": "/write", "body": {"height": 1}},
            )
            self.assertEqual(
                endpoint.requests,
                [
                    ("GET", "/read", None, b""),
                    ("POST", "/write", str(len(b'{"height": 1}')), b'{"height": 1}'),
                ],
            )

    def test_request_json_rejects_non_success_without_response_body(self):
        with JsonEndpoint() as endpoint:
            with self.assertRaisesRegex(RuntimeError, "HTTP 418") as failure:
                request_json(endpoint.url, "/failure")
        self.assertNotIn("private detail", str(failure.exception))

    def test_request_json_rejects_malformed_success_body(self):
        with JsonEndpoint() as endpoint:
            with self.assertRaises(json.JSONDecodeError):
                request_json(endpoint.url, "/bad-json")

    def test_rpc_returns_result_and_rejects_http_json_rpc_and_missing_results(self):
        with RecordingRpcServer() as upstream:
            self.assertEqual(rpc(upstream.url, "getblockchaininfo", []), {"method": "getblockchaininfo", "params": []})
            self.assertIn("id", upstream.requests[0])
            with self.assertRaisesRegex(RuntimeError, "HTTP 503"):
                rpc(upstream.url, "upstream_error", [])
            with self.assertRaisesRegex(RuntimeError, "contains an error"):
                rpc(upstream.url, "rpc_error", [])
            with self.assertRaisesRegex(RuntimeError, "missing result"):
                rpc(upstream.url, "missing_result", [])
            with self.assertRaisesRegex(RuntimeError, "ID mismatch"):
                rpc(upstream.url, "mismatched_id", [])


class RegtestStackCleanupTests(unittest.TestCase):
    def test_builds_a_healthy_funded_fixture_from_owned_resources(self):
        with tempfile.TemporaryDirectory() as directory:
            server = Path(directory) / "tsz-server"
            server.write_text("#!/bin/sh\n")
            os.chmod(server, 0o700)
            commands = []

            def fake_run(command, **kwargs):
                command = list(command)
                commands.append(command)
                if command[:3] == ["docker", "inspect", "--format"]:
                    if command[3] == "{{.State.Running}}":
                        output = "true\n"
                    elif command[-1].endswith("-zakura"):
                        output = "19001\n"
                    else:
                        output = "19067\n"
                    return subprocess.CompletedProcess(command, 0, stdout=output)
                return subprocess.CompletedProcess(command, 0, stdout="")

            class FakeProcess:
                def __init__(self):
                    self.running = True

                def poll(self):
                    return None if self.running else 0

                def terminate(self):
                    self.running = False

                def wait(self, timeout):
                    self.running = False
                    return 0

                def kill(self):
                    self.running = False

            process = FakeProcess()
            stack = RegtestStack(server)
            with (
                patch("regtest_support.subprocess.run", side_effect=fake_run),
                patch("regtest_support.subprocess.Popen", return_value=process) as popen,
                patch("regtest_support.rpc", return_value={"blocks": 1}),
                patch(
                    "regtest_support.request_json",
                    side_effect=lambda base, path, body=None, timeout=5: {
                        "/api/v1/health": {"instance": stack.prefix},
                        "/api/v1/status": {"wallet_sync": {"state": "ready"}},
                        "/api/v1/accounts": [{"id": 1, "orchard_zatoshi": 500000000}],
                    }[path],
                ),
            ):
                with stack:
                    self.assertTrue(stack.prefix.startswith("tsz-recovery-"))
                    self.assertEqual(stack.node_url, "http://127.0.0.1:19001")
                    self.assertIsInstance(stack.proxy, GenerateFaultProxy)
                    self.assertTrue(stack.data_dir.is_dir())
                    environment = popen.call_args.kwargs["env"]
                    self.assertEqual(environment["TSZ_ZAKURA_RPC"], stack.proxy.url)
                    self.assertEqual(environment["TSZ_LIGHTWALLETD"], "http://127.0.0.1:19067")
                    self.assertEqual(environment["TSZ_INSTANCE"], stack.prefix)
                    self.assertTrue(environment["TSZ_LISTEN"].startswith("127.0.0.1:"))

            self.assertIn(["docker", "network", "create", stack.prefix], commands)
            self.assertIn(["docker", "volume", "create", f"{stack.prefix}-chain"], commands)
            self.assertIn(["docker", "volume", "create", f"{stack.prefix}-lightwalletd"], commands)
            node_command = next(command for command in commands if command[:3] == ["docker", "run", "--detach"] and command[command.index("--name") + 1].endswith("-zakura"))
            self.assertIn("127.0.0.1::18232", node_command)
            self.assertNotIn("127.0.0.1::18233", node_command)
            self.assertEqual(stack.cleanup_errors, [])

    def test_failed_setup_cleans_only_registered_resources_and_keeps_original_error(self):
        with tempfile.TemporaryDirectory() as directory:
            server = Path(directory) / "tsz-server"
            server.write_text("#!/bin/sh\n")
            os.chmod(server, 0o700)
            stack = RegtestStack(server)
            commands = []

            def fake_run(command, **kwargs):
                command = list(command)
                commands.append(command)
                if command[:3] == ["docker", "volume", "create"] and command[-1].endswith("-lightwalletd"):
                    raise subprocess.CalledProcessError(19, command)
                return subprocess.CompletedProcess(command, 0, stdout="")

            with patch("regtest_support.subprocess.run", side_effect=fake_run):
                with self.assertRaises(subprocess.CalledProcessError) as failure:
                    stack.__enter__()

            self.assertEqual(failure.exception.returncode, 19)
            chain = f"{stack.prefix}-chain"
            self.assertEqual(
                [command for command in commands if command[:3] in (["docker", "volume", "rm"], ["docker", "network", "rm"])],
                [
                    ["docker", "volume", "rm", "-f", chain],
                    ["docker", "network", "rm", stack.prefix],
                ],
            )
            cleanup_commands = list(commands)
            stack.close()
            self.assertEqual(commands, cleanup_commands)


if __name__ == "__main__":
    unittest.main()
