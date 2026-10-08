#!/usr/bin/env python3
"""
将 backend/ 部署到远程服务器：上传源码 → 远程 cargo build --release → 重启服务。

从项目根目录 .env 读取 SSH 配置：
  ssh_server / ssh_port / ssh_user / ssh_pwd

用法:
    python deploy_backend.py
    python deploy_backend.py --remote-dir /home/rock/project/lingxi
    python deploy_backend.py --dry-run
    python deploy_backend.py --skip-build      # 只上传不编译
    python deploy_backend.py --skip-restart    # 上传+编译，不重启
"""

from __future__ import annotations

import argparse
import fnmatch
import os
import sys
import time
from pathlib import Path

import paramiko
from dotenv import load_dotenv

PROJECT_ROOT = Path(__file__).resolve().parent.parent
load_dotenv(PROJECT_ROOT / ".env")

SSH_HOST = os.getenv("ssh_server", "").strip()
SSH_PORT = int(os.getenv("ssh_port", "22"))
SSH_USER = os.getenv("ssh_user", "root")
SSH_PWD = os.getenv("ssh_pwd", "")

BACKEND_DIR = PROJECT_ROOT / "backend"
DEFAULT_REMOTE_DIR = os.getenv("deploy_remote_dir", "/home/rock/project/lingxi").strip()
SERVICE_NAME = "lingxi-backend"
BINARY_NAME = "lingxi-backend"
LISTEN_PORT = int(os.getenv("app_http_port", os.getenv("PORT", "8000")))


def parse_gitignore(repo_dir: Path) -> list[str]:
    gitignore = repo_dir / ".gitignore"
    if not gitignore.exists():
        return []
    patterns = []
    for line in gitignore.read_text(encoding="utf-8").splitlines():
        line = line.strip()
        if not line or line.startswith("#"):
            continue
        patterns.append(line)
    return patterns


def is_ignored(path: Path, repo_dir: Path, patterns: list[str]) -> bool:
    rel = path.relative_to(repo_dir).as_posix()
    for pat in patterns:
        pat_clean = pat.lstrip("/")
        if fnmatch.fnmatch(rel, pat_clean) or fnmatch.fnmatch(rel, pat_clean + "/*"):
            return True
        parts = rel.split("/")
        for i in range(len(parts)):
            sub = "/".join(parts[i:])
            if fnmatch.fnmatch(sub, pat_clean) or fnmatch.fnmatch(sub, pat_clean + "/*"):
                return True
        if fnmatch.fnmatch(path.name, pat_clean):
            return True
    return False


def collect_files(repo_dir: Path, patterns: list[str]) -> list[Path]:
    files = []
    for root, dirs, filenames in os.walk(repo_dir):
        root_path = Path(root)
        if ".git" in dirs:
            dirs.remove(".git")
        kept_dirs = []
        for d in dirs:
            full = root_path / d
            if not is_ignored(full, repo_dir, patterns):
                kept_dirs.append(d)
        dirs[:] = kept_dirs
        for f in filenames:
            full = root_path / f
            if not is_ignored(full, repo_dir, patterns):
                files.append(full)
    return files


def ensure_remote_dir(sftp: paramiko.SFTPClient, remote_dir: str):
    try:
        sftp.stat(remote_dir)
    except FileNotFoundError:
        parent = os.path.dirname(remote_dir.rstrip("/"))
        if parent and parent != remote_dir:
            ensure_remote_dir(sftp, parent)
        sftp.mkdir(remote_dir)


def upload_file(sftp: paramiko.SFTPClient, local: Path, remote: str):
    ensure_remote_dir(sftp, os.path.dirname(remote))
    sftp.put(str(local), remote)


def _safe_print(text: str, file=sys.stdout):
    """避免 Windows 控制台 GBK 无法打印部分 Unicode 时崩溃。"""
    try:
        print(text, file=file)
    except UnicodeEncodeError:
        enc = getattr(file, "encoding", None) or "utf-8"
        print(text.encode(enc, errors="replace").decode(enc, errors="replace"), file=file)


def run_ssh(ssh: paramiko.SSHClient, cmd: str, check: bool = True) -> tuple[int, str, str]:
    _safe_print(f"  $ {cmd}")
    stdin, stdout, stderr = ssh.exec_command(cmd, get_pty=True)
    out = stdout.read().decode("utf-8", errors="replace")
    err = stderr.read().decode("utf-8", errors="replace")
    code = stdout.channel.recv_exit_status()
    if out.strip():
        _safe_print(out.rstrip())
    if err.strip():
        _safe_print(err.rstrip(), file=sys.stderr)
    if check and code != 0:
        raise RuntimeError(f"远程命令失败 (exit={code}): {cmd}")
    return code, out, err


def systemd_unit(remote_dir: str, port: int) -> str:
    bin_path = f"{remote_dir}/target/release/{BINARY_NAME}"
    return f"""[Unit]
Description=Lingxi Backend
After=network.target

[Service]
Type=simple
WorkingDirectory={remote_dir}
Environment=HOST=0.0.0.0
Environment=PORT={port}
Environment=DATABASE_URL=sqlite://{remote_dir}/data.db?mode=rwc
Environment=RUST_LOG=info
ExecStart={bin_path}
Restart=on-failure
RestartSec=3

[Install]
WantedBy=multi-user.target
"""


def ensure_systemd_and_restart(ssh: paramiko.SSHClient, sftp: paramiko.SFTPClient, remote_dir: str):
    unit_path = f"/etc/systemd/system/{SERVICE_NAME}.service"
    local_tmp = PROJECT_ROOT / "script" / f".{SERVICE_NAME}.service.tmp"
    local_tmp.write_text(systemd_unit(remote_dir, LISTEN_PORT), encoding="utf-8")
    try:
        sftp.put(str(local_tmp), f"/tmp/{SERVICE_NAME}.service")
        run_ssh(ssh, f"sudo mv /tmp/{SERVICE_NAME}.service {unit_path}")
        run_ssh(ssh, "sudo systemctl daemon-reload")
        run_ssh(ssh, f"sudo systemctl enable {SERVICE_NAME}")
        run_ssh(ssh, f"sudo systemctl restart {SERVICE_NAME}")
        time.sleep(2)
        run_ssh(ssh, f"sudo systemctl --no-pager --full status {SERVICE_NAME}", check=False)
    finally:
        if local_tmp.exists():
            local_tmp.unlink()


def remote_build(ssh: paramiko.SSHClient, remote_dir: str):
    # schema 由进程启动时自动执行 migrations（见 backend/src/db.rs）
    run_ssh(
        ssh,
        f"cd {remote_dir} && cargo build --release --bin {BINARY_NAME}",
        check=True,
    )


def main():
    parser = argparse.ArgumentParser(description="部署 backend/ 到远程服务器")
    parser.add_argument(
        "--remote-dir",
        default=DEFAULT_REMOTE_DIR,
        help=f"远程目标目录（默认 {DEFAULT_REMOTE_DIR}）",
    )
    parser.add_argument("--dry-run", action="store_true", help="仅打印将要上传的文件")
    parser.add_argument("--skip-build", action="store_true", help="只上传，不远程编译")
    parser.add_argument("--skip-restart", action="store_true", help="不写入 systemd / 不重启")
    args = parser.parse_args()

    if not BACKEND_DIR.exists():
        print(f"错误: backend 目录不存在: {BACKEND_DIR}", file=sys.stderr)
        sys.exit(1)
    if not SSH_HOST:
        print("错误: .env 中未找到 ssh_server", file=sys.stderr)
        sys.exit(1)
    if not SSH_PWD:
        print("错误: .env 中未找到 ssh_pwd", file=sys.stderr)
        sys.exit(1)

    print(f"SSH: {SSH_USER}@{SSH_HOST}:{SSH_PORT}")
    print(f"远程目录: {args.remote_dir}")
    print(f"服务端口: {LISTEN_PORT}")

    patterns = parse_gitignore(BACKEND_DIR)
    files = collect_files(BACKEND_DIR, patterns)
    print(f"待上传文件数: {len(files)}")

    if args.dry_run:
        for f in sorted(files):
            rel = f.relative_to(BACKEND_DIR).as_posix()
            print(f"  {rel} -> {args.remote_dir}/{rel}")
        print("\n（dry-run，未实际上传）")
        return

    ssh = paramiko.SSHClient()
    ssh.set_missing_host_key_policy(paramiko.AutoAddPolicy())
    print(f"\n连接 {SSH_USER}@{SSH_HOST}:{SSH_PORT} ...")
    ssh.connect(SSH_HOST, port=SSH_PORT, username=SSH_USER, password=SSH_PWD, timeout=30)
    sftp = ssh.open_sftp()
    print("已连接")

    try:
        ensure_remote_dir(sftp, args.remote_dir)
        success = failed = 0
        for f in sorted(files):
            rel = f.relative_to(BACKEND_DIR).as_posix()
            remote_path = f"{args.remote_dir}/{rel}"
            try:
                upload_file(sftp, f, remote_path)
                print(f"  [OK] {rel}")
                success += 1
            except Exception as e:
                print(f"  [FAIL] {rel}: {e}", file=sys.stderr)
                failed += 1
        print(f"\n上传完成: 成功 {success}, 失败 {failed}")
        if failed:
            sys.exit(1)

        if not args.skip_build:
            print("\n远程编译 release ...")
            remote_build(ssh, args.remote_dir)

        if not args.skip_restart:
            print("\n配置 systemd 并重启 ...")
            ensure_systemd_and_restart(ssh, sftp, args.remote_dir)

        print("\n健康检查 ...")
        run_ssh(
            ssh,
            f"curl -fsS http://127.0.0.1:{LISTEN_PORT}/api/health || "
            f"curl -fsS http://127.0.0.1:{LISTEN_PORT}/api/health",
            check=False,
        )
        print(f"\n公网探测: http://{SSH_HOST}:{LISTEN_PORT}/api/health")
        print("完成。")
    finally:
        sftp.close()
        ssh.close()


if __name__ == "__main__":
    main()
