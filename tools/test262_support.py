"""Shared process and async-harness support for RuJa's test262 tools."""

import os
import shutil
import subprocess
import tempfile
from pathlib import Path


ASYNC_COMPLETE = "Test262:AsyncTestComplete"
ASYNC_FAILURE = "Test262:AsyncTestFailure:"
ASYNC_PRINT_SHIM = "function print(message) { console.log(message); }"


def append_async_harness(parts, harness, flags):
    """Append test262's async host harness after any directive prologue."""
    if "async" not in flags:
        return
    parts.append(ASYNC_PRINT_SHIM)
    done = harness / "doneprintHandle.js"
    if done.exists():
        parts.append(done.read_text())


def combined_output(stdout, stderr):
    return "\n".join(part.strip() for part in (stderr, stdout) if part.strip())


def classify_result(meta, returncode, stdout, stderr):
    """Return a test262 status and diagnostic for one RuJa process result."""
    out = combined_output(stdout, stderr)
    flags = meta.get("flags", [])

    if "async" in flags:
        lines = [line.strip() for line in out.splitlines() if line.strip()]
        if any(line.startswith(ASYNC_FAILURE) for line in lines):
            return "fail", out
        if returncode != 0:
            return "fail", out or f"RuJa exited with status {returncode}"
        complete_count = sum(line == ASYNC_COMPLETE for line in lines)
        if complete_count == 0:
            return "fail", out or "Test262 async completion marker missing"
        if complete_count > 1:
            return "fail", "Test262 async completion marker repeated"
        unexpected = [line for line in lines if line != ASYNC_COMPLETE]
        if unexpected:
            return "fail", out
        return "pass", ""

    negative = meta.get("negative")
    if negative:
        expected = negative.get("type", "")
        if expected and expected in out:
            return "pass", ""
        return "fail", out
    if returncode == 0 and not out:
        return "pass", ""
    return "fail", out


def execute_source(source, meta, ruja, timeout=8, source_path=None):
    """Execute one assembled test262 source and classify its result."""
    try:
        staging = None
        if source_path is not None:
            original = Path(source_path).resolve()
            staging = tempfile.TemporaryDirectory(prefix="ruja-test262-module-")
            staging_dir = Path(staging.name)
            for sibling in original.parent.iterdir():
                destination = staging_dir / sibling.name
                if sibling.is_dir():
                    shutil.copytree(sibling, destination)
                else:
                    shutil.copy2(sibling, destination)
            path = staging_dir / original.name
            if path.exists() or path.is_symlink():
                path.unlink()
            path.write_text(source)
        else:
            with tempfile.NamedTemporaryFile(
                "w", suffix=".js", prefix=".ruja-test262-", delete=False
            ) as test_file:
                test_file.write(source)
                path = Path(test_file.name)
        try:
            process_env = os.environ.copy()
            flags = meta.get("flags", [])
            if "CanBlockIsTrue" in flags:
                process_env["RUJA_AGENT_CAN_BLOCK"] = "1"
            elif "CanBlockIsFalse" in flags:
                process_env["RUJA_AGENT_CAN_BLOCK"] = "0"
            command = [ruja]
            negative_phase = (meta.get("negative") or {}).get("phase")
            if negative_phase == "parse":
                command.append("--module-parse" if "module" in flags else "--parse")
            elif negative_phase == "resolution":
                command.append("--module-link")
            elif "module" in flags:
                command.append("--module")
            command.append(str(path))
            result = subprocess.run(
                command,
                capture_output=True,
                text=True,
                timeout=timeout,
                env=process_env,
            )
        finally:
            if staging is not None:
                staging.cleanup()
            else:
                os.unlink(path)
        return classify_result(
            meta, result.returncode, result.stdout, result.stderr
        )
    except subprocess.TimeoutExpired:
        return "timeout", ""
    except Exception as error:
        return "error", str(error)
