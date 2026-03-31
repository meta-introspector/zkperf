from __future__ import annotations

import os
import subprocess
from datetime import datetime, timezone
from hashlib import sha256
from typing import Any, Callable
from urllib.parse import urlparse

import requests


def _utc_now() -> str:
    return datetime.now(timezone.utc).replace(microsecond=0).isoformat().replace("+00:00", "Z")


def parse_ipfs_uri(uri: str) -> dict[str, str]:
    if not uri.startswith("ipfs://"):
        raise ValueError(f"unsupported IPFS URI: {uri}")
    parsed = urlparse(uri)
    cid = parsed.netloc or parsed.path.lstrip("/").split("/", 1)[0]
    path = ""
    if parsed.netloc and parsed.path:
        path = parsed.path.lstrip("/")
    elif parsed.path.lstrip("/") and "/" in parsed.path.lstrip("/"):
        path = parsed.path.lstrip("/").split("/", 1)[1]
    return {"cid": cid, "path": path}


def _gateway_url(*, cid: str, path: str = "", base_url: str | None = None) -> str:
    gateway = base_url.rstrip("/") if base_url else "https://ipfs.io"
    suffix = f"/{path}" if path else ""
    return f"{gateway}/ipfs/{cid}{suffix}"


def fetch_ipfs_object(
    *,
    ipfs_uri: str,
    base_url: str | None = None,
    get: Callable[..., Any] | None = None,
    timeout: float = 20.0,
) -> dict[str, Any]:
    parsed = parse_ipfs_uri(ipfs_uri)
    url = _gateway_url(cid=parsed["cid"], path=parsed["path"], base_url=base_url)
    caller = get or requests.get
    response = caller(url, allow_redirects=True, timeout=timeout)
    if hasattr(response, "raise_for_status"):
        response.raise_for_status()
    content = response.content if hasattr(response, "content") else bytes(str(response), "utf-8")
    headers = getattr(response, "headers", {})
    binary_like = b"\x00" in content
    return {
        "cid": parsed["cid"],
        "path": parsed["path"],
        "gatewayUrl": url,
        "statusCode": getattr(response, "status_code", 200),
        "finalUrl": getattr(response, "url", url),
        "etag": headers.get("etag"),
        "contentLength": headers.get("content-length"),
        "contentType": headers.get("content-type"),
        "sha256": sha256(content).hexdigest(),
        "sizeBytes": len(content),
        "text": None if binary_like else content.decode("utf-8", "replace"),
        "textPreview": None if not binary_like else content[:64].hex(),
        "fetchedAt": _utc_now(),
    }


def download_ipfs_object_bytes(
    *,
    ipfs_uri: str,
    base_url: str | None = None,
    get: Callable[..., Any] | None = None,
    timeout: float = 20.0,
) -> dict[str, Any]:
    parsed = parse_ipfs_uri(ipfs_uri)
    url = _gateway_url(cid=parsed["cid"], path=parsed["path"], base_url=base_url)
    caller = get or requests.get
    response = caller(url, allow_redirects=True, timeout=timeout)
    if hasattr(response, "raise_for_status"):
        response.raise_for_status()
    content = response.content if hasattr(response, "content") else bytes(str(response), "utf-8")
    return {
        "bytes": content,
        "metadata": fetch_ipfs_object(ipfs_uri=ipfs_uri, base_url=base_url, get=lambda *args, **kwargs: response),
    }


def publish_ipfs_file_with_ack(
    *,
    local_path: str,
    api_base_url: str = "http://127.0.0.1:5001",
    run: Callable[..., Any] | None = None,
    post: Callable[..., Any] | None = None,
    pin: bool = True,
    timeout: float = 30.0,
) -> dict[str, Any]:
    local_bytes = open(local_path, "rb").read()
    local_sha256 = sha256(local_bytes).hexdigest()
    api_post = post or requests.post
    use_api = post is not None
    if not use_api:
        try:
            version_response = api_post(f"{api_base_url.rstrip('/')}/api/v0/version", timeout=timeout)
            if hasattr(version_response, "raise_for_status"):
                version_response.raise_for_status()
            use_api = True
        except Exception:
            use_api = False
    if use_api:
        with open(local_path, "rb") as handle:
            response = api_post(
                f"{api_base_url.rstrip('/')}/api/v0/add",
                files={"file": (os.path.basename(local_path), handle)},
                timeout=timeout,
            )
        if hasattr(response, "raise_for_status"):
            response.raise_for_status()
        text = response.text if hasattr(response, "text") else str(response)
    else:
        cli = run or subprocess.run
        completed = cli(["ipfs", "add", "-Q", local_path], check=True, capture_output=True, text=True)
        text = completed.stdout
    cid = text.strip().splitlines()[-1]
    ipfs_uri = f"ipfs://{cid}"
    fetched = fetch_ipfs_object(ipfs_uri=ipfs_uri, base_url="https://ipfs.io")
    return {
        "cid": cid,
        "ipfsUri": ipfs_uri,
        "localSha256": local_sha256,
        "localSizeBytes": len(local_bytes),
        "verified": fetched.get("sha256") == local_sha256,
        "fetch": fetched,
        "pinRequested": pin,
    }


__all__ = [
    "download_ipfs_object_bytes",
    "fetch_ipfs_object",
    "parse_ipfs_uri",
    "publish_ipfs_file_with_ack",
]
