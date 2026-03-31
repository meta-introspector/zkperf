from __future__ import annotations

from dataclasses import dataclass
from hashlib import sha256
import subprocess
import time
from typing import Any, Callable

import requests


def _is_transient_hf_upload_error(detail: str, returncode: int | None = None) -> bool:
    text = detail.lower()
    transient_markers = (
        "temporary failure in name resolution",
        "name or service not known",
        "failed to resolve",
        "connection reset",
        "connection aborted",
        "connection refused",
        "timed out",
        "timeout",
        "tlsv1 alert internal error",
        "remote disconnected",
        "network is unreachable",
    )
    if any(marker in text for marker in transient_markers):
        return True
    return returncode in {6, 7, 28}


@dataclass(frozen=True)
class HfObjectReference:
    repo_type: str
    repo_id: str
    object_path: str

    @property
    def resolve_url(self) -> str:
        return self.resolve_url_for_revision("main")

    def resolve_url_for_revision(self, revision: str) -> str:
        repo_prefix = "" if self.repo_type == "models" else f"{self.repo_type}/"
        return f"https://huggingface.co/{repo_prefix}{self.repo_id}/resolve/{revision}/{self.object_path}"


def parse_hf_uri(uri: str) -> HfObjectReference:
    if not uri.startswith("hf://"):
        raise ValueError(f"unsupported HF URI: {uri}")
    parts = [part for part in uri[len("hf://") :].split("/") if part]
    if len(parts) < 4:
        raise ValueError(f"HF URI must include type, repo id, and object path: {uri}")
    repo_type = parts[0]
    if repo_type not in {"datasets", "models", "spaces"}:
        raise ValueError(f"unsupported HF repo type: {repo_type}")
    return HfObjectReference(repo_type=repo_type, repo_id=f"{parts[1]}/{parts[2]}", object_path="/".join(parts[3:]))


def fetch_hf_object(
    *,
    hf_uri: str,
    revision: str | None = None,
    get: Callable[..., Any] | None = None,
    timeout: float = 20.0,
) -> dict[str, Any]:
    reference = parse_hf_uri(hf_uri)
    caller = get or requests.get
    url = reference.resolve_url_for_revision(revision) if revision else reference.resolve_url
    response = caller(url, allow_redirects=True, timeout=timeout)
    if hasattr(response, "raise_for_status"):
        response.raise_for_status()
    content = response.content if hasattr(response, "content") else bytes(str(response), "utf-8")
    headers = getattr(response, "headers", {})
    history = list(getattr(response, "history", []) or [])
    history_headers = [getattr(item, "headers", {}) for item in history]
    x_repo_commit = headers.get("x-repo-commit")
    if x_repo_commit is None:
        for item_headers in reversed(history_headers):
            x_repo_commit = item_headers.get("x-repo-commit")
            if x_repo_commit:
                break
    effective_etag = headers.get("etag")
    if effective_etag is None:
        for item_headers in reversed(history_headers):
            effective_etag = item_headers.get("x-linked-etag") or item_headers.get("etag")
            if effective_etag:
                break
    binary_like = b"\x00" in content
    return {
        "repoType": reference.repo_type,
        "repoId": reference.repo_id,
        "objectPath": reference.object_path,
        "resolveUrl": url,
        "finalUrl": getattr(response, "url", url),
        "statusCode": getattr(response, "status_code", 200),
        "revision": revision,
        "etag": effective_etag,
        "xRepoCommit": x_repo_commit,
        "contentLength": headers.get("content-length"),
        "sha256": sha256(content).hexdigest(),
        "sizeBytes": len(content),
        "text": None if binary_like else content.decode("utf-8", "replace"),
        "textPreview": None if not binary_like else content[:64].hex(),
    }


def download_hf_object_bytes(
    *,
    hf_uri: str,
    revision: str | None = None,
    get: Callable[..., Any] | None = None,
    timeout: float = 20.0,
) -> dict[str, Any]:
    reference = parse_hf_uri(hf_uri)
    caller = get or requests.get
    url = reference.resolve_url_for_revision(revision) if revision else reference.resolve_url
    response = caller(url, allow_redirects=True, timeout=timeout)
    if hasattr(response, "raise_for_status"):
        response.raise_for_status()
    content = response.content if hasattr(response, "content") else bytes(str(response), "utf-8")
    return {
        "bytes": content,
        "metadata": fetch_hf_object(hf_uri=hf_uri, revision=revision, get=lambda *args, **kwargs: response),
    }


def upload_hf_file_with_ack(
    *,
    local_path: str,
    hf_uri: str,
    commit_message: str | None = None,
    run: Callable[..., Any] | None = None,
    fetch: Callable[..., dict[str, Any]] | None = None,
    max_attempts: int = 2,
    retry_delay_seconds: float = 1.0,
) -> dict[str, Any]:
    reference = parse_hf_uri(hf_uri)
    cli = run or subprocess.run
    local_bytes = open(local_path, "rb").read()
    local_sha256 = sha256(local_bytes).hexdigest()
    repo_type_flag = {"datasets": "dataset", "models": "model", "spaces": "space"}[reference.repo_type]
    command = ["hf", "upload", reference.repo_id, local_path, reference.object_path, "--repo-type", repo_type_flag]
    if commit_message:
        command.extend(["--commit-message", commit_message])
    completed = None
    last_error = None
    for attempt in range(1, max_attempts + 1):
        try:
            completed = cli(command, check=True, capture_output=True, text=True)
            break
        except subprocess.CalledProcessError as exc:
            detail = exc.stderr or exc.stdout or str(exc)
            last_error = detail
            if attempt >= max_attempts or not _is_transient_hf_upload_error(detail, exc.returncode):
                raise
            time.sleep(retry_delay_seconds)
    ack = (fetch or fetch_hf_object)(hf_uri=hf_uri)
    return {
        "acknowledgedRevision": ack.get("xRepoCommit"),
        "localSha256": local_sha256,
        "localSizeBytes": len(local_bytes),
        "hfUri": hf_uri,
        "fetch": ack,
        "verified": ack.get("sha256") == local_sha256,
        "commandStdout": getattr(completed, "stdout", None),
        "commandStderr": getattr(completed, "stderr", None),
        "lastTransientError": last_error,
    }


__all__ = [
    "download_hf_object_bytes",
    "fetch_hf_object",
    "parse_hf_uri",
    "upload_hf_file_with_ack",
]
