#!/usr/bin/env python3
"""Exercise the macOS GUI's mailboxes through two real PTYs and its bundled CLI."""

import hashlib
import json
import os
from pathlib import Path
import shutil
import socket
import subprocess
import sys
import tempfile
import time
import traceback
import uuid


def write(path, value):
    temporary = path.with_suffix(".tmp")
    temporary.write_text(json.dumps(value, ensure_ascii=False, indent=2))
    temporary.replace(path)


def until(read, description, seconds=40):
    end = time.monotonic() + seconds
    while time.monotonic() < end:
        value = read()
        if value:
            return value
        time.sleep(0.15)
    raise AssertionError(f"Timed out: {description}")


def peer(root):
    role = Path.cwd().name
    cli = os.environ["ELY_CLI"]
    assert shutil.which("ely-tea-party") == cli

    def msg(*args, failure=None):
        result = subprocess.run(
            [cli, "msg", *args], capture_output=True, text=True, timeout=12
        )
        if failure:
            assert result.returncode != 0 and failure in result.stderr, result.stderr
            return
        assert result.returncode == 0, result.stderr
        return json.loads(result.stdout)

    def hook(conversation):
        subprocess.run(
            [os.environ["ELY_HOOK"], "hook", "claude" if role == "sender" else "codex",
             "SessionStart", json.dumps({"session_id": conversation})],
            check=True, timeout=12,
        )

    def ready():
        who = msg("whoami")
        return who if ("Claude" if role == "sender" else "Codex") in who["agent"] else None

    def read(name):
        path = root / name
        return json.loads(path.read_text()) if path.exists() else None

    hook(f"acceptance-{role}")
    me = until(ready, "hook metadata")
    me["endpoint"] = os.environ["ELY_MSG_ENDPOINT"]
    assert me["folder"] == str(Path.cwd()), me
    assert me["title"] == f"Acceptance {role}", me
    guide = msg("guide")
    assert guide["protocol_version"] == 1 and guide["instructions"]
    write(root / f"{role}-ready.json", me)
    if role == "sender":
        other = until(lambda: read("receiver-ready.json"), "receiver registration")
        found = msg("list", "--agent", "codex", "--folder", "receiver",
                    "--query", "Acceptance")
        assert len(found) == 1 and found[0]["address"] == other["address"], found
        key = str(uuid.uuid4())
        sent = msg("send", other["address"], "登录接口已更新", "--key", key)
        retried = msg("send", other["address"], "登录接口已更新", "--key", key)
        assert sent == retried and sent["state"] == "stored"
        msg("send", other["address"], "different", "--key", key, failure="different content")
        msg("ack", sent["message"]["id"], failure="only the recipient")
        request = {"protocol": 1, "session": int(os.environ["ELY_SESSION"]),
                   "token": "wrong-token", "command": {"method": "whoami"}}
        for kind in ("credentials", "protocol mismatch"):
            if kind == "protocol mismatch":
                request.update(protocol=999, token=os.environ["ELY_MSG_TOKEN"])
            with socket.socket(socket.AF_UNIX) as stream:
                stream.connect(os.environ["ELY_MSG_ENDPOINT"].removeprefix("unix:"))
                stream.sendall(json.dumps(request).encode() + b"\n")
                response = json.loads(stream.makefile("rb").readline())
            assert kind in response["error"], response
        write(root / "sent.json", sent)
        reply = until(lambda: msg("inbox"), "reply in sender inbox")
        assert len(reply) == 1 and reply[0]["body"] == "收到，类型已同步"
        assert reply[0]["reply_to"] == sent["message"]["id"]
        msg("ack", reply[0]["id"])
        assert msg("inbox") == []
        assert msg("inbox", "--all")[0]["acknowledged"]
        write(root / "replace.json", True)
        replaced = until(lambda: read("replaced.json"), "new recipient incarnation")
        assert replaced["address"] != other["address"]
        msg("send", other["address"], "stale destination", failure="target is closed")
        after = msg("send", other["address"], "登录接口已更新", "--key", key)
        assert after["message"]["id"] == sent["message"]["id"]
        assert after["message"]["acknowledged"]
        write(root / "sender-result.json", {"ok": True, "checks": [
            "PATH inheritance", "live hook metadata", "filtered discovery", "send",
            "retry deduplication", "request-key conflict", "recipient-only ack",
            "credential rejection", "protocol rejection", "reply", "explicit ack",
            "retired address rejection", "retry after recipient replacement"]})
    else:
        sent = until(lambda: read("sent.json"), "sender receipt")
        inbox = msg("inbox")
        assert len(inbox) == 1 and inbox[0]["id"] == sent["message"]["id"]
        assert not inbox[0]["acknowledged"] and inbox[0]["body"] == "登录接口已更新"
        assert msg("whoami")["unread"] == 1
        msg("reply", inbox[0]["id"], "收到，类型已同步")
        msg("ack", inbox[0]["id"])
        assert msg("inbox") == [] and msg("inbox", "--all")[0]["acknowledged"]
        until(lambda: read("replace.json"), "replacement requested")
        hook("acceptance-replacement")
        replaced = until(lambda: (value if (value := msg("whoami"))["address"] !=
                                  me["address"] else None), "replacement hook")
        assert msg("inbox", "--all") == []
        write(root / "replaced.json", replaced)
        write(root / "receiver-result.json", {"ok": True, "checks": [
            "single delivery", "unread count", "read without ack", "reply linkage",
            "explicit ack", "replacement cannot inherit old inbox"]})
    print(f"Acceptance {role}: PASS", flush=True)
    until(lambda: (root / "finish").exists(), "test shutdown", seconds=120)


def main():
    if sys.platform != "darwin":
        raise SystemExit("This GUI acceptance driver currently requires macOS.")
    repo = Path(__file__).resolve().parents[1]
    binary = repo / "target/debug/ely-tea-party"
    root = Path(tempfile.mkdtemp(prefix="ely-messaging-acceptance-")).resolve()
    home, config = root / "home", root / "config"
    home.mkdir()
    config.mkdir()
    for role in ("sender", "receiver"):
        (root / role).mkdir()
    env = {"HOME": str(home), "PATH": "/usr/bin:/bin:/usr/sbin:/sbin",
           "ELY_TEST_CONFIG_DIR": str(config), "RUST_LOG": "ely_tea_party=info",
           "LANG": "en_US.UTF-8"}
    for key in ("TMPDIR", "USER", "LOGNAME"):
        if key in os.environ:
            env[key] = os.environ[key]
    for agent in ("claude", "codex"):
        subprocess.run([binary, "skills", "install", agent], env=env,
                       check=True, capture_output=True)
    claude = home / ".claude/skills/ely-session-messaging"
    original = (claude / "SKILL.md").read_bytes()
    old = b"old managed bootstrap\n"
    (claude / "SKILL.md").write_bytes(old)
    manifest = json.loads((claude / ".ely-managed.json").read_text())
    manifest.update(version=0, hash=hashlib.sha256(old).hexdigest())
    write(claude / ".ely-managed.json", manifest)
    codex = home / ".agents/skills/ely-session-messaging/SKILL.md"
    codex.write_text("user customization\n")
    write(config / "settings.json", {"shell": "/bin/zsh", "restore_sessions": True,
                                      "sidebar_style": "compact"})
    write(config / "sessions-dev.json", {"active": 0, "sessions": [
        {"folder": str(root / role), "resume": None, "name": f"Acceptance {role}"}
        for role in ("sender", "receiver")]})
    import shlex
    command = shlex.join([sys.executable, str(Path(__file__).resolve()), "--peer", str(root)])
    (home / ".zshrc").write_text(f"{command}\nexit\n")
    print(f"Evidence: {root}", flush=True)
    with (root / "app.log").open("w") as log:
        app = subprocess.Popen([binary], env=env, stdout=log, stderr=log)
        try:
            def completed():
                failures = list(root.glob("*-error.json"))
                assert not failures, [p.read_text() for p in failures]
                assert app.poll() is None, f"App exited: read {root / 'app.log'}"
                return all((root / f"{role}-result.json").exists()
                           for role in ("sender", "receiver"))
            until(completed, "two GUI sessions complete", seconds=60)
            assert (claude / "SKILL.md").read_bytes() == original
            assert codex.read_text() == "user customization\n"
            results = {role: json.loads((root / f"{role}-result.json").read_text())
                       for role in ("sender", "receiver")}
            results["skills"] = {"startup_upgrade": True, "user_edit_preserved": True}
        finally:
            (root / "finish").touch()
            try:
                app.wait(timeout=8)
            except subprocess.TimeoutExpired:
                app.terminate()
                app.wait(timeout=8)
        endpoint = json.loads((root / "sender-ready.json").read_text())["endpoint"]
        assert not Path(endpoint.removeprefix("unix:")).parent.exists(), "socket directory leaked"
        results["transport"] = {"clean_shutdown": True}
        write(root / "result.json", results)
        print(json.dumps(results, ensure_ascii=False, indent=2), flush=True)


if __name__ == "__main__":
    if sys.argv[1:2] == ["--peer"]:
        root = Path(sys.argv[2])
        try:
            peer(root)
        except Exception as error:
            write(root / f"{Path.cwd().name}-error.json", {"error": traceback.format_exc()})
            raise
    else:
        main()
