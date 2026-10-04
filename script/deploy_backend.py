#!/usr/bin/env python3
"""
将 backend/ 目录部署到远程服务器。
- 从项目根目录 .env 读取 SSH 配置
- 自动解析 backend/.gitignore，排除被忽略的文件
- 通过 SFTP 上传
- 服务器上目标目录默认为 /opt/lingxi-backend

用法:
    python deploy_backend.py                  # 使用 .env 中的配置
    python deploy_backend.py --remote-dir /opt/lingxi-backend
    python deploy_backend.py --dry-run        # 仅打印将要上传的文件，不实际上传
"""

import argparse
import fnmatch
import os
import sys
from pathlib import Path

import paramiko
from dotenv import load_dotenv

# ---- 加载 .env ----
PROJECT_ROOT = Path(__file__).resolve().parent.parent
load_dotenv(PROJECT_ROOT / ".env")

SSH_HOST = os.getenv("ssh_server", "8.138.113.84")
SSH_PORT = int(os.getenv("ssh_port", "22"))
SSH_USER = os.getenv("ssh_user", "root")
SSH_PWD = os.getenv("ssh_pwd", "")

BACKEND_DIR = PROJECT_ROOT / "backend"
DEFAULT_REMOTE_DIR = "/home/rock/project/lingxi"


def parse_gitignore(repo_dir: Path) -> list[str]:
    """读取 .gitignore，返回匹配规则列表"""
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
    """判断文件是否被 .gitignore 匹配"""
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
    """收集所有需要上传的文件（排除 .gitignore 中的 + .git 目录）"""
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
    """递归创建远程目录"""
    try:
        sftp.stat(remote_dir)
    except FileNotFoundError:
        parent = os.path.dirname(remote_dir)
        ensure_remote_dir(sftp, parent)
        sftp.mkdir(remote_dir)


def upload_file(sftp: paramiko.SFTPClient, local: Path, remote: str):
    """上传单个文件，自动创建远程子目录"""
    remote_dir = os.path.dirname(remote)
    ensure_remote_dir(sftp, remote_dir)
    sftp.put(str(local), remote)


def main():
    parser = argparse.ArgumentParser(description="部署 backend/ 到远程服务器")
    parser.add_argument("--remote-dir", default=DEFAULT_REMOTE_DIR, help=f"远程目标目录（默认 {DEFAULT_REMOTE_DIR}）")
    parser.add_argument("--dry-run", action="store_true", help="仅打印将要上传的文件，不实际上传")
    args = parser.parse_args()

    if not BACKEND_DIR.exists():
        print(f"错误: backend 目录不存在: {BACKEND_DIR}", file=sys.stderr)
        sys.exit(1)

    if not SSH_PWD:
        print("错误: .env 中未找到 ssh_pwd", file=sys.stderr)
        sys.exit(1)

    print(f"SSH: {SSH_USER}@{SSH_HOST}:{SSH_PORT}")

    patterns = parse_gitignore(BACKEND_DIR)
    print(f".gitignore 规则: {patterns}")

    files = collect_files(BACKEND_DIR, patterns)
    print(f"待上传文件数: {len(files)}")

    if args.dry_run:
        print("\n--- Dry Run: 文件列表 ---")
        for f in sorted(files):
            rel = f.relative_to(BACKEND_DIR).as_posix()
            print(f"  {rel} -> {args.remote_dir}/{rel}")
        print("\n（dry-run 模式，未实际上传）")
        return

    print(f"\n连接 {SSH_USER}@{SSH_HOST}:{SSH_PORT} ...")
    ssh = paramiko.SSHClient()
    ssh.set_missing_host_key_policy(paramiko.AutoAddPolicy())
    ssh.connect(SSH_HOST, port=SSH_PORT, username=SSH_USER, password=SSH_PWD, timeout=30)
    sftp = ssh.open_sftp()
    print("已连接")

    ensure_remote_dir(sftp, args.remote_dir)

    success = 0
    failed = 0
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

    sftp.close()
    ssh.close()

    print(f"\n完成: 成功 {success}, 失败 {failed}")
    print(f"远程目录: {args.remote_dir}")
    if failed:
        sys.exit(1)


if __name__ == "__main__":
    main()
