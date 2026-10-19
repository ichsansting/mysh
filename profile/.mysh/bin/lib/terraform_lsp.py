import json
import os
from pathlib import Path
import subprocess
import sys
import threading
from urllib.parse import unquote, urlsplit


def read_message(stream):
    headers = bytearray()
    length = None
    while True:
        line = stream.readline()
        if not line and not headers:
            return None
        if not line:
            raise ValueError("incomplete LSP headers")
        headers.extend(line)
        if line == b"\r\n":
            break
        if line.lower().startswith(b"content-length:"):
            length = int(line.split(b":", 1)[1])
    if length is None or length < 0:
        raise ValueError("invalid LSP content length")
    body = stream.read(length)
    if len(body) != length:
        raise ValueError("incomplete LSP message")
    return bytes(headers) + body, json.loads(body)


def manifest_notification(message):
    if message.get("method") != "textDocument/didOpen":
        return None
    document_uri = urlsplit(message["params"]["textDocument"]["uri"])
    if document_uri.scheme != "file" or document_uri.netloc not in ("", "localhost"):
        return None
    manifest = Path(unquote(document_uri.path)).parent / ".terraform/modules/modules.json"
    if not manifest.is_file():
        return None
    notification = {"jsonrpc": "2.0", "method": "workspace/didChangeWatchedFiles",
                    "params": {"changes": [{"uri": manifest.as_uri(), "type": 2}]}}
    body = json.dumps(notification).encode()
    return f"Content-Length: {len(body)}\r\n\r\n".encode() + body


def exit_with_server(server):
    status = server.wait()
    os._exit(status if status >= 0 else 128 - status)


def main():
    server = subprocess.Popen(sys.argv[1:], stdin=subprocess.PIPE)
    threading.Thread(target=exit_with_server, args=(server,), daemon=True).start()
    try:
        while packet := read_message(sys.stdin.buffer):
            framed, message = packet
            notification = manifest_notification(message)
            if notification is not None:
                # shortcut: announce manifests before didOpen; remove when Terraform LS fixes startup discovery.
                server.stdin.write(notification)
            server.stdin.write(framed)
            server.stdin.flush()
    except (OSError, ValueError) as error:
        print(f"Terraform LSP bridge: {error}", file=sys.stderr)
        server.terminate()
    finally:
        server.stdin.close()
    try:
        return server.wait(timeout=5)
    except subprocess.TimeoutExpired:
        server.kill()
        return server.wait()


if __name__ == "__main__":
    sys.exit(main())
