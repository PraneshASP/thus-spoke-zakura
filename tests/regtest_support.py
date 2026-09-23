"""Standard-library support for isolated Zcash regtest integration tests."""

import json
import os
import signal
import socket
import subprocess
import tempfile
import threading
import time
import uuid
from collections.abc import Callable
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from typing import Any
from urllib.error import HTTPError, URLError
from urllib.request import Request, urlopen


def wait_until(check: Callable[[], Any], *, timeout: float, label: str) -> Any:
    """Return the first truthy check result before a monotonic deadline."""
    deadline = time.monotonic() + timeout
    while True:
        result = check()
        if result:
            return result
        remaining = deadline - time.monotonic()
        if remaining <= 0:
            raise TimeoutError(f"timed out waiting for {label}")
        time.sleep(min(0.25, remaining))


def request_json(
    base: str,
    path: str,
    body: dict[str, Any] | None = None,
    *,
    timeout: float = 5,
) -> Any:
    """Request JSON, using GET without a body and POST with a JSON body."""
    url = f"{base.rstrip('/')}/{path.lstrip('/')}"
    if body is None:
        request = Request(url, method="GET")
    else:
        request = Request(
            url,
            data=json.dumps(body).encode(),
            headers={"Content-Type": "application/json"},
            method="POST",
        )
    try:
        with urlopen(request, timeout=timeout) as response:
            if not 200 <= response.status < 300:
                raise RuntimeError(f"HTTP {response.status}")
            return json.loads(response.read())
    except HTTPError as error:
        try:
            raise RuntimeError(f"HTTP {error.code}") from None
        finally:
            error.close()
    except (OSError, TimeoutError, URLError):
        raise RuntimeError("HTTP transport failure") from None


def rpc(endpoint: str, method: str, params: list[Any]) -> Any:
    """Make one strict JSON-RPC request without retrying mutating methods."""
    request_id = uuid.uuid4().hex
    response = request_json(
        endpoint,
        "",
        {"jsonrpc": "2.0", "id": request_id, "method": method, "params": params},
        timeout=30,
    )
    if not isinstance(response, dict):
        raise RuntimeError("JSON-RPC response is not an object")
    if response.get("id") != request_id:
        raise RuntimeError("JSON-RPC response ID mismatch")
    if response.get("error") is not None:
        raise RuntimeError("JSON-RPC response contains an error")
    if "result" not in response:
        raise RuntimeError("JSON-RPC response is missing result")
    return response["result"]


class GenerateFaultProxy:
    """Forward JSON-RPC while reserving a one-shot generate fault for tests."""

    def __init__(self, upstream_url: str):
        self._upstream_url = upstream_url
        self._httpd = None
        self._thread = None
        self._cleanup_lock = threading.Lock()
        self._state_lock = threading.Lock()
        self._armed = False
        self._rejected_generates = 0
        self._forwarded_generates = 0
        self.url = ""

    def __enter__(self):
        owner = self

        class Handler(BaseHTTPRequestHandler):
            def write_response(self, status, body):
                self.send_response(status)
                self.send_header("Content-Type", "application/json")
                self.send_header("Content-Length", str(len(body)))
                self.end_headers()
                self.wfile.write(body)

            def record_forwarded_generate(self, is_generate):
                if is_generate:
                    with owner._state_lock:
                        owner._forwarded_generates += 1

            def do_POST(self):  # noqa: N802 - HTTP handler API
                size = int(self.headers["Content-Length"])
                body = self.rfile.read(size)
                rpc_request = json.loads(body)
                is_generate = rpc_request.get("method") == "generate"
                is_fault_target = is_generate and rpc_request.get("params") == [1]
                with owner._state_lock:
                    reject = is_fault_target and owner._armed
                    if reject:
                        owner._armed = False
                        owner._rejected_generates += 1
                if reject:
                    response_body = json.dumps(
                        {
                            "jsonrpc": "2.0",
                            "id": rpc_request.get("id"),
                            "result": None,
                            "error": {
                                "code": -32603,
                                "message": "injected generate failure",
                            },
                        }
                    ).encode()
                    self.write_response(500, response_body)
                    return
                upstream_request = Request(
                    owner._upstream_url,
                    data=body,
                    headers={"Content-Type": "application/json"},
                    method="POST",
                )
                try:
                    with urlopen(upstream_request, timeout=120 if is_generate else 5) as upstream_response:
                        response_body = upstream_response.read()
                        self.record_forwarded_generate(is_generate)
                        self.write_response(upstream_response.status, response_body)
                except HTTPError as error:
                    try:
                        response_body = error.read()
                    finally:
                        error.close()
                    self.record_forwarded_generate(is_generate)
                    self.write_response(error.code, response_body)
                except (OSError, TimeoutError, URLError):
                    response_body = json.dumps(
                        {
                            "jsonrpc": "2.0",
                            "id": rpc_request.get("id"),
                            "result": None,
                            "error": {
                                "code": -32000,
                                "message": "upstream transport failure",
                            },
                        }
                    ).encode()
                    self.write_response(502, response_body)

            def log_message(self, format, *args):
                return

        self._httpd = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
        self._httpd.daemon_threads = True
        self.url = f"http://127.0.0.1:{self._httpd.server_port}"
        self._thread = threading.Thread(target=self._httpd.serve_forever, daemon=True)
        self._thread.start()
        return self

    def fail_next_generate(self):
        with self._state_lock:
            if self._armed:
                raise RuntimeError("generate fault is already armed")
            self._armed = True

    def counts(self):
        with self._state_lock:
            return self._rejected_generates, self._forwarded_generates

    def close(self):
        with self._cleanup_lock:
            httpd, thread = self._httpd, self._thread
            self._httpd = None
            self._thread = None
        if httpd is not None:
            httpd.shutdown()
            httpd.server_close()
        if thread is not None:
            thread.join(timeout=5)

    def __exit__(self, exc_type, exc, traceback):
        self.close()


class RegtestStack:
    """Own an isolated node, lightwalletd and server process for one regtest."""

    def __init__(self, server: Path):
        self.server = Path(server).resolve()
        self.prefix = f"tsz-recovery-{uuid.uuid4()}"
        self.api_url = ""
        self.node_url = ""
        self.proxy: GenerateFaultProxy | None = None
        self.data_dir: Path | None = None
        self.cleanup_errors: list[str] = []
        self._cleanup_actions: list[tuple[str, Callable[[], None]]] = []
        self._cleanup_lock = threading.Lock()
        self._closed = False
        self._process: subprocess.Popen[bytes] | None = None
        self._temporary_directory: tempfile.TemporaryDirectory[str] | None = None
        self._previous_signals: dict[int, Any] = {}

    def __enter__(self):
        try:
            self._install_signal_handlers()
            self._setup()
            return self
        except BaseException:
            self.close()
            raise

    def __exit__(self, exc_type, exc, traceback):
        self.close()
        if exc_type is None and self.cleanup_errors:
            raise RuntimeError("regtest cleanup failed: " + "; ".join(self.cleanup_errors))
        return False

    def _setup(self):
        if not self.server.is_file() or not os.access(self.server, os.X_OK):
            raise ValueError("TSZ_TEST_SERVER must be an executable source-built tsz-server")
        self._run(["docker", "version", "--format", "{{.Server.Version}}"])

        self._temporary_directory = tempfile.TemporaryDirectory(
            prefix=f"{self.prefix}-", dir=os.environ.get("TMPDIR")
        )
        self._register_cleanup("temporary directory", self._temporary_directory.cleanup)
        root = Path(self._temporary_directory.name)
        self.data_dir = root / "data"
        config_dir = root / "config"
        self.data_dir.mkdir()
        config_dir.mkdir()
        self._run(
            [
                str(self.server),
                "init",
                "--data-dir",
                str(self.data_dir),
                "--config-dir",
                str(config_dir),
            ]
        )

        chain_volume = f"{self.prefix}-chain"
        lightwalletd_volume = f"{self.prefix}-lightwalletd"
        node_container = f"{self.prefix}-zakura"
        lightwalletd_container = f"{self.prefix}-lightwalletd"

        self._docker(["network", "create", self.prefix])
        self._register_cleanup("network", lambda: self._remove_docker("network", self.prefix))
        self._docker(["volume", "create", chain_volume])
        self._register_cleanup(
            "chain volume", lambda: self._remove_docker("volume", chain_volume)
        )
        self._docker(["volume", "create", lightwalletd_volume])
        self._register_cleanup(
            "lightwalletd volume",
            lambda: self._remove_docker("volume", lightwalletd_volume),
        )

        self._docker(
            [
                "run",
                "--detach",
                "--name",
                node_container,
                "--network",
                self.prefix,
                "--network-alias",
                "zakura",
                "--volume",
                f"{chain_volume}:/data",
                "--volume",
                f"{config_dir}:/config:ro",
                "--env",
                "CONFIG_FILE_PATH=/config/zakurad.toml",
                "--publish",
                "127.0.0.1::18232",
                "zakuracore/zakura:1.4.0",
                "zakurad",
                "start",
            ]
        )
        self._register_cleanup(
            "node container", lambda: self._remove_docker("container", node_container)
        )
        node_port = self._published_port(node_container, "18232/tcp")
        self.node_url = f"http://127.0.0.1:{node_port}"
        wait_until(
            self._node_ready(node_container), timeout=120, label="Zakura RPC readiness"
        )

        self._docker(
            [
                "run",
                "--detach",
                "--name",
                lightwalletd_container,
                "--network",
                self.prefix,
                "--user",
                "0:0",
                "--volume",
                f"{lightwalletd_volume}:/var/lib/lightwalletd",
                "--publish",
                "127.0.0.1::9067",
                "tsz-recovery-lightwalletd:local",
                "--no-tls-very-insecure",
                "--grpc-bind-addr",
                "0.0.0.0:9067",
                "--rpchost",
                "zakura",
                "--rpcport",
                "18232",
                "--rpcuser",
                "unused",
                "--rpcpassword",
                "unused",
                "--data-dir",
                "/var/lib/lightwalletd",
                "--log-file",
                "/dev/stdout",
            ]
        )
        self._register_cleanup(
            "lightwalletd container",
            lambda: self._remove_docker("container", lightwalletd_container),
        )
        lightwalletd_port = self._published_port(lightwalletd_container, "9067/tcp")

        self.proxy = GenerateFaultProxy(self.node_url).__enter__()
        self._register_cleanup("generate proxy", self.proxy.close)
        api_port = self._unused_loopback_port()
        self.api_url = f"http://127.0.0.1:{api_port}"
        environment = os.environ.copy()
        environment.update(
            {
                "TSZ_ZAKURA_RPC": self.proxy.url,
                "TSZ_LIGHTWALLETD": f"http://127.0.0.1:{lightwalletd_port}",
                "TSZ_INSTANCE": self.prefix,
                "TSZ_LISTEN": f"127.0.0.1:{api_port}",
            }
        )
        self._process = subprocess.Popen(
            [str(self.server), "serve", "--data-dir", str(self.data_dir)],
            env=environment,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
        )
        self._register_cleanup("server process", self._stop_process)
        wait_until(self._funded_server_ready, timeout=300, label="funded server startup")

    def _register_cleanup(self, label: str, action: Callable[[], None]):
        self._cleanup_actions.append((label, action))

    def _run(
        self, command: list[str], *, timeout: float = 30, capture: bool = False, check: bool = True
    ) -> subprocess.CompletedProcess[str]:
        return subprocess.run(
            command,
            check=check,
            timeout=timeout,
            stdout=subprocess.PIPE if capture else subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
            text=capture,
        )

    def _docker(self, arguments: list[str]):
        return self._run(["docker", *arguments])

    def _docker_output(self, arguments: list[str]) -> str:
        output = self._run(["docker", *arguments], capture=True).stdout.strip()
        if not output:
            raise RuntimeError("Docker returned an empty inspection result")
        return output

    def _published_port(self, container: str, container_port: str) -> int:
        template = (
            "{{(index (index .NetworkSettings.Ports "
            f'"{container_port}") 0).HostPort}}'
        )
        try:
            return int(self._docker_output(["inspect", "--format", template, container]))
        except ValueError:
            raise RuntimeError("Docker returned an invalid published port") from None

    def _container_running(self, container: str):
        if self._docker_output(["inspect", "--format", "{{.State.Running}}", container]) != "true":
            raise RuntimeError(f"container {container} exited during fixture setup")

    def _node_ready(self, container: str) -> Callable[[], Any]:
        def probe():
            self._container_running(container)
            try:
                return rpc(self.node_url, "getblockchaininfo", [])
            except RuntimeError as error:
                if str(error) in {"HTTP transport failure", "HTTP 502", "HTTP 503"}:
                    return False
                raise

        return probe

    def _funded_server_ready(self):
        self._server_running()
        try:
            health = request_json(self.api_url, "/api/v1/health")
            status = request_json(self.api_url, "/api/v1/status")
            accounts = request_json(self.api_url, "/api/v1/accounts")
        except RuntimeError as error:
            if str(error) in {"HTTP transport failure", "HTTP 502", "HTTP 503"}:
                return False
            raise
        if health["instance"] != self.prefix:
            raise RuntimeError("fixture API instance did not match its owned prefix")
        if status["wallet_sync"]["state"] != "ready":
            return False
        account_one = next(account for account in accounts if account["id"] == 1)
        if account_one["orchard_zatoshi"] != 500000000:
            return False
        return health

    def _server_running(self):
        if self._process is None:
            raise RuntimeError("server process was not started")
        exit_code = self._process.poll()
        if exit_code is not None:
            raise RuntimeError(f"server exited during fixture setup with status {exit_code}")

    def _unused_loopback_port(self) -> int:
        with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as listener:
            listener.bind(("127.0.0.1", 0))
            return listener.getsockname()[1]

    def _stop_process(self):
        if self._process is None or self._process.poll() is not None:
            return
        self._process.terminate()
        try:
            self._process.wait(timeout=10)
        except subprocess.TimeoutExpired:
            self._process.kill()
            self._process.wait(timeout=5)

    def _remove_docker(self, kind: str, name: str):
        arguments = ["docker", kind, "rm"]
        if kind in {"container", "volume"}:
            arguments.append("-f")
        arguments.append(name)
        result = self._run(arguments, check=False)
        if result.returncode != 0:
            raise RuntimeError(f"failed to remove owned Docker {kind}")

    def _install_signal_handlers(self):
        if threading.current_thread() is not threading.main_thread():
            return

        def cleanup_then_exit(signum, frame):
            self.close()
            if signum == signal.SIGINT:
                raise KeyboardInterrupt
            raise SystemExit(128 + signum)

        for signum in (signal.SIGINT, signal.SIGTERM):
            self._previous_signals[signum] = signal.getsignal(signum)
            signal.signal(signum, cleanup_then_exit)
        self._register_cleanup("signal handlers", self._restore_signal_handlers)

    def _restore_signal_handlers(self):
        for signum, handler in self._previous_signals.items():
            signal.signal(signum, handler)
        self._previous_signals.clear()

    def close(self):
        with self._cleanup_lock:
            if self._closed:
                return
            self._closed = True
            actions = list(reversed(self._cleanup_actions))
        for label, action in actions:
            try:
                action()
            except Exception as error:
                self.cleanup_errors.append(f"{label}: {error}")
