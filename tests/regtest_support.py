"""Standard-library support for isolated Zcash regtest integration tests."""

import json
import os
import re
import signal
import socket
import sys
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
        self._cleanup_errors_reported = False
        self._cleanup_actions: list[tuple[str, Callable[[], None]]] = []
        self._cleanup_lock = threading.Lock()
        self._closed = False
        self._process: subprocess.Popen[bytes] | None = None
        self._network_name: str | None = None
        self._chain_volume: str | None = None
        self._lightwalletd_volume: str | None = None
        self._node_container: str | None = None
        self._lightwalletd_container: str | None = None
        self._api_port: int | None = None
        self._serve_stderr_path: Path | None = None
        self._temporary_directory: tempfile.TemporaryDirectory[str] | None = None
        self._previous_signals: dict[int, Any] = {}

    def __enter__(self):
        try:
            self._install_signal_handlers()
            self._setup()
            return self
        except BaseException as error:
            self.close()
            self._attach_cleanup_errors(error)
            raise

    def __exit__(self, exc_type, exc, traceback):
        self.close()
        if self.cleanup_errors:
            if exc is not None:
                self._attach_cleanup_errors(exc)
            else:
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
        self._network_name = self.prefix
        self._register_cleanup("network", lambda: self._remove_docker("network", self.prefix))
        self._docker(["volume", "create", chain_volume])
        self._chain_volume = chain_volume
        self._register_cleanup(
            "chain volume", lambda: self._remove_docker("volume", chain_volume)
        )
        self._docker(["volume", "create", lightwalletd_volume])
        self._lightwalletd_volume = lightwalletd_volume
        self._register_cleanup(
            "lightwalletd volume",
            lambda: self._remove_docker("volume", lightwalletd_volume),
        )

        self._node_container = node_container
        self._register_cleanup(
            "node container", lambda: self._remove_docker("container", node_container)
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
        node_port = self._published_port(node_container, "18232/tcp")
        self.node_url = f"http://127.0.0.1:{node_port}"
        wait_until(
            self._node_ready(node_container), timeout=120, label="Zakura RPC readiness"
        )

        self._lightwalletd_container = lightwalletd_container
        self._register_cleanup(
            "lightwalletd container",
            lambda: self._remove_docker("container", lightwalletd_container),
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
        lightwalletd_port = self._published_port(lightwalletd_container, "9067/tcp")

        self.proxy = GenerateFaultProxy(self.node_url).__enter__()
        self._register_cleanup("generate proxy", self.proxy.close)
        api_port = self._select_loopback_port()
        self._api_port = api_port
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
        self._serve_stderr_path = root / "serve.stderr"
        stderr_fd = os.open(
            self._serve_stderr_path,
            os.O_WRONLY | os.O_CREAT | os.O_EXCL,
            0o600,
        )
        with os.fdopen(stderr_fd, "wb") as serve_stderr:
            self._process = subprocess.Popen(
                [str(self.server), "serve", "--data-dir", str(self.data_dir)],
                env=environment,
                stdout=subprocess.DEVNULL,
                stderr=serve_stderr,
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
            f'"{container_port}") 0).HostPort}}}}'
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
        self.assert_owned_containers_running()
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

    def assert_owned_containers_running(self):
        """Fail immediately when an owned runtime container exits."""
        for container in (self._node_container, self._lightwalletd_container):
            if container is None:
                raise RuntimeError("fixture container was not started")
            self._container_running(container)

    def _server_running(self):
        if self._process is None:
            raise RuntimeError("server process was not started")
        exit_code = self._process.poll()
        if exit_code is not None:
            if self._serve_lost_api_port():
                raise RuntimeError(
                    f"server could not bind fixture API port {self._api_port} "
                    "because it is already in use"
                )
            raise RuntimeError(f"server exited during fixture setup with status {exit_code}")

    def _select_loopback_port(self) -> int:
        """Select a fresh port and diagnose the unavoidable child-bind race safely."""
        with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as listener:
            listener.bind(("127.0.0.1", 0))
            return listener.getsockname()[1]

    def _serve_lost_api_port(self) -> bool:
        if self._api_port is None or self._serve_stderr_path is None:
            return False
        try:
            with self._serve_stderr_path.open("rb") as serve_stderr:
                diagnostics = serve_stderr.read(64 * 1024).lower()
        except OSError:
            return False
        return any(
            marker in diagnostics
            for marker in (b"address already in use", b"eaddrinuse", b"os error 98")
        )

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

    def _attach_cleanup_errors(self, original: BaseException):
        if self._cleanup_errors_reported:
            return
        for cleanup_error in self.cleanup_errors:
            original.add_note(f"regtest cleanup failed: {cleanup_error}")
        self._cleanup_errors_reported = True


class RecoveryFailureReporter:
    """Emit bounded, test-only recovery diagnostics without failure payloads."""

    _PHASES = frozenset(
        {"setup", "broadcast", "auto-mine", "direct-mine", "recovery", "cleanup"}
    )
    _HEIGHTS = frozenset(
        {"before_auto_mine", "after_auto_mine", "inclusion", "tip", "scanned"}
    )
    _STATUSES = frozenset({"broadcast", "confirmed"})
    _TXID = re.compile(r"[0-9a-fA-F]{64}\Z")
    _NOT_REACHED = "not-reached"
    _UNAVAILABLE = "unavailable"

    def __init__(self, stack: RegtestStack | None = None, *, stream: Any = None):
        self._stack = stack
        self._stream = sys.stderr if stream is None else stream
        self._phase = "setup"
        self._heights: dict[str, int | str] = {
            name: self._NOT_REACHED for name in self._HEIGHTS
        }
        self._txid: str = self._NOT_REACHED
        self._activity_id: int | str = self._NOT_REACHED
        self._activity_status: str = self._NOT_REACHED

    def attach_stack(self, stack: RegtestStack):
        self._stack = stack

    def phase(self, phase: str):
        self._phase = phase if phase in self._PHASES else self._UNAVAILABLE

    def record_height(self, name: str, height: Any):
        if name not in self._HEIGHTS:
            return
        self._heights[name] = (
            height
            if isinstance(height, int) and not isinstance(height, bool) and height >= 0
            else self._UNAVAILABLE
        )

    def record_activity(self, activity: Any):
        if not isinstance(activity, dict):
            self._activity_id = self._UNAVAILABLE
            self._activity_status = self._UNAVAILABLE
            self._txid = self._UNAVAILABLE
            return
        activity_id = activity.get("id")
        self._activity_id = (
            activity_id
            if isinstance(activity_id, int)
            and not isinstance(activity_id, bool)
            and activity_id >= 0
            else self._UNAVAILABLE
        )
        status = activity.get("status")
        self._activity_status = status if status in self._STATUSES else self._UNAVAILABLE
        txid = activity.get("txid")
        self._txid = txid if isinstance(txid, str) and self._TXID.fullmatch(txid) else self._UNAVAILABLE

    def emit_failure(self, error: BaseException):
        """Print one sanitized record while leaving the original failure untouched."""
        summary = {
            "activity": {"id": self._activity_id, "status": self._activity_status},
            "cleanup": self._cleanup_state(),
            "exit_codes": {
                "server": self._server_exit_code(),
                "trigger": self._trigger_exit_code(error),
            },
            "failure_route": self._failure_route(error),
            "heights": self._heights,
            "owned_resources": self._owned_resources(),
            "phase": self._phase,
            "proxy_counts": self._proxy_counts(),
            "txid": self._txid,
        }
        print(
            "ACTIVITY_RECOVERY_FAILURE_SUMMARY "
            + json.dumps(summary, sort_keys=True, separators=(",", ":")),
            file=self._stream,
            flush=True,
        )

    def _cleanup_state(self) -> str:
        stack = self._stack
        if stack is None:
            return self._NOT_REACHED
        if stack.cleanup_errors:
            return "failed"
        return "complete" if stack._closed else self._NOT_REACHED

    def _failure_route(self, error: BaseException) -> str:
        if isinstance(error, (KeyboardInterrupt, SystemExit)):
            return "signal"
        if isinstance(error, TimeoutError):
            return "timeout"
        if self._phase == "cleanup":
            return "cleanup"
        if isinstance(error, AssertionError):
            return "assertion"
        if self._phase == "setup":
            return "setup"
        return "error"

    def _trigger_exit_code(self, error: BaseException) -> int | str:
        if isinstance(error, subprocess.CalledProcessError):
            returncode = error.returncode
            if isinstance(returncode, int) and not isinstance(returncode, bool):
                return returncode
        return self._UNAVAILABLE

    def _server_exit_code(self) -> int | str:
        stack = self._stack
        if stack is None:
            return self._NOT_REACHED
        process = stack._process
        if process is None:
            return self._NOT_REACHED
        try:
            returncode = process.poll()
        except (OSError, ValueError):
            return self._UNAVAILABLE
        if returncode is None:
            return "running"
        if isinstance(returncode, int) and not isinstance(returncode, bool):
            return returncode
        return self._UNAVAILABLE

    def _safe_prefix(self) -> str | None:
        stack = self._stack
        if stack is None:
            return None
        prefix = stack.prefix
        if not isinstance(prefix, str) or not prefix.startswith("tsz-recovery-"):
            return None
        try:
            parsed = uuid.UUID(prefix.removeprefix("tsz-recovery-"))
        except ValueError:
            return None
        return prefix if prefix == f"tsz-recovery-{parsed}" else None

    def _safe_resource(self, name: Any, suffix: str) -> str:
        prefix = self._safe_prefix()
        expected = None if prefix is None else f"{prefix}{suffix}"
        return expected if expected is not None and name == expected else self._NOT_REACHED

    def _owned_resources(self) -> dict[str, str]:
        stack = self._stack
        if stack is None:
            return {
                "chain_volume": self._NOT_REACHED,
                "lightwalletd_container": self._NOT_REACHED,
                "lightwalletd_volume": self._NOT_REACHED,
                "network": self._NOT_REACHED,
                "node_container": self._NOT_REACHED,
            }
        return {
            "chain_volume": self._safe_resource(stack._chain_volume, "-chain"),
            "lightwalletd_container": self._safe_resource(
                stack._lightwalletd_container, "-lightwalletd"
            ),
            "lightwalletd_volume": self._safe_resource(
                stack._lightwalletd_volume, "-lightwalletd"
            ),
            "network": self._safe_resource(stack._network_name, ""),
            "node_container": self._safe_resource(stack._node_container, "-zakura"),
        }

    def _proxy_counts(self) -> dict[str, int | str]:
        stack = self._stack
        if stack is None or stack.proxy is None:
            return {
                "forwarded_generates": self._NOT_REACHED,
                "rejected_generates": self._NOT_REACHED,
            }
        try:
            rejected, forwarded = stack.proxy.counts()
        except (AttributeError, OSError, RuntimeError, TypeError, ValueError):
            return {
                "forwarded_generates": self._UNAVAILABLE,
                "rejected_generates": self._UNAVAILABLE,
            }
        if any(
            not isinstance(count, int) or isinstance(count, bool) or count < 0
            for count in (rejected, forwarded)
        ):
            return {
                "forwarded_generates": self._UNAVAILABLE,
                "rejected_generates": self._UNAVAILABLE,
            }
        return {"forwarded_generates": forwarded, "rejected_generates": rejected}
