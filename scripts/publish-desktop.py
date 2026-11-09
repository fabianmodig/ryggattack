#!/usr/bin/env python3
"""Metadata-gated Forgejo draft publication, then public byte verification.

Existing releases/assets are never overwritten. Failed uploads stay draft;
failures during/after publication attempt an identity-bound return to draft.
"""
import argparse
import hashlib
import importlib
import json
import os
import pathlib
import re
import subprocess
import tarfile
import urllib.error
import urllib.parse
import urllib.request
import uuid
import zipfile

pack = importlib.import_module("package-desktop")
NAMES = {"linux": "ryggattack-linux-x86_64.tar.gz", "windows": "ryggattack-windows-x86_64.zip"}


def verify_archive(path, platform, sha, tag):
    prefix = f"ryggattack-{platform}-x86_64/"
    files = {}
    if platform == "linux":
        with tarfile.open(path, "r:gz") as stream:
            for member in stream:
                if not member.isfile() or member.name in files:
                    raise ValueError("unexpected/duplicate archive member")
                handle = stream.extractfile(member)
                if handle is None:
                    raise ValueError("unreadable archive member")
                files[member.name] = handle.read()
                if member.name in (prefix + "ryggattack", prefix + "launch.sh") and member.mode != 0o755:
                    raise ValueError("missing executable archive modes")
    else:
        with zipfile.ZipFile(path) as stream:
            for member in stream.infolist():
                if member.filename in files or (member.external_attr >> 16) & 0o170000 != 0o100000:
                    raise ValueError("unexpected/duplicate archive member")
                files[member.filename] = stream.read(member)
    relative = {}
    for name, data in files.items():
        if not name.startswith(prefix):
            raise ValueError("wrong archive root")
        name = name[len(prefix):]
        if any(p in ("", ".", "..") for p in name.split("/")) or "\\" in name:
            raise ValueError("unsafe archive path")
        relative[name] = data
    exe = "ryggattack" + (".exe" if platform == "windows" else "")
    required = {exe, "source-sha.txt", "metadata.json", "README.txt"}
    if platform == "linux":
        required.add("launch.sh")
    if not required <= relative.keys() or not any(n.startswith("assets/") for n in relative):
        raise ValueError("incomplete desktop archive")
    if any(n not in required and not n.startswith("assets/") for n in relative):
        raise ValueError("unexpected desktop payload")
    metadata = json.loads(relative["metadata.json"])
    if (metadata.get("source_sha") != sha or metadata.get("tag") != tag
            or metadata.get("platform") != platform or metadata.get("target") != pack.TARGETS[platform]
            or relative["source-sha.txt"] != (sha + "\n").encode()):
        raise ValueError("desktop artifact source/tag/platform mismatch")
    pack.verify_binary(relative[exe], platform)
    if platform == "linux" and relative["launch.sh"] != pack.LAUNCHER:
        raise ValueError("unexpected Linux launcher")


def collect(directory, sha, tag):
    if not re.fullmatch(r"[0-9a-f]{40}", sha):
        raise ValueError("full source SHA required")
    payloads = {}
    for platform, name in NAMES.items():
        path = pathlib.Path(directory) / platform / name
        sums = path.parent / "SHA256SUMS"
        if path.is_symlink() or not path.is_file() or not sums.is_file():
            raise ValueError("missing desktop artifact/checksums")
        data = path.read_bytes()
        digest = hashlib.sha256(data).hexdigest()
        if sums.read_text() != f"{digest}  {name}\n":
            raise ValueError("desktop artifact checksum mismatch")
        verify_archive(path, platform, sha, tag)
        payloads[name] = data
    payloads["SHA256SUMS"] = "".join(f"{hashlib.sha256(data).hexdigest()}  {name}\n"
                                      for name, data in sorted(payloads.items())).encode()
    return payloads


class NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, req, fp, code, msg, headers, newurl):
        # Never send the automatic token to a redirected host.
        raise RuntimeError("unexpected Forgejo redirect")


class API:
    def __init__(self, server, repository, token):
        parsed = urllib.parse.urlsplit(server)
        if parsed.scheme != "https" and not (parsed.scheme == "http" and parsed.hostname == "127.0.0.1"):
            raise ValueError("HTTPS Forgejo origin required")
        if parsed.username or parsed.password or parsed.query or parsed.fragment or parsed.path not in ("", "/"):
            raise ValueError("bare Forgejo origin required")
        if not re.fullmatch(r"[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+", repository) or not token:
            raise ValueError("repository/token required")
        self.origin = server.rstrip("/")
        self.base = self.origin + "/api/v1/repos/" + repository
        self.token = token
        self.opener = urllib.request.build_opener(NoRedirect())

    def request(self, method, path, data=None, content_type="application/json", missing=False, raw=False):
        url = path if path.startswith(self.origin + "/") else self.base + path
        if urllib.parse.urlsplit(url)[:2] != urllib.parse.urlsplit(self.origin)[:2]:
            raise ValueError("cross-origin Forgejo URL")
        if data is not None and not isinstance(data, bytes):
            data = json.dumps(data).encode()
        request = urllib.request.Request(url, data=data, method=method,
                                        headers={"Authorization": "token " + self.token,
                                                 "Content-Type": content_type})
        try:
            with self.opener.open(request, timeout=120) as response:
                body = response.read()
        except urllib.error.HTTPError as error:
            if missing and error.code == 404:
                return None
            raise RuntimeError(f"Forgejo {method} failed: HTTP {error.code}") from None
        return body if raw else json.loads(body)


def publish(server, repository, token, tag, sha, directory):
    if not tag or any(ord(c) < 32 for c in tag):
        raise ValueError("tag required, without control characters")
    payloads = collect(directory, sha, tag)  # Validate before any API writes.
    api = API(server, repository, token)
    quoted = urllib.parse.quote(tag, safe="")

    def bind_tag():
        current = api.request("GET", "/tags/" + quoted)
        if current.get("name") != tag or current.get("commit", {}).get("sha") != sha:
            raise ValueError("remote tag no longer points to built source")

    def bind_release(release, draft):
        if (type(release.get("id")) is not int or release.get("id") != identity
                or release.get("tag_name") != tag or release.get("target_commitish") != sha
                or release.get("draft") is not draft):
            raise ValueError("release source/tag/draft mismatch")

    bind_tag()
    if api.request("GET", "/releases/tags/" + quoted, missing=True) is not None:
        raise ValueError("release already exists; refusing to overwrite even a draft")
    release = api.request("POST", "/releases", {"tag_name": tag, "target_commitish": sha,
                         "name": "Ryggattack " + tag, "draft": True, "prerelease": False,
                         "hide_archive_links": False,
                         "body": "Unsigned Windows/Linux x64 desktop builds. Source: " + sha
                         + "\nExtract the complete archive and read README.txt. See docs/desktop-releases.md for dependencies and limitations."})
    identity = release.get("id")
    if type(identity) is not int or identity <= 0:
        raise ValueError("invalid release ID")
    bind_release(release, True)
    path = f"/releases/{identity}"
    bind_release(api.request("GET", path), True)
    if api.request("GET", path + "/assets") != []:
        raise ValueError("new draft already has assets")
    for name, data in payloads.items():
        boundary = "desktop-" + uuid.uuid4().hex
        body = (f'--{boundary}\r\nContent-Disposition: form-data; name="attachment"; filename="{name}"\r\n'
                'Content-Type: application/octet-stream\r\n\r\n').encode() + data + f"\r\n--{boundary}--\r\n".encode()
        uploaded = api.request("POST", path + "/assets?name=" + urllib.parse.quote(name, safe=""), body,
                               "multipart/form-data; boundary=" + boundary)
        if uploaded.get("name") != name or uploaded.get("size") != len(data):
            raise ValueError("upload receipt mismatch")

    def verify_assets(download=False):
        assets = api.request("GET", path + "/assets")
        if len(assets) != len(payloads) or {a.get("name") for a in assets} != set(payloads):
            raise ValueError("release assets incomplete or unexpected")
        for asset in assets:
            data = payloads[asset["name"]]
            url = asset.get("browser_download_url", "")
            if not url.startswith(api.origin + "/") or asset.get("size") != len(data):
                raise ValueError("invalid asset readback URL/size")
            if download and api.request("GET", url, raw=True) != data:
                raise ValueError("uploaded asset bytes mismatch")

    verify_assets()
    bind_tag()
    bind_release(api.request("GET", path), True)
    # Automatic workflow tokens can inspect draft metadata but cannot download
    # draft attachments on Forgejo. Verify bytes only after making it public.
    try:
        bind_release(api.request("PATCH", path, {"draft": False}), False)
        bind_release(api.request("GET", path), False)
        verify_assets(download=True)
        bind_tag()
    except Exception as error:
        try:
            current = api.request("GET", path)
            if not isinstance(current, dict) or type(current.get("draft")) is not bool:
                raise ValueError("invalid rollback draft state")
            draft = current["draft"]
            bind_release(current, draft)  # Never mutate a different release/source.
            rollback_write_error = None
            if not draft:
                try:
                    api.request("PATCH", path, {"draft": True})
                except Exception as write_error:
                    # A failed response does not prove the write failed.
                    rollback_write_error = write_error
            try:
                bind_release(api.request("GET", path), True)
            except Exception:
                if rollback_write_error is not None:
                    raise rollback_write_error
                raise
        except Exception as rollback_error:
            error.add_note(f"ROLLBACK FAILED: cannot confirm release {identity} is draft: {rollback_error}")
        raise
    return identity


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--directory", required=True, type=pathlib.Path)
    args = parser.parse_args()
    ref = os.environ.get("GITHUB_REF", "")
    tag = os.environ.get("GITHUB_REF_NAME", "")
    if os.environ.get("GITHUB_EVENT_NAME") != "push" or ref != "refs/tags/" + tag or not tag:
        raise ValueError("publication is restricted to push tag events")
    sha = subprocess.check_output(["git", "rev-parse", "HEAD"], text=True).strip()
    for revision in (ref + "^{commit}", os.environ["GITHUB_SHA"] + "^{commit}"):
        if subprocess.check_output(["git", "rev-parse", "--verify", revision], text=True).strip() != sha:
            raise ValueError("checkout/event/tag SHA mismatch")
    publish(os.environ["GITHUB_SERVER_URL"], os.environ["GITHUB_REPOSITORY"],
            os.environ["FORGEJO_RELEASE_TOKEN"], tag, sha, args.directory)


if __name__ == "__main__":
    main()
