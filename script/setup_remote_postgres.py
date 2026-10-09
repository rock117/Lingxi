#!/usr/bin/env python3
"""
远端必须以 Docker 方式运行 PostgreSQL。

步骤：
1. 若无 docker：用阿里云 docker-ce 源安装（Alibaba Cloud Linux 不支持 get.docker.com）
2. 上传 docker-compose.yml + .env
3. docker compose up -d
4. 将 lingxi-backend systemd 的 DATABASE_URL 改为 postgres 并重启
"""

from __future__ import annotations

import os
import sys
import time
from pathlib import Path

import paramiko
from dotenv import load_dotenv

ROOT = Path(__file__).resolve().parent.parent
load_dotenv(ROOT / ".env")

HOST = os.getenv("ssh_server", "").strip()
PORT = int(os.getenv("ssh_port", "22"))
USER = os.getenv("ssh_user", "root")
PWD = os.getenv("ssh_pwd", "")
APP_DIR = os.getenv("deploy_app_dir", "/home/rock/apps/lingxi").strip()
COMPOSE_REMOTE = os.getenv("deploy_compose_dir", "/home/rock/apps/lingxi").strip()
PG_DATA = os.getenv("deploy_postgres_data_dir", "/home/rock/data/lingxi-postgres").strip()
PG_USER = os.getenv("POSTGRES_USER", "lingxi")
PG_PASS = os.getenv("POSTGRES_PASSWORD", "lingxi")
PG_DB = os.getenv("POSTGRES_DB", "lingxi")
PG_PORT = os.getenv("POSTGRES_PORT", "5432")
DATABASE_URL = f"postgres://{PG_USER}:{PG_PASS}@127.0.0.1:{PG_PORT}/{PG_DB}"
LISTEN = int(os.getenv("app_http_port", "8000"))
SERVICE = "lingxi-backend"


def run(ssh: paramiko.SSHClient, cmd: str, check: bool = True) -> tuple[int, str]:
    print(f"$ {cmd}", flush=True)
    _, stdout, _stderr = ssh.exec_command(cmd, get_pty=True)
    chan = stdout.channel
    chan.settimeout(0.25)
    parts: list[str] = []

    def drain() -> None:
        while chan.recv_ready():
            chunk = chan.recv(4096).decode("utf-8", errors="replace")
            parts.append(chunk)
            sys.stdout.write(chunk)
            sys.stdout.flush()
        while chan.recv_stderr_ready():
            chunk = chan.recv_stderr(4096).decode("utf-8", errors="replace")
            parts.append(chunk)
            sys.stderr.write(chunk)
            sys.stderr.flush()

    while True:
        try:
            drain()
        except Exception:
            pass
        if chan.exit_status_ready():
            for _ in range(40):
                try:
                    drain()
                except Exception:
                    pass
                if not chan.recv_ready() and not chan.recv_stderr_ready():
                    break
                time.sleep(0.05)
            break
        time.sleep(0.05)

    code = chan.recv_exit_status()
    if parts and not parts[-1].endswith("\n"):
        print(flush=True)
    if check and code != 0:
        raise SystemExit(f"命令失败 exit={code}: {cmd}")
    return code, "".join(parts)


def ensure_registry_mirrors(ssh: paramiko.SSHClient, sftp: paramiko.SFTPClient) -> None:
    """国内拉 Docker Hub 常超时，写入 registry-mirrors。"""
    run(ssh, "mkdir -p /etc/docker", check=False)
    body = (
        "{\n"
        '  "registry-mirrors": [\n'
        '    "https://docker.m.daocloud.io",\n'
        '    "https://mirror.ccs.tencentyun.com"\n'
        "  ],\n"
        '  "log-driver": "json-file",\n'
        '  "log-opts": {"max-size": "50m", "max-file": "3"}\n'
        "}\n"
    )
    tmp = ROOT / "script" / ".daemon.json.tmp"
    tmp.write_text(body, encoding="utf-8")
    try:
        sftp.put(str(tmp), "/etc/docker/daemon.json")
    finally:
        tmp.unlink(missing_ok=True)
    run(ssh, "systemctl restart docker", check=True)
    time.sleep(2)


def pull_postgres_image(ssh: paramiko.SSHClient) -> None:
    """优先经 DaoCloud 镜像拉取，再 tag 为 compose 使用的官方名。"""
    mirrored = "docker.m.daocloud.io/library/postgres:16-alpine"
    code, _ = run(ssh, "docker image inspect postgres:16-alpine >/dev/null 2>&1", check=False)
    if code == 0:
        return
    print("\n=== 拉取 postgres:16-alpine（经国内镜像）===", flush=True)
    code, _ = run(ssh, f"docker pull {mirrored}", check=False)
    if code != 0:
        # 回退：依赖 daemon registry-mirrors 直拉官方名
        run(ssh, "docker pull postgres:16-alpine", check=True)
    else:
        run(ssh, f"docker tag {mirrored} postgres:16-alpine", check=True)


def ensure_docker(ssh: paramiko.SSHClient, sftp: paramiko.SFTPClient) -> None:
    code, _ = run(ssh, "command -v docker", check=False)
    if code == 0:
        run(ssh, "systemctl enable --now docker", check=False)
        run(ssh, "docker --version", check=True)
        ensure_registry_mirrors(ssh, sftp)
        return

    print("\n=== 安装 Docker（阿里云 docker-ce 源，alinux 用 el8）===", flush=True)
    run(ssh, "dnf install -y yum-utils", check=False)
    run(
        ssh,
        "yum-config-manager --add-repo https://mirrors.aliyun.com/docker-ce/linux/centos/docker-ce.repo",
        check=True,
    )
    # Alibaba Cloud Linux 3 ≈ el8；$releasever 常无法直接匹配 docker-ce 仓库
    run(
        ssh,
        "sed -i 's/\\$releasever/8/g' /etc/yum.repos.d/docker-ce.repo",
        check=True,
    )
    run(
        ssh,
        "dnf install -y docker-ce docker-ce-cli containerd.io docker-buildx-plugin docker-compose-plugin",
        check=True,
    )
    run(ssh, "systemctl enable --now docker", check=True)
    run(ssh, "docker --version", check=True)
    ensure_registry_mirrors(ssh, sftp)

    code, _ = run(ssh, "docker compose version", check=False)
    if code != 0:
        raise SystemExit("docker compose 插件未安装成功")


def main() -> None:
    if not HOST or not PWD:
        raise SystemExit("缺少 ssh_server / ssh_pwd")

    print(f"目标 {USER}@{HOST}")
    print(f"Postgres(Docker): {PG_USER}@{PG_PORT}/{PG_DB}  data={PG_DATA}")

    ssh = paramiko.SSHClient()
    ssh.set_missing_host_key_policy(paramiko.AutoAddPolicy())
    ssh.connect(HOST, port=PORT, username=USER, password=PWD, timeout=60)
    sftp = ssh.open_sftp()

    try:
        ensure_docker(ssh, sftp)

        run(ssh, f"mkdir -p {COMPOSE_REMOTE} {PG_DATA} {APP_DIR}", check=True)
        sftp.put(str(ROOT / "docker-compose.yml"), f"{COMPOSE_REMOTE}/docker-compose.yml")

        env_body = "\n".join(
            [
                f"POSTGRES_USER={PG_USER}",
                f"POSTGRES_PASSWORD={PG_PASS}",
                f"POSTGRES_DB={PG_DB}",
                f"POSTGRES_PORT={PG_PORT}",
                f"POSTGRES_DATA_DIR={PG_DATA}",
                "",
            ]
        )
        tmp_env = ROOT / "script" / ".remote-pg.env.tmp"
        tmp_env.write_text(env_body, encoding="utf-8")
        try:
            sftp.put(str(tmp_env), f"{COMPOSE_REMOTE}/.env")
        finally:
            tmp_env.unlink(missing_ok=True)

        pull_postgres_image(ssh)

        print("\n=== docker compose up -d ===", flush=True)
        run(ssh, f"cd {COMPOSE_REMOTE} && docker compose up -d", check=True)
        run(ssh, "docker ps --filter name=lingxi-postgres", check=False)

        ready = False
        for i in range(40):
            code, _ = run(
                ssh,
                f"docker exec lingxi-postgres pg_isready -U {PG_USER} -d {PG_DB}",
                check=False,
            )
            if code == 0:
                ready = True
                print("Postgres 容器已就绪", flush=True)
                break
            time.sleep(2)
        if not ready:
            run(ssh, "docker logs --tail 80 lingxi-postgres", check=False)
            raise SystemExit("Postgres 容器未就绪")

        print("\n=== 更新 systemd DATABASE_URL -> postgres ===", flush=True)
        db_esc = DATABASE_URL.replace("%", "%%")
        unit = f"""[Unit]
Description=Lingxi Backend
After=network.target docker.service
Wants=docker.service

[Service]
Type=simple
WorkingDirectory={APP_DIR}
Environment=HOST=0.0.0.0
Environment=PORT={LISTEN}
Environment=DATABASE_URL={db_esc}
Environment=RUST_LOG=info
ExecStart={APP_DIR}/lingxi-backend
Restart=on-failure
RestartSec=3

[Install]
WantedBy=multi-user.target
"""
        tmp_unit = ROOT / "script" / ".lingxi-backend.service.tmp"
        tmp_unit.write_text(unit, encoding="utf-8")
        try:
            sftp.put(str(tmp_unit), "/tmp/lingxi-backend.service")
        finally:
            tmp_unit.unlink(missing_ok=True)

        run(ssh, "mv /tmp/lingxi-backend.service /etc/systemd/system/lingxi-backend.service")
        run(ssh, "systemctl daemon-reload")
        run(ssh, f"fuser -k {LISTEN}/tcp 2>/dev/null || true", check=False)
        run(ssh, f"systemctl restart {SERVICE}")
        time.sleep(2)
        run(ssh, f"systemctl --no-pager --full status {SERVICE}", check=False)
        run(ssh, f"curl -fsS http://127.0.0.1:{LISTEN}/api/health; echo")

        print("\n完成：Postgres 仅通过 Docker 运行；后端已指向 postgres://127.0.0.1")
        print(f"  compose 目录: {COMPOSE_REMOTE}")
        print(f"  数据目录:     {PG_DATA}")
        print(f"  管理: docker compose -f {COMPOSE_REMOTE}/docker-compose.yml ps")
    finally:
        sftp.close()
        ssh.close()


if __name__ == "__main__":
    main()
