#!/usr/bin/env python3
"""ssh ProxyCommand：把 SSH 連線包進 HTTP proxy 的 CONNECT 隧道。

情境（見 experiences/02 §6）
----------------------------
Claude Code 的 Bash 跑在 macOS Seatbelt sandbox 裡，對外網路只有一個洞：
本機的 HTTP 過濾 proxy，位址放在環境變數 HTTPS_PROXY，形如
``http://<user>:<pass>@127.0.0.1:<port>``，帳密與 port 每個 session 都不同。

curl、git over https、pip 這些程式認 HTTPS_PROXY，會自動把請求包給 proxy，
所以在 sandbox 內「上網」沒問題。ssh 不認 HTTP proxy，它直接對
``github.com:22`` 開 TCP，而 sandbox 裡沒有這條路（連 DNS 都解不到），
於是 ``git push`` 走 SSH 時死在 ``Could not resolve hostname``。

proxy 本身允許 ``CONNECT host:22``（實測拿得到 SSH banner）。CONNECT 建好
之後 proxy 就是一條純 TCP 管子，上面跑什麼協定它不管。所以只要有人替 ssh
「先跟 proxy 講好 CONNECT，再把 bytes 原樣轉送」，SSH 就能走這個唯一的洞。
ssh 的 ProxyCommand 正是為此設計：ssh 不自己開 TCP，改 spawn 這支程式，
把它的 stdin / stdout 當成連線。

為什麼自己寫
------------
- macOS 內建 ``nc -X connect`` 支援 CONNECT，但 proxy 密碼只能互動輸入，
  ProxyCommand 沒有 TTY。
- corkscrew / socat / ncat 預設沒裝，而且帳密、port 每個 session 隨機，
  寫死在參數或設定檔裡下次就過期。
- 這支只用標準函式庫，執行時才讀 HTTPS_PROXY，約 60 行。

它不做什麼
----------
- 不碰任何私鑰：認證仍由 ssh 透過 unix socket 請 ssh-agent 簽名。
- 不決定「能不能連」：proxy 的網域白名單、sandbox 的 socket 白名單都在它之外。

用法
----
放進 ``~/.ssh/config``，用 Match … exec 讓它只在有 HTTPS_PROXY 的環境（也就是
sandbox 內）啟用，平常終端機不受影響::

    Match host github.com exec "test -n \\"$HTTPS_PROXY\\""
        ProxyCommand python3 -I /path/to/connect-proxy.py %h %p

``ssh -G github.com | grep proxycommand`` 在外面應印 ``none``、在 sandbox 內
應印這支腳本，兩邊各看一次就知道開關方向對不對。
"""
import base64
import os
import select
import socket
import sys
from urllib.parse import unquote, urlparse


def main() -> None:
    host, port = sys.argv[1], int(sys.argv[2])
    url = os.environ.get("HTTPS_PROXY") or os.environ.get("HTTP_PROXY")
    if not url:
        sys.exit("connect-proxy: HTTPS_PROXY / HTTP_PROXY not set")
    proxy = urlparse(url)
    sock = socket.create_connection((proxy.hostname, proxy.port))

    # 對 proxy 發 CONNECT；帳密在 URL 裡，可能被 percent-encode，先還原
    req = f"CONNECT {host}:{port} HTTP/1.1\r\nHost: {host}:{port}\r\n"
    if proxy.username:
        cred = f"{unquote(proxy.username)}:{unquote(proxy.password or '')}"
        token = base64.b64encode(cred.encode()).decode()
        req += f"Proxy-Authorization: Basic {token}\r\n"
    req += "\r\n"
    sock.sendall(req.encode())

    # 讀到完整的回應標頭為止；只接受 2xx，其餘印出狀態行後結束
    buf = b""
    while b"\r\n\r\n" not in buf:
        chunk = sock.recv(4096)
        if not chunk:
            sys.exit("connect-proxy: proxy closed the connection")
        buf += chunk
    head, rest = buf.split(b"\r\n\r\n", 1)
    status = head.split(b"\r\n", 1)[0]
    if b" 200" not in status:
        sys.exit("connect-proxy: " + status.decode(errors="replace"))

    # 隧道建好。標頭之後若已夾帶資料（SSH banner 常常如此），先吐給 ssh
    out = sys.stdout.buffer
    if rest:
        out.write(rest)
        out.flush()

    # 雙向轉送：socket → stdout、stdin → socket；stdin 收到 EOF 就半關 socket，
    # 等 socket 那頭也關了才結束
    stdin_fd = sys.stdin.fileno()
    readers = [sock, stdin_fd]
    while readers:
        ready, _, _ = select.select(readers, [], [])
        if sock in ready:
            data = sock.recv(65536)
            if not data:
                break
            out.write(data)
            out.flush()
        if stdin_fd in ready:
            data = os.read(stdin_fd, 65536)
            if not data:
                sock.shutdown(socket.SHUT_WR)
                readers.remove(stdin_fd)
                continue
            sock.sendall(data)


if __name__ == "__main__":
    try:
        main()
    except (BrokenPipeError, ConnectionResetError):
        # ssh 那頭先結束了；這是正常收尾，不要噴 traceback
        pass
