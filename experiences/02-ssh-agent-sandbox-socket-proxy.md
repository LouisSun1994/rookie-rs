# Experience 02 — ssh-agent、unix socket 與單一出口：sandbox 裡的 `git push` 為什麼要過三關

> 經驗不是筆記：起點是一個實際撞到的問題，往下挖到能解釋它為止。這一篇從「想在 Claude Code 的 sandbox 裡直接 `git push`，結果 `Operation not permitted`」出發，挖到 SSH 認證要過哪三層、ssh-agent 把 key 關在哪裡、unix socket 的路徑是怎麼被比對的（symlink 為什麼沒用）、launchd 的 socket 為什麼每次開機換名字、以及 sandbox 的網路為什麼只有一個洞。三天後重開機、同一條路又斷了一次，再往下挖到：key 是怎麼回到 agent 的、passphrase 與鑰匙圈各管什麼、agent 為什麼不是 proxy。
> 記錄日期：2026-11-06 → 2026-11-12｜當時定位：🥚 小白（仍未動手寫程式；這條線是 note 05 Seatbelt 的實戰續篇，碰到 VFS 路徑解析、unix socket、HTTP CONNECT）
> 起因：在 sandbox 內跑 `git status` 先死在 `~/.gitconfig`，放行後 `git push` 死在 `ssh: Could not resolve hostname github.com`；再查 `ssh-add -l` 回 `Error connecting to agent: Operation not permitted`。續集（2026-11-12）：重開機後，sandbox 內對第二個 git 主機 `git fetch` 回 `Permission denied (publickey)`，同一時間終端機卻是好的。
> 接在哪裡：note 05 §3（SBPL filter：`literal` 與 `remote unix-socket`、`network-outbound (remote ip "localhost:PORT")` 那條就是本篇的 proxy）、§8（`namei → vnode → MAC hook`：symlink 在 MAC 看到之前就解掉了）、§9（kernel 層與 harness 層各管什麼：本篇最後被攔的那一下是 harness 層）。OS 藍圖：子系統 ③（名字 → vnode）、④（unix socket）、⑤（MAC filter）。
> 本篇多數結論是**實驗觀察**（每一步都實際跑過：在 sandbox 內，或需要讀 key 檔時在自己的終端機）；少數是從觀察推的解釋，標 **推論**，未經導師確認。標 **補強** 的 blockquote 是整理進 repo 時對照 man page 補的，同樣未經導師確認。沒有腦筋急轉彎留在本文，未答的題記在 [`blueprints/os-checklist.md`](../blueprints/os-checklist.md)。

---

## 0. 一句話版本

**sandbox 裡的 SSH push 要同時過三關：網路出口、設定檔可讀、有人能用私鑰簽名。** 網路這關 sandbox 只開一個洞（本機 HTTP proxy），ssh 不認 proxy 所以得用 `ProxyCommand` 把連線包成 `CONNECT`；簽名這關靠 ssh-agent：私鑰住在 agent 的記憶體，sandbox 內的 ssh 只透過 unix socket 請它簽，key 檔從頭到尾不用開。而「能不能連到那個 socket」是 Seatbelt 對 **解析後的真實路徑** 做字面比對——symlink 不算數、glob 不算數、launchd 每次開機換名字的路徑寫不死，所以 agent 要自己起、綁在固定路徑。

**通了之後還有一件事：key 得自己回到 agent。** agent 的記憶體每次重啟歸零，而 sandbox 內的 ssh 沒有「自己讀 key 檔」這條退路，所以 agent 少一把 key 就是 `Permission denied (publickey)`，同一時間終端機照樣能用。key 回不回得來，不看 `~/.ssh` 裡有什麼檔案，看鑰匙圈裡有沒有那把 key 的項目；passphrase 加密的是私鑰檔的**內容**，不是讀取權限；agent 也不是 proxy，連線從頭到尾不經過它，它只在簽名那一下出場。

## 1. 事件：發生了什麼

### 事件一（2026-11-06 → 2026-11-09）：把 `git push` 放進 sandbox

目標：讓 Claude Code 的 Bash（跑在 Seatbelt sandbox 裡）能直接對 GitHub 做 `git fetch` / `git push`，而不是每次 push 都要關 sandbox。remote 是 `git@github.com:…`，走 SSH。

```
t0  git status (sandbox)        fatal: unable to access '/Users/admin/.gitconfig': Operation not permitted
t1  放行 ~/.gitconfig           fatal: unable to access '/Users/admin/.gitconfig-rs'   ← includeIf 拉進第二個檔
t2  放行兩個 → git status ✅     git push → ssh: Could not resolve hostname github.com   ← 第一關：網路
t3  ls ~/.ssh                   Operation not permitted                                 ← 第二關：檔案
    ssh-add -l                  Error connecting to agent: Operation not permitted       ← 第三關：簽名
t4  curl -p --proxy $HTTPS_PROXY telnet://github.com:22   → SSH-2.0-…   ← proxy 允許 CONNECT 到 22
t5  allowUnixSockets 加 ~/.ssh/agent.sock 與 /var/run/com.apple.launchd.*/Listeners
    allowRead 加 known_hosts、config          ssh-add -l 仍 EPERM；readlink ~/.ssh/agent.sock 也 EPERM
t6  allowRead 加 ~/.ssh/agent.sock            readlink ✅ → /var/run/com.apple.launchd.XXXX/Listeners
                                             connect 經 symlink / /var/run/… / /private/var/run/… 三條全 EPERM
t7  .zshrc 改成自起 ssh-agent -a ~/.ssh/agent.sock（真實 socket）   ssh-add -l ✅ 列出 key
t8  ssh-add --apple-use-keychain <github key>                     agent 裡兩把 key
t9  ~/.ssh/config: Match host github.com exec "test -n \"$HTTPS_PROXY\"" → ProxyCommand
    ssh -T git@github.com     Hi <user>! You've successfully authenticated ✅
    git fetch / git push --dry-run   ✅   ← 三關全過，不再需要關 sandbox
t10 第二個 git 主機           Host key verification failed   ← known_hosts 沒它的紀錄，與 sandbox 無關
```

當時的錯誤模型，每一個都讓某一步的結果出乎意料：

- 「sandbox 擋了 key 檔，所以 push 不行」→ 只看到三關裡的一關；就算 key 可讀，DNS 都解不出來。
- 「sandbox 內 `curl` 連得到 GitHub，所以 ssh 也連得到」→ 連得到的是 HTTPS 經 proxy；ssh 不認 proxy，直接對 22 port 開 TCP，而 sandbox 沒有這條路。
- 「ssh-agent 每次開機都要重新 `ssh-add`」→ 一半對：agent 的記憶體每次開機歸零，但 passphrase 可以存進 keychain 自動載回；當時把「key 在 agent 裡」和「passphrase 在 keychain 裡」當成同一件事。
- 「把 launchd 的 socket symlink 到固定路徑，白名單寫固定路徑就好」→ Seatbelt 比對的是解析後的路徑，symlink 只是指路牌。
- 「`/private/var/run` symlink 到 `/var/run`」→ 方向反了：是 `/var → /private/var`，前者才是真身。

### 事件二（2026-11-12）：通了三天，重開機後 `Permission denied (publickey)`

情境：事件一的設定原封不動，當天重開過機。在 sandbox 內對第二個 git 主機（下面寫成 `other-host.example`）跑 `git fetch`。時間軸接著事件一編號：

```
t11 git fetch (sandbox)                     git@other-host.example: Permission denied (publickey)
                                            ← verbose：CONNECT 隧道通、host key 比對通過，死在認證（第 ③ 關）
    ssh-add -l                              只有另一把 key，git 用的那把不在          ← agent 隨開機重啟過
t12 終端機（sandbox 外）ssh -T               一樣 Permission denied                    ← 不只是 sandbox 的問題
    ssh -o IdentitiesOnly=yes -i <key> -T   本機兩把候選 key 逐一試，都被拒
t13 ssh-keygen -lf <key>.pub                fingerprint 與主機帳號設定頁上登記的那把不符
                                            ← 幾天前用預設檔名重產過 key，舊的被覆寫，主機上登記的公鑰配不到任何私鑰
t14 把現在的公鑰登記到主機                    終端機 ssh -T ✅   sandbox 仍 Permission denied
                                            ← 終端機自己讀 key 檔；sandbox 只能問 agent，而 agent 裡沒有
t15 終端機 ssh-add --apple-use-keychain <key>   Identity added（沒問 passphrase）   sandbox fetch ✅
t16 ~/.ssh/config 加 Host 段：IdentityFile + IdentitiesOnly yes + AddKeysToAgent + UseKeychain
    sandbox fetch                           no such identity: …/<key>: Operation not permitted → Permission denied
    拿掉 IdentitiesOnly                      ✅
t17 臨時 agent 模擬重開機：ssh-agent sh -c "ssh-add -q --apple-load-keychain; ssh-add -l"
                                            只列出另一把；git 那把不會自己回來          ← t15 沒在鑰匙圈留下任何東西
t18 想替 key 設 passphrase：第一次打成 ssh-add -p（沒這個參數，什麼都沒發生）
    第二次 ssh-keygen -p，新 passphrase 直接 Enter
                                            印 saved with the new passphrase，但 ssh-keygen -y -P "" 仍印出公鑰
                                            ← passphrase 還是空的
t19 ssh-keygen -p 輸入真的 passphrase
    ssh-add --apple-use-keychain <key>      這次有問 Enter passphrase for …
    臨時 agent 再測                          兩把都列出 ✅
t20 同一天該主機把 SSH 入口搬到新 hostname     fetch：舊 hostname 不再接受 SSH 連線
    config：Host <舊> → HostName <新>；Match 清單加新 hostname
    sandbox 第一次連新 hostname              Failed to add the host to the list of known hosts
    終端機先連一次收下 host key               sandbox fetch ✅（remote URL 一個都沒改）
t21 想親眼看鑰匙圈那筆 passphrase             鑰匙圈存取 App 搜不到；security 指令查檔案型鑰匙圈也沒有
                                            ← 但臨時 agent 明明載得回來
t22 盤點每把私鑰有沒有 passphrase：ssh-keygen -y -P "" -f <key>，只看 exit code
                                            出現反例：有一把舊 key 檔案沒加密，臨時 agent 卻照樣載回它
                                            ← t17 當時「鑰匙圈只記得有 passphrase 的 key」的解釋被推翻一半
```

這一輪的錯誤模型，幾乎每一條都對應一句當時說出口的話：

- 「config 的 `Match host` 有列這個主機，所以 sandbox 內應該通」→ Match 段裡只有 `ProxyCommand`，它的工作（第 ① 關）早就做完了；死的是第 ③ 關。
- 「終端機的 git 能用，就代表 agent 裡有那把 key」→ 終端機的 ssh 會自己讀預設檔名的 key 檔，agent 裡有沒有都過。
- 「`ssh-add -l` 現在列得出來，重開機後也會在」→ `-l` 看的是這個 agent 此刻的記憶體；重開機後誰回來，是鑰匙圈決定的。
- 「`.zshrc` 已經有 `ssh-add --apple-load-keychain`，`~/.ssh` 的 key 都會自動載回，config 不用再寫什麼」→ 它不掃目錄，只認鑰匙圈裡的項目；沒 passphrase 的 key 用 `--apple-use-keychain` 加，沒有東西可存。
- 「關掉 terminal 再開，agent 就是新的」→ agent 是獨立的 process；`.zshrc` 的邏輯是「活著就沿用」。
- 「passphrase 是鑰匙圈裡那筆的 label」「`cat` 得出私鑰檔，表示 passphrase 沒在保護它」→ label 是 key 的路徑、passphrase 是那筆的 secret；passphrase 加密的是檔案內容，`cat` 看到的只是信封。
- 「ssh-agent 感覺就是一種 ssh proxy」→ 連線不經過 agent，它只回簽名。
- 「`ssh-keygen` 不是產 key 的嗎，怎麼能拿來『開鎖』」→ 它是 key 的工具箱；`-y -P ""` 只是拿空 passphrase 試一次門。

往下挖的順序：三關各是什麼（§2）→ ssh-agent 把 key 關在哪（§3），事件二的東西幾乎都落在這一節：agent 為什麼不是 proxy（§3.1）、一把 key 拆成哪四樣（§3.2）、client 怎麼挑 key（§3.3）、key 怎麼回到 agent（§3.4）、passphrase 是什麼（§3.5）、鑰匙圈替你記了什麼（§3.6）、不開檔能問出什麼（§3.7）→ unix socket 的路徑怎麼被比對、symlink 為什麼沒用（§4）→ launchd 的 socket 為什麼每次換名字、`/var` 與 `/private` 是什麼（§5）→ sandbox 的網路為什麼只有一個洞、ssh 怎麼走它、config 的兩種段落各管什麼（§6）→ 回頭解剖兩次事件（§7）→ 解法對照（§8）。

## 2. SSH 認證要過的三關

**核心規則**：`git push` 走 SSH 時，ssh 這個 process 要做三件事，每件都會碰到 sandbox 的一條規則；任一條沒過就失敗，而且**最先倒的那關會遮住後面的**。

| 關 | ssh 要做什麼 | 碰到哪條 sandbox 規則 | 擋住時的錯誤 |
|---|---|---|---|
| ① 網路 | DNS 解 `github.com`、對 22 port 開 TCP | `network-outbound`：只允許連到本機 proxy | `Could not resolve hostname`（DNS 都沒路） |
| ② 檔案 | 讀 `~/.ssh/config`、`known_hosts`（驗 host key） | `file-read*`：`$HOME` 整個 deny，只開白名單 | `Host key verification failed` 或 config 被忽略 |
| ③ 簽名 | 用私鑰對 server 的 challenge 簽名 | 讀 key 檔 → `file-read*` deny `**/id_*`、`**/*.pem`；或連 agent → `network-outbound (remote unix-socket …)` | `Permission denied (publickey)`、`Error connecting to agent` |

- 三關是三個不同的 syscall：①是 `socket` + `connect(AF_INET)`，②是 `open`，③是 `open`（讀 key 檔）或 `connect(AF_UNIX)`（連 agent）。Seatbelt 在每個「取得資源」的 syscall 上各查一次 label（note 05 §2），所以三關各自獨立放行。
- 第 ③ 關有兩條路，sandbox 的設計意圖是**封死讀 key 檔那條、只留 agent 那條**：讀 key 檔等於把私鑰 bytes 交給 sandbox 內的程式；連 agent 只是請外面的程序簽一次名。
- 第 ① 關最先倒，所以 t2 只看到 DNS 錯誤；要知道 ②③ 的狀態得分開測（t3 直接 `ls ~/.ssh`、`ssh-add -l`）。

## 3. ssh-agent：key 住在記憶體，簽名不出門

**核心規則**：ssh-agent 是一個 user-mode daemon，私鑰解開後只存在它的記憶體；任何 ssh client 要用 key 時，把「要簽的資料」透過 unix socket 送進去，拿回「簽名」。**key 的 bytes 永遠不經過 client**，所以 client 所在的 sandbox 不需要讀 key 檔。

**關鍵細節**

```
ssh (sandbox 內)                       ssh-agent (sandbox 外，使用者自己的 process)
 │ connect(AF_UNIX, $SSH_AUTH_SOCK)      │  私鑰（已解開）在 heap 裡
 │──── SSH_AGENTC_REQUEST_IDENTITIES ───▶│
 │◀─── 公鑰清單 ─────────────────────────│  server 用公鑰決定接受哪把
 │──── SSH_AGENTC_SIGN_REQUEST(data) ───▶│
 │◀─── signature ────────────────────────│  簽名在 agent 內完成
 │  把 signature 轉給 server              │
```

- 兩個邊界要分清：**kernel 的 MAC 邊界**（sandbox 內的 ssh 能不能 `connect` 到那個 socket）與 **agent 自己的信任邊界**（連上了就能請它簽任何東西）。sandbox 放行 socket 之後，sandbox 內任何程式都能用你的身分簽 SSH，只是拿不走 key；這是「開這個洞」真正的代價。
- agent 裡的東西壽命 = 到 agent 程序結束（通常是關機）。這跟「passphrase 存在哪」是兩回事，§3.2 把它們拆開。

事件一只用到上面這個模型就通了。事件二暴露的是它沒講的部分：agent 裡的 key 是**怎麼來的**、重開機後**誰負責讓它回來**。下面七小節是事件二挖出來的，按「全景 → 東西住哪 → client 怎麼挑 → 怎麼載回 → passphrase → 鑰匙圈 → 怎麼觀測」排。

### 3.1 全景：agent 是簽名代辦，不是 proxy

**核心規則**：agent 站在 client **旁邊**，不在 client 與 server **之間**。SSH 連線的 bytes 從頭到尾不經過它；它只在認證那一刻被問兩件事：「你有哪些公鑰」「幫我簽這一段」。

```
磁碟（永久）                                   記憶體（到 agent 這個 process 結束）
  私鑰檔 ~/.ssh/<key>  ── ssh-add <檔> ──────────▶  ssh-agent
  （內容可用 passphrase 加密）  讀檔、解密             ├ 解開的私鑰，放在 heap
                                                    └ 對外只有一個 unix socket：$SSH_AUTH_SOCK
  鑰匙圈：只記 passphrase，不存 key
        └── ssh-add --apple-load-keychain ──▶ 拿這個 passphrase 去解上面那個檔，再放進 agent

誰來用 agent
  終端機的 ssh / git ───┐
                        ├─ connect(AF_UNIX) ─▶ agent：① 列出公鑰  ② 簽這段 challenge
  sandbox 的 ssh / git ─┘                      ◀─ 只回簽名，key 不出門
        │
        │  TCP，上面跑 SSH（sandbox 內這一段包在 ProxyCommand 的 CONNECT 隧道裡，§6）
        ▼
  git 主機的 sshd：發 challenge，拿帳號上登記的公鑰驗簽名
```

- 三個角色：**私鑰檔**（磁碟上的檔案）、**ssh-agent**（常駐 process，狀態只有記憶體裡那幾把解開的 key）、**ssh client**（終端機的 ssh / git，或 sandbox 裡的）。鑰匙圈是配角，只幫忙記 passphrase。
- `SSH_AUTH_SOCK` 這個環境變數就是「agent 在哪」的全部資訊；§4–§5 整段都在處理「sandbox 內連不連得到它指的那個 socket」。
- 「感覺像 ssh proxy」的直覺錯在位置：

| | ssh-agent | proxy / `ProxyCommand`（§6） |
|---|---|---|
| 站在哪 | client 旁邊，本機 unix socket | client 與 server 之間的 TCP 路徑上 |
| 經手什麼 | 一段要簽的資料進去、一個簽名出來 | 整條連線的每一個 byte |
| 什麼時候出場 | 認證那一刻 | 連線從頭到尾 |
| 少了它的錯誤 | `Permission denied (publickey)`（沒人能簽） | `Could not resolve hostname`（沒路可走） |
| 對應哪一關 | ③ | ① |

### 3.2 一把 key 拆成四樣東西

**核心規則**：「一把 key」其實是四樣東西，住的地方與壽命各不相同；搞混其中任兩樣，就會預測錯「重開機後還在不在」。

| 東西 | 存在哪 | 壽命 | 誰寫進去 | 丟了會怎樣 |
|---|---|---|---|---|
| 公鑰 | `<key>.pub`、私鑰檔內的明文區、**伺服器帳號上登記的那份** | 永久 | `ssh-keygen` 產生；人貼到伺服器 | 本機的可從私鑰重算（`ssh-keygen -y`）；伺服器那份與本機私鑰配不上 = `Permission denied (publickey)`（t13） |
| 私鑰檔 | `~/.ssh/<key>`，內容可用 passphrase 加密 | 永久 | `ssh-keygen` 產生；同檔名重產、答應覆寫，舊的就沒了 | 沒得救，只能重產、重新登記公鑰 |
| passphrase | 人的腦袋；選配：macOS 鑰匙圈裡的一筆項目 | 永久 | `ssh-keygen -p` 設定；`ssh-add --apple-use-keychain <key>` 存進鑰匙圈（**key 有 passphrase 才有東西可存**，§3.4） | 鑰匙圈只替你用、不給你看（§3.6）；忘了就解不開私鑰檔 |
| 解開的私鑰 | ssh-agent 的記憶體 | 到 agent 這個 process 結束 | `ssh-add`（手動、預載、懶載入三條路，§3.4） | 再載一次就好，前提是前三樣都在 |

- 「每次開機都要重新加」的體感來自第四列：agent 的記憶體每次都空。解法不是不用加，是讓「加」這件事**自動且不問密碼**：passphrase 從鑰匙圈來（第三列）。
- agent 的壽命是 **process 的壽命**，不是 terminal 的：關掉視窗、開新 session，`.zshrc` 的邏輯是「`ssh-add -l` 連得上就沿用」（§5），看到的是同一個 agent、同一份清單。會讓清單歸零的是重開機，或 agent 被 kill。
- 檔名不代表內容：這次那把 key 檔名是 `id_ed25519`，`ssh-add -l` 顯示的卻是 RSA 3072。預設檔名只是「ssh 會去試的路徑」，裡面放什麼演算法的 key 它都照用。

### 3.3 client 挑 key：問 agent 一條路，自己讀檔一條路

**核心規則**：ssh 認證時的候選 key 有兩個來源：**agent 回報的清單**，以及 **`IdentityFile` 指到的檔案**（沒設就是預設檔名 `~/.ssh/id_rsa`、`id_ecdsa`、`id_ed25519`…）。終端機兩條都通，sandbox 只通第一條。事件二整個就是這張表：

| 來源 | 終端機 | sandbox |
|---|---|---|
| 問 agent（`connect` 到 `$SSH_AUTH_SOCK`） | ✅ | ✅（socket 在白名單，§4） |
| 自己讀 key 檔（預設檔名或 `IdentityFile`） | ✅ | ❌ `file-read*` deny → `no such identity: …: Operation not permitted` |
| agent 少一把 key 時 | 退到讀檔，照樣過，所以平常無感 | 沒有退路 → `Permission denied (publickey)` |

- 這解釋 t14：同一台機器、同一把 key、同一份 config，終端機 ✅、sandbox ❌。差別不在 ssh 的設定，在「讀 key 檔」這個 syscall 過不過得了 MAC（§2 第 ③ 關的兩條路）。
- `Permission denied (publickey)` 的字面意思是：server 看過 client 遞上來的每一把公鑰，沒有一把在這個帳號上登記過。它有兩種完全不同的成因，要分開驗：
  - **遞不出對的那把**（t11、t14）：對的 key 不在 agent、檔案又讀不到。看 `ssh-add -l`。
  - **遞了但伺服器不認**（t12–t13）：本機的 key 與伺服器登記的不是同一把。`ssh-keygen -lf <key>.pub` 印 fingerprint，與伺服器帳號設定頁上那串對照。
- `IdentitiesOnly yes` 在 sandbox 內的坑（t16）：它的語意是「只用設定檔指名的 identity 檔，agent 多給的不要」。ssh 得先知道「指名的那個檔對應哪把公鑰」，才能從 agent 的清單裡把它挑出來；sandbox 內 key 檔讀不到，於是 agent 裡明明有同一把 key 也被略過，剩下讀檔這條路又是 EPERM。加了就壞、拿掉就好，兩次都實測。拿掉的代價只是 agent 裡其他 key 會先被遞出去試一次。

> **推論**：t13 的遠因是幾天前用預設檔名重新跑過 `ssh-keygen`，舊私鑰被覆寫。依據是私鑰檔與 `.pub` 的修改日期相同，而且正好是伺服器上那把 key「最後使用」的隔天；沒有其他紀錄可以直接證明。

> **推論**：`<key>.pub` 在 sandbox 內同樣不可讀（`$HOME` 整個 deny，`~/.ssh` 底下只放行 `config`、`known_hosts`、`agent.sock`），所以 ssh 連「從公鑰檔認出是哪一把」這條路也沒有。這點是從白名單內容推的，沒有單獨測過只放行 `.pub` 的情況。

> **補強**：`man ssh_config` 的 `IdentityFile` 段寫明可以把它指向**公鑰檔**，讓 ssh 使用 agent 裡對應的那把私鑰。所以真的需要 `IdentitiesOnly` 時（agent 裡 key 很多、伺服器對嘗試次數有上限），sandbox 內理論上可行的寫法是 `IdentityFile ~/.ssh/<key>.pub`，並把這個 `.pub` 加進 read 白名單。這次沒有驗證。

### 3.4 key 回到 agent 的三條路

**核心規則**：agent 重啟後，一把 key 要回到記憶體只有三條路。三條都需要**某個 process 讀得到 key 檔**，所以沒有一條能從 sandbox 內觸發；差別在「誰讀、什麼時候、憑什麼」。

| 路 | 誰讀 key 檔 | 什麼時候發生 | 前提 | 缺口 |
|---|---|---|---|---|
| 手動：`ssh-add <key>` | ssh-add | 人下指令時 | 人記得 | 每次重開機都要做 |
| 預載：`ssh-add --apple-load-keychain` | ssh-add | 開 shell 時（寫在 `.zshrc`，§5） | **鑰匙圈裡有那把 key 的項目** | 沒有項目的 key，它根本不知道存在 |
| 懶載入：config 的 `AddKeysToAgent yes`（+ `UseKeychain yes`） | ssh client | 第一次**用檔案**認證成功時，順手加進 agent | 那一次 ssh 讀得到 key 檔 | 重開機後第一個 git 動作若發生在 sandbox，仍是 `Permission denied` |

- `AddKeysToAgent` 預設是 `no`。所以終端機每次都是直接讀檔、用完就算了，agent 不會因為「這把 key 剛用過」而有它。t11 就是這樣來的：終端機一直好好的，agent 裡卻始終沒有那把。
- 預載**不掃 `~/.ssh` 目錄**。man page 對它的描述是「用鑰匙圈裡存著的 passphrase 把 identity 加進 agent」：清單來源是鑰匙圈，不是檔案系統。
- `--apple-use-keychain` 是「存」那一半：加 identity 的同時把 passphrase 存進鑰匙圈。key 沒有 passphrase 時 ssh-add 不需要問、也就沒有東西可存，鑰匙圈不會多出項目（t15 → t17 實測：加完之後開臨時 agent 預載，那把 key 沒回來）。
- 一眼分辨「這次有沒有存進鑰匙圈」：`ssh-add --apple-use-keychain <key>` **有沒有問** `Enter passphrase for …`。沒問、直接印 `Identity added`，這次只進了 agent 的記憶體（t15）；有問，鑰匙圈才有那一筆（t19）。
- 這一條回頭修正了事件一：t8 用 `--apple-use-keychain` 加進去的 key，重開機後同樣沒回來（t11 的 agent 只剩一把），是同一個原因。2026-11-09 的「三關全過」其實只活到下一次重開機，事件二就是它到期的那一天。「`--apple-use-keychain` 做一次就好」少了一個前提：key 要有 passphrase。
- 懶載入需要 client 自己讀 key 檔，所以 sandbox 內的第一次 ssh 走不了這條；只有預載能讓 sandbox 內永遠只需要 socket。

### 3.5 passphrase：加密的是私鑰檔的內容，不是讀取權限

**核心規則**：「能不能讀到這個檔的 bytes」由檔案權限（DAC，`-rw-------`）與 sandbox（MAC）決定；「讀到的 bytes 能不能當私鑰用」由 passphrase 決定。兩層互不相干，所以有 passphrase 的私鑰檔照樣 `cat` 得出來。

```
-----BEGIN OPENSSH PRIVATE KEY-----      ← 信封：純文字，有沒有加密都長這樣
base64(
  "openssh-key-v1"                        格式標記
  ciphername    aes256-ctr   │ none       私鑰區用什麼加密
  kdfname       bcrypt       │ none       passphrase 怎麼推成加密金鑰
  kdfoptions    salt、rounds │ （空）
  公鑰                                     永遠明文
  私鑰區        密文         │ 明文        只有這一段受 passphrase 保護
)
-----END OPENSSH PRIVATE KEY-----
```

- 欄位是簡化過的 OpenSSH 私鑰格式；左欄是有 passphrase 的檔、右欄是沒有的。兩種檔 `cat` 起來幾乎一樣，差別藏在 base64 解開後的前幾個欄位，肉眼分不出來，工具分得出來（§3.7 的 `ssh-keygen -y -P ""`）。
- 用 zip 比：沒 passphrase 的私鑰 = 沒加密的 zip，拿到檔案就等於拿到 key；有 passphrase 的 = 有密碼的 zip，一樣能複製、能 `cat`，解不開就只是一團亂數。
- 公鑰本來就不加密：`ssh-rsa AAAA…` 那一行設計上就是要給別人的，貼到伺服器的就是它。passphrase 從頭到尾只跟私鑰區有關。
- 改 passphrase 用 `ssh-keygen -p -f <key>`（`ssh-add` 沒有 `-p`，t18 第一次就是拿錯工具）。它只重寫私鑰檔的加密層，**公鑰不變**，伺服器上登記的那份不用動。
- `ssh-keygen -p` 在新 passphrase 直接按 Enter 時等於設成空的，訊息卻一樣印 `saved with the new passphrase`（t18 第二次）。有沒有真的設上，要另外驗（§3.7）。

| | 情境 | 為什麼 |
|---|---|---|
| 防得了 | 檔案離開你手上：備份、不小心 commit 進 repo、掃 `~/.ssh` 的惡意套件、被拆走的硬碟 | 拿到檔案還得猜 passphrase；bcrypt 讓每猜一次都很慢 |
| 防不了 | 對方已經在你登入中的機器上、連得到 agent 的 socket | agent 裡是解開後的 key，請它簽名不會再問 passphrase（§3 開頭的信任邊界） |

- 所以 passphrase 與 sandbox 擋的是兩個不同方向：sandbox 的 `file-read*` 讓裡面的程式**讀不到** key 檔；passphrase 讓**讀到了**的人也用不了。
- 上面那張欄位圖是格式規格，不是這次親眼看到的輸出：當時在終端機跑了「把私鑰檔 base64 解開、只印前三個字串」的指令，但要把那個分頁的輸出讀回來時，被 Claude Code 的 auto-mode 分類器拒絕。這是 harness 層第二次出手（第一次是 §6 的 `-o ProxyCommand`），kernel 一樣沒參與（note 05 §9）。

### 3.6 鑰匙圈：替你用，不給你看

**核心規則**：鑰匙圈裡每把 key 一筆項目，內容是 passphrase，不是 key。它的工作是在預載時**替你輸入** passphrase；決定「重開機後哪些 key 會自己回來」的，是鑰匙圈裡**有沒有那一筆**。

- 那筆項目依慣例是「label `SSH: <key 的路徑>`、secret 是 passphrase」：label 用來找檔案，secret 用來解檔案。所以「passphrase 是 label」說反了。這次沒能直接看到這筆項目，見下一點。
- 看不到（t21）：`security list-keychains` 列出的檔案型鑰匙圈（login、System 等）裡，用 label 找、用路徑找、整個 dump 出來 grep，都沒有任何 SSH 項目；「鑰匙圈存取」App 也搜不到。但 `--apple-load-keychain` 明明把 key 載回來了，所以 passphrase 一定存在某處。

> **推論**：這版 macOS（Darwin 25）的 `ssh-add` 把 passphrase 存在 Data Protection 鑰匙圈（與 iCloud 鑰匙圈同一套底層），不是傳統的 `login.keychain-db` 檔案，而且沒有開放給 `security` 指令或 GUI 列出。只從「檔案型鑰匙圈查不到、行為上卻載得回來」推得，沒有查 Apple 對 OpenSSH 的 patch。

- 於是**唯一的探針是行為**：開一個全新的 agent 只做預載，看它列出誰（§3.7 的臨時 agent）。這比在 GUI 看到一筆項目更直接，因為驗的就是「重開機後會發生什麼」。
- 實際的風險：passphrase 一旦忘記，沒有地方查得回來。鑰匙圈只會替你用；`ssh-keygen -p` 要先輸入舊的；agent 裡解開的 key 設計上不吐出來。結局是重產 key、重新登記公鑰，回到 t13。所以 passphrase 要另外存進密碼管理工具：鑰匙圈負責讓它自動運作，密碼管理工具負責讓人查得到。
- 要移除那一筆也是用指令：`ssh-add -d` 搭配 `--apple-use-keychain`，移除 identity 的同時把 passphrase 從鑰匙圈刪掉（man page）。刪掉的只是「自動載回」這件事，私鑰檔不受影響。
- 反例（t22）：t17 當時的解釋是「鑰匙圈只記得有 passphrase 的 key」。盤點後發現有一把舊 key，檔案用空 passphrase 就開得了，臨時 agent 卻照樣把它載回來。所以站得住的說法只有：**決定因素是鑰匙圈裡有沒有那把 key 的項目，不是檔案現在有沒有加密**。至於「沒加密的 key 怎麼會有項目」，這次的實驗反而顯示對沒 passphrase 的 key 跑 `--apple-use-keychain` 不會建項目（t15 → t17）。兩個觀察並存、來源未解，記到 checklist。

> **推論**：那筆項目的來源當時猜了兩個，都無法驗證（項目列不出來）：這把 key 以前有過 passphrase、當時存進鑰匙圈，後來 passphrase 被拿掉而項目留著；或是較舊版 macOS 的 ssh-add 對空 passphrase 也會建項目。

> **補強**：還有第三個可能。那把舊 key 在 config 裡有自己的 `Host` 段（`UseKeychain yes`），是由 ssh client 載入的；新的這把只走過 `ssh-add --apple-use-keychain`。兩條路對「空 passphrase 要不要建項目」的行為未必相同。同樣沒有驗證。

### 3.7 觀測工具：不開檔也問得出來的事

**核心規則**：每個問題找一個「只回答這一件事」的指令，並且看它**碰了什麼、輸出去了哪**。判斷一把 key 有沒有 passphrase，不需要任何人看到 key 的內容。

| 想知道 | 指令 | 看什麼 | 它碰了什麼 |
|---|---|---|---|
| 這個 agent 此刻有誰 | `ssh-add -l` | 清單與 exit code 0 / 1 / 2（§5） | 只問 agent，不碰檔案 |
| 重開機後誰會自己回來 | `ssh-agent sh -c "ssh-add -q --apple-load-keychain; ssh-add -l"` | 臨時 agent 的清單 | 新起一個 agent，跑完就消失；不碰平常那個、不改鑰匙圈 |
| 這把 key 有沒有 passphrase | `ssh-keygen -y -P "" -f <key> >/dev/null 2>&1` | exit code：0 = 空的，非 0 = 有設 | ssh-keygen 讀檔；輸出全丟，只留 1 bit |
| 本機哪把是伺服器登記的那把 | `ssh-keygen -lf <key>.pub` | fingerprint，對照伺服器帳號設定頁 | 只讀公鑰 |
| ssh 對這個 host 最後套了哪些設定 | `ssh -G <host>` | `hostname`、`identityfile`、`identitiesonly`、`proxycommand` | 只解析 config，不連線 |

- `ssh-agent <command>` 這個用法：起一個新的 agent、把 `SSH_AUTH_SOCK` 指過去、把 command 當它的子程序跑，command 結束 agent 自動退出（man page）。壽命就是那一行，所以是乾淨的「模擬重開機」：全新的 agent 什麼都不知道，它列得出來的 key 只可能來自鑰匙圈。
- `ssh-add -l` 與臨時 agent 問的是兩個不同的問題。t15 之後兩者答案不一樣正是重點：前者說「現在有」，後者說「重開機後沒有」。
- `ssh-keygen` 的名字有誤導性，它是 key 的管理工具箱，「產生」只是不帶參數時的預設動作：

| 參數 | 做什麼 | 這次用在哪 |
|---|---|---|
| （無） | 產生新 key；檔名已存在時會問要不要覆寫 | 事件二的遠因（t13） |
| `-p` | 改私鑰檔的 passphrase | t18–t19 |
| `-y` | 讀私鑰、印出對應的公鑰 | 判斷有沒有 passphrase |
| `-P <字串>` | 搭配 `-y` / `-p`，直接給 passphrase，不互動問 | 給空字串當探針 |
| `-l` | 算 fingerprint | t13 對照伺服器登記的 key |

- `-y -P ""` 不是開鎖，是**拿空鑰匙試一次門**：要印公鑰就得先載入私鑰，載入時用的 passphrase 是空字串。檔案沒加密，門本來就開著，exit 0；檔案有加密，空字串推不出對的金鑰，報 `incorrect passphrase`，exit 非 0。它不繞過任何加密，給錯 passphrase 一樣失敗；能「開」的只有本來就沒鎖的檔。
- 從資訊流看這條指令：讀 key 檔的是 `ssh-keygen` 這個 process，它的 stdout / stderr 導到 `/dev/null`，呼叫的人拿到的只有 exit code，每個檔一個 bit。「碰到 key 檔的指令」與「看到 key 的內容」是兩回事，要追的是輸出流到哪。
- 這些要讀 key 檔的指令在 sandbox 內全部跑不了（`file-read*` deny），所以事件二裡每一條都是在自己的終端機跑、逐條核准的。這是 §3 信任邊界的另一面：sandbox 內的程式能請 agent 簽名，卻連「這把 key 有沒有 passphrase」這一個 bit 都問不到。

## 4. unix socket 的名字是檔案系統裡的路徑：MAC 看的是解析後的那個

**核心規則**：`connect(AF_UNIX, path)` 的 `path` 是檔案系統路徑，kernel 一樣走 `namei()` 把它解成 vnode（symlink 在這一步解掉），**之後**才到 MAC hook；Seatbelt 的 `remote unix-socket` filter 拿到的是解析完的路徑。所以白名單寫 symlink 的名字，永遠對不上。

**關鍵細節**

```
ssh-add: connect("/Users/admin/.ssh/agent.sock")
 └ VFS namei
     /Users/admin/.ssh/agent.sock  是 symlink → /var/run/com.apple.launchd.XXXX/Listeners
     /var                          是 symlink → /private/var
     → vnode = /private/var/run/com.apple.launchd.XXXX/Listeners          ← 真身
   ├ MAC hook (Sandbox.kext)：拿這個路徑比對 network-outbound 的 unix-socket filter
   │     "/Users/admin/.ssh/agent.sock"                 ← 牌子上的名字，不是真身，沒中
   │     "/var/run/com.apple.launchd.*/Listeners"       ← 含 *，沒中（見下）
   │     "/private/var/run/com.apple.launchd.*/Listeners" ← 含 *，沒中
   └ EPERM
```

實驗對照（t5–t7，三條白名單都在時）：

| 連的路徑 | 結果 | 讀法 |
|---|---|---|
| `~/.ssh/agent.sock`（symlink） | EPERM | 解析後不是白名單上的任何一條字面值 |
| `/var/run/com.apple.launchd.XXXX/Listeners` | EPERM | `/var` 本身是 symlink，解成 `/private/var/…`；且白名單那條含 `*` |
| `/private/var/run/com.apple.launchd.XXXX/Listeners` | EPERM | 已是真身；白名單那條含 `*` 仍沒中 |
| 自起 agent 綁在 `~/.ssh/agent.sock`（真實 socket） | ✅ | 解析後就是它自己，等於白名單的字面值 |

- 這跟 note 05 §8 的 `open("/Users/admin/.gitconfig")` 是同一條流程，只是 syscall 換成 `connect`；unix socket 在 VFS 眼裡是一個 `S_IFSOCK` 類型的 vnode，有名字、有 owner、有 mode。
- 要讓 symlink 本身可被 `readlink`（t6），symlink 那個路徑也得在 `file-read*` 白名單；但那只解決「能不能解析」，解析完的目的地仍要過 `network-outbound`。兩條規則、兩個 operation family（note 05 §3）。

> **推論**：Claude Code 的 `allowUnixSockets` 翻成 SBPL 時用的是 `literal` filter，沒有把 `*` 翻成 `regex`，所以 glob 不生效。SBPL 本身有 `regex` filter（note 05 §3 的表），所以這是 harness 層的選擇，不是 kernel 做不到。官方範例（`/var/run/docker.sock`）也都是字面路徑。這點只從「glob 三種寫法全部沒中、字面真實 socket 一次就中」推得，沒看 harness 原始碼。

## 5. launchd 的 socket 為什麼每次開機換名字：`/var/run`、`/private` 與固定路徑

**核心規則**：macOS 登入時 launchd 自動為每個 GUI session 起一個 ssh-agent，socket 放在 `/var/run/com.apple.launchd.<隨機>/Listeners`，路徑透過 `SSH_AUTH_SOCK` 注進整個登入 session 的環境。隨機是設計：每個登入 session 各一份、互不撞名、外人猜不到。但「字面比對的白名單」需要的恰好是「開機後不變的名字」，兩者衝突。

**關鍵細節**

| 目錄 | 放什麼 | 壽命 | 為什麼 agent socket 在這 |
|---|---|---|---|
| `/var` | **var**iable：執行期會長大、會變的資料（log、spool、cache） | 持久 | — |
| `/var/run` | 這次開機的執行期狀態：pid 檔、lock、unix socket | **只活到關機** | socket 是「一個正在跑的程序的入口」，不是設定也不是資料，教科書式歸類 |
| `/tmp` | 任何人可寫的暫存 | 開機清或定期清 | 舊版 macOS 把這個目錄放 `/private/tmp`，後來搬到 `/var/run`，分類更精確 |
| `/private` | macOS 獨有：`/etc`、`/var`、`/tmp` 的真身 | — | NeXTSTEP 時代為了 NetBoot 把「每台機器自己的、可寫的」集中到一處，根目錄放 symlink；佈局留到今天 |

- `launchctl getenv SSH_AUTH_SOCK` 看得到 launchd 給的路徑；sandbox 內看不到（`launchctl` 要跟 launchd 講 Mach IPC，被 `mach-lookup` 擋，note 05 §7）。
- 在一台機器上 `/private/tmp` 下找不到 `com.apple.launchd.*` 不代表 agent 沒跑，只是這版 macOS 放在 `/private/var/run`。

**兩種拿到固定路徑的做法**

| | symlink 版：`ln -sf $SSH_AUTH_SOCK ~/.ssh/agent.sock` | 自起版：`ssh-agent -a ~/.ssh/agent.sock` |
|---|---|---|
| agent 程序 | 用系統的那個，不多起 | 多一個，與系統的並存 |
| tmux 舊 session | 解決（symlink 每次新 shell 重指） | 解決（路徑本來就固定） |
| Seatbelt 字面白名單 | **不解決**（§4：比對的是解析後的隨機路徑） | 解決（socket 真身就在固定路徑） |
| 從 Dock 開的 GUI 程式 | 繼承 launchd 的路徑 → 同一個 agent | 繼承 launchd 的 → 系統那個空 agent；要用得 `launchctl setenv SSH_AUTH_SOCK …` |
| 殘留 socket 處理 | 不用（`ln -sf` 覆蓋） | 要：開機後舊 socket 檔是死的，`ssh-add -l` 回 exit 2 時 `rm -f` 再起 |

- `ssh-add -l` 的三個 exit code 要分開用：0 有 key、1 agent 在但空的、2 連不上。重起 agent 的條件是 **2**；寫成 `if ! ssh-add -l` 會在 exit 1 時把活著的 agent 殺掉重來，切斷其他 shell 的連線。
- 另一個坑（t7 之前卡了一輪）：若 symlink 還在、而且它指到的 launchd agent 活著，新 `.zshrc` 的 `exit 2` 判斷永遠不成立，舊 symlink 會一直留著；要手動 `rm -f` 一次。
- `.zshrc` 不是 daemon 管理員：靠「第一個碰巧啟動的 shell」帶起 agent，在「開了 IDE 卻沒開 Terminal」的情境會失效。正統做法是 LaunchAgent（`~/Library/LaunchAgents/*.plist`，`RunAtLoad` + `KeepAlive`）在登入時起 agent 並 `--apple-load-keychain`，`.zshrc` 只剩 `export SSH_AUTH_SOCK`。這次先用 `.zshrc` 版把整條鏈驗通，再搬。

## 6. sandbox 的網路只有一個洞：proxy，以及 ssh 怎麼走它

**核心規則**：Seatbelt profile 裡 `network-outbound` 只允許連到本機一個 port（note 05 §3 範例的 `(allow network-outbound (remote ip "localhost:7777"))` 就是它），那是 Claude Code 自己起的 HTTP 過濾 proxy；`allowedDomains` 白名單由 proxy 執行，不是 kernel。所以 sandbox 內**沒有「直連」這個選項**，只有「認不認 `HTTPS_PROXY`」的差別。

**關鍵細節**

| 程式 | 認 `HTTPS_PROXY` 嗎 | sandbox 內 | 原因 |
|---|---|---|---|
| `curl`、`wget`、`git` 走 `https://`、`pip`、`npm`、`brew` | 認 | 通 | 請求自動包給 proxy |
| Python `requests`、Node `fetch` | 認（預設讀環境變數） | 通 | 同上 |
| `ssh`、`git` 走 `git@…` | **不認** | 擋 | 直接 `connect(AF_INET, github.com:22)`，沒路；連 DNS 都不給（`getaddrinfo` 走 UDP 53，也是 `network-outbound`） |
| `psql`、`redis-cli`、`mysql` | 不認 | 擋 | 原生 TCP |
| `ping`、`dig` | 不認 | 擋 | ICMP / 直接 DNS |

- proxy 支援 HTTP `CONNECT host:port`，而且對 22 與 443 都放行（t4 實測拿到 SSH banner）。CONNECT 建立後 proxy 變成一條純 TCP 管子，上面跑什麼協定它不管——這就是「HTTP proxy 可以載 SSH」的原因。
- ssh 要走這條管子得靠 `ProxyCommand`：ssh 不自己開 TCP，改 spawn 一個子程序，把它的 stdin / stdout 當成連線。子程序負責對 proxy 發 CONNECT、帶 `Proxy-Authorization: Basic …`、等 `200`，之後雙向轉送 bytes。
- 現成工具都不合用：macOS 內建 `nc -X connect` 支援 CONNECT 但 proxy 密碼只能互動輸入（ProxyCommand 沒有 TTY）；`corkscrew` / `socat` / `ncat` 沒裝，且 proxy 的帳密與 port **每個 session 隨機**，寫死在參數裡會過期。最後寫了一支約 60 行、只用標準函式庫的 Python，執行時讀 `$HTTPS_PROXY`：[`tools/connect-proxy.py`](../tools/connect-proxy.py)，檔頭的 docstring 把情境、為什麼自己寫、它不做什麼寫在一起，可直接拿去用。
- 只在 sandbox 內啟用、平常終端機不變，用 `Match … exec`：

```
Match host github.com exec "test -n \"$HTTPS_PROXY\""
    ProxyCommand python3 -I /path/to/tools/connect-proxy.py %h %p
```

腳本放在 repo 外的固定位置即可（ssh config 要絕對路徑）；`-I` 讓 Python 忽略使用者 site-packages 與 `PYTHON*` 環境變數，ProxyCommand 這種被 ssh 隱式呼叫的東西少吃一點環境比較安全。`ssh -G github.com | grep proxycommand` 在外面印 `none`、在 sandbox 內印腳本路徑，兩邊各驗一次就知道開關方向對不對。`%h %p` 是 ssh 的代換符，所以同一段 config 可以列多個主機（`Match host github.com,other-host.example`），腳本不綁任何主機。

- 這段最早用 `ssh -o ProxyCommand=…` 直接在命令列測時，被 Claude Code 的 auto-mode 分類器以「Containment Escape」為由拒絕執行；改寫進 `~/.ssh/config`、由 ssh 自己讀取後，同一條連線沒被攔。這是 note 05 §9 的第二層（harness 的 permission 規則）在動作，不是 kernel；kernel 層從頭到尾只看 syscall。

**一次 `git push` 的完整流程：誰 spawn 誰、哪一步是 HTTP、哪一步是 SSH**

```
t0  zsh (sandbox)     git push origin main
t1  git               看 remote URL = git@github.com:…  → 決定用 ssh 當傳輸層
                      spawn: ssh git@github.com "git-receive-pack '…/repo.git'"
t2  ssh               讀 ~/.ssh/config → Match host github.com 成立（HTTPS_PROXY 存在）
                      → 不自己 connect()，改 spawn: python3 connect-proxy.py github.com 22
                        並把自己的 stdin / stdout 接到這個子程序
t3  connect-proxy.py  讀 env HTTPS_PROXY → connect(127.0.0.1:<port>)        ← Seatbelt 允許的唯一 TCP 出口
                      送一行 HTTP：CONNECT github.com:22 + Proxy-Authorization: Basic …
t4  proxy (本機)       驗帳密、查 allowedDomains → connect(github.com:22) → 回 "200 Connection established"
                      ─── 從這裡起 proxy 不再解讀任何內容，只搬 bytes ───
t5  connect-proxy.py  收到 200 → 進入純轉送迴圈（stdin → socket、socket → stdout）
t6  ssh ◀──────────▶ github sshd      SSH 協定：banner、金鑰交換、建立加密通道
t7  ssh → ssh-agent   connect(AF_UNIX ~/.ssh/agent.sock)                      ← Seatbelt 的 unix-socket 白名單（§4）
                      請 agent 簽 server 的 challenge → 拿回簽名 → publickey 認證通過（§3）
t8  git ◀──────────▶ git-receive-pack   在 SSH 加密通道裡跑 git 的 pack 協定（送 objects、更新 ref）
t9  結束              git-receive-pack 回報 → ssh 關連線 → 腳本 stdin EOF → shutdown → proxy 關 TCP
```

- 腳本的全部工作是 t3–t5：用**一行明文 HTTP**向本機 proxy 要一條到 `github.com:22` 的 TCP 管子，之後就只搬 bytes。它不實作 SSH、不實作 git、也沒有任何 HTTPS。
- `Proxy-Authorization` 是給**中間的 proxy** 看的 header（沒帶會回 `407 Proxy Authentication Required`），與給目的地看的 `Authorization`（沒帶回 `401`）是兩個 header；GitHub 永遠不會看到它，在 proxy 那一站就被拆掉。`Basic` 只是 `user:pass` 的 base64，是編碼不是加密，這一跳到 `127.0.0.1` 不出機器所以能接受。
- 帳密不是從 OS 的金鑰庫拿的，是 URL 的 userinfo 欄位（RFC 3986 的 `http://user:pass@host:port`），`urlparse` 原生拆得出來。腳本拿得到這個環境變數，是因為 env 跟著 fork / exec 的血緣一路下來：Claude Code 起 proxy、產隨機帳密、設進子程序 env → `sandbox-exec` → zsh → git → ssh → python（note 05 §5、§11）。帳密的用途是讓「只有這棵 process tree 的子孫」用得了 proxy；每 session 隨機讓洩漏只影響當下。同 uid 的其他程序用 `ps -E` 仍看得到，所以它是 session 級的臨時憑證，不是 secret。
- 三層協定各自獨立，不要混：

```
最外層  腳本 ↔ proxy              HTTP（只有 t3 那一行 CONNECT，明文，對 localhost）
中間層  ssh ↔ github sshd         SSH（加密由 ssh 做；proxy 與腳本都看不懂內容）
最內層  git ↔ git-receive-pack    git pack 協定（跑在 SSH 的一個 channel 裡）
```

- 對照「它只是透過 HTTPS 出去」這個第一印象：

| 說法 | 判定 | 修正 |
|---|---|---|
| sandbox 內 git 相關指令都走 ssh | 一半對 | 走 ssh 是因為 remote 寫 `git@…`；remote 寫 `https://` 的 repo，git 內建的 HTTP 傳輸層自己認 `HTTPS_PROXY`，不需要這支腳本 |
| 所以才建了這段 proxy | 用詞 | proxy 本來就在（Claude Code 起的）；建的是讓 ssh **走它**的 ProxyCommand |
| 本質是建好 TCP 通道到 22 port | 對 | t3–t5，就是這支腳本的全部 |
| 然後根據指令做不同 HTTPS 應用 | 不對 | 通道裡是 SSH（t6–t8）；push / fetch 是 SSH 連線上的遠端指令 `git-receive-pack` / `git-upload-pack`，不是 HTTP 請求。整條鏈裡沒有 HTTPS，它只是這個 proxy 平常載的東西 |

**config 的兩種段落：`Match … exec` 管路，`Host` 管身分**（事件二 t16、t20）

事件二的第一個疑問是「`Match host` 明明列了這個主機，怎麼沒用」。它有用，而且工作已經做完了：隧道通、host key 也比對過，死的是下一個階段。兩種段落管的是不同的關：

| | `Match host … exec "test -n \"$HTTPS_PROXY\""` 段 | `Host <主機>` 段 |
|---|---|---|
| 管什麼 | 連線怎麼出去：`ProxyCommand` | 用哪把 key、要不要順手進 agent、實際連哪個 hostname |
| 在哪生效 | 只在 sandbox（`exec` 條件成立時） | 終端機與 sandbox 都套 |
| 對應哪一關 | ① | ③，以及 `HostName` 轉向 |
| 放錯段的後果 | 拿掉 `exec` 條件 → 終端機也跑 ProxyCommand，沒有 `HTTPS_PROXY`，平常的 git 直接壞掉 | 把 `AddKeysToAgent` 寫進 Match 段 → 只在 sandbox 生效，而 sandbox 讀不到 key 檔，等於白寫 |

- 兩種段落會疊加：同一個主機先吃 `Host` 段的身分設定，再吃 `Match` 段的 `ProxyCommand`。`ssh -G <host>` 在 sandbox 內外各跑一次，看得到實際合成的結果。
- `Match` 段一次列了多個主機，所以不能把某一台的 `IdentityFile` 塞進去，否則其他台也會先被遞這把 key。
- 主機換 SSH hostname（t20）：伺服器端改成只在新 hostname 收 SSH。不必去改每個 repo 與 submodule 的 remote URL，在 config 做一次轉向即可，`ProxyCommand` 的 `%h` 會跟著代換成新 hostname：

```
Host other-host.example
    HostName ssh.other-host.example

Match host github.com,other-host.example,*.other-host.example exec "test -n \"$HTTPS_PROXY\""
    ProxyCommand python3 -I /path/to/tools/connect-proxy.py %h %p
```

- 新 hostname 的 host key 要先進 `known_hosts`，而且**只能在 sandbox 外做**：`known_hosts` 在 sandbox 內可讀不可寫，第一次連會印 `Failed to add the host to the list of known hosts`。在終端機連一次收下（trust on first use），要嚴謹就對照服務商公告的 fingerprint。事件一的 t10 是同一件事的另一個症狀。

> **補強**：`man ssh_config` 寫明 `Match host` 比對的是**經 `HostName` 代換之後**的目標主機名（要比對命令列上原本打的名字得用 `originalhost`）。所以轉向的 `Host` 段要放在 `Match` 段前面，`Match` 的清單也要涵蓋新 hostname。這次的 config 新舊兩個名字都列了，所以沒有單獨驗證「只列舊名字」會不會漏掉。

## 7. 事件解剖：三關各死在哪、各怎麼過

| 步 | 症狀 | 哪一關 | 哪一層擋的 | 怎麼過 |
|---|---|---|---|---|
| t0–t1 | `unable to access ~/.gitconfig` / `~/.gitconfig-rs` | ②（git 的，不是 ssh 的） | kernel `file-read*`：`$HOME` deny | 兩個檔加進 `allowRead`；`includeIf` 拉進來的檔也要列 |
| t2 | `Could not resolve hostname github.com` | ① | kernel `network-outbound`：沒有 proxy 以外的路 | §6 ProxyCommand |
| t3 | `ls ~/.ssh` EPERM | ② | kernel `file-read*` | 只放 `known_hosts`、`config`、`agent.sock` 三個，key 檔不放 |
| t3 | `Error connecting to agent` | ③ | kernel `network-outbound (unix-socket)`：`allowUnixSockets` 空 | §4–§5 固定路徑的真實 socket |
| t5 | 加了 symlink 路徑與 glob 仍 EPERM | ③ | kernel：比對解析後路徑、glob 不生效 | 自起 agent |
| t6 | `readlink agent.sock` EPERM | ② | kernel `file-read*`：symlink 本身也是檔案 | `allowRead` 加它 |
| t8 | agent 只有一把內部伺服器的 key，GitHub 用的那把不在 | ③（認證層，sandbox 無關） | — | `ssh-add --apple-use-keychain <github key>` 一次（當時漏了前提：key 要有 passphrase 才會進鑰匙圈，§3.4） |
| t9 | 直接 `-o ProxyCommand` 被拒 | — | **harness** 分類器 | 寫進 `~/.ssh/config` |
| t10 | 第二個主機 `Host key verification failed` | ②（內容，不是權限） | — | 在 sandbox 外先連一次讓它寫 `known_hosts` |
| t11 | sandbox `Permission denied (publickey)`，`ssh-add -l` 少一把 | ③ | 認證層：agent 重啟後那把 key 沒回來；sandbox 沒有讀檔這條退路 | §3.3–§3.4 |
| t12–t13 | 終端機也 `Permission denied`，fingerprint 對不上 | ③（sandbox 無關） | 伺服器：登記的公鑰配不到本機任何私鑰 | 把現在的公鑰登記上去 |
| t14 | 終端機 ✅、sandbox ❌ | ③ | kernel `file-read*`：key 檔不可讀，只剩 agent 一條路 | `ssh-add` 把 key 放進 agent |
| t16 | 加 `IdentitiesOnly yes` 後 `no such identity … Operation not permitted` | ③ | kernel `file-read*`（讀不到 identity 檔）＋ ssh 自己的過濾（agent 的 key 對不回指名的檔，全部略過） | 不設 `IdentitiesOnly` |
| t17–t19 | 臨時 agent 載不回那把 key | ③（重開機後才發作） | 鑰匙圈：沒有那筆項目，因為 key 沒 passphrase、`--apple-use-keychain` 沒東西可存 | key 設 passphrase 後再存一次 |
| t20 | 舊 hostname 不收 SSH；新 hostname `Failed to add the host to the list of known hosts` | ①（伺服器端）、②（寫入） | 伺服器政策；kernel `file-write*`：`known_hosts` 在 sandbox 內只讀 | `HostName` 轉向；終端機先連一次 |
| t21 | 鑰匙圈那筆 passphrase 哪裡都看不到 | — | 鑰匙圈的儲存位置（§3.6 推論） | 用臨時 agent 當探針 |

- 事件二沒有任何一步是 §4–§6 那些機制壞掉：socket 連得到、隧道通、config 讀得到。它全部發生在第 ③ 關內部，而且根因在 sandbox 外（agent 裡有什麼、鑰匙圈記得什麼、伺服器認哪把）；sandbox 只是把「終端機有退路所以一直沒發現」的問題顯影出來。
- t0 與 t2 之間還有一段只有 read-only git 能用的過渡期：`GIT_CONFIG_GLOBAL=/dev/null` 跳過全域設定，`status` / `log` / `diff` 都能跑，但 commit 會帶錯 identity（includeIf 的檔沒讀到）。這是「繞過」不是「解決」。
- 三關的錯誤訊息都不說「sandbox」：DNS 說解不到主機、ls 說不允許、ssh-add 說連不到 agent。判斷是不是 sandbox 擋的，看 errno 是 `EPERM`（MAC）還是 `EACCES`（DAC）／`ENOENT`（真的沒有），note 05 §8。
- 事件二同一句 `Permission denied (publickey)` 出現了三次，成因各不相同：t11 是 agent 裡沒有、t12 是伺服器不認、t16 是 `IdentitiesOnly` 把 agent 的 key 濾掉。分辨方式是換一個變因重測：換到終端機（排除 sandbox）、指定單一把 key（排除挑 key 順序）、對 fingerprint（排除伺服器端）。

## 8. 解法對照

| 解法 | 改哪裡 | 過哪幾關 | 代價 |
|---|---|---|---|
| A. SSH + 固定路徑 agent + ProxyCommand（本篇採用） | `.zshrc`、`~/.ssh/config`、sandbox 的 `allowUnixSockets` / `allowRead`、一支 ProxyCommand 腳本 | ①②③ 全過，fetch / push 都在 sandbox 內 | 自維護腳本；agent 對 sandbox 內所有程式可用（簽名，非 key） |
| B. remote 改 `https://` + token | remote URL、credential helper 存 token | ①② 原生就過（git 認 proxy）；③ 換成 token | token 要管；keychain 能否從 sandbox 內讀未驗證 |
| C. push 排除在 sandbox 外 | `sandbox.excludedCommands: ["git push"]` 或每次 `dangerouslyDisableSandbox` | 不過，直接繞 | 最省事；key 與 agent 完全不進 sandbox；push 以外的 fetch / pull 若走 SSH 也要一起排除 |
| D. fetch 走 HTTPS、push 走 SSH 分流 | `git remote set-url` + `set-url --push` | 讀取類在 sandbox 內過；push 走 C | 只對能匿名 HTTPS 讀的 repo 有效（公開 repo） |

- A 是唯一「SSH 在 sandbox 內全通」的路，代價都集中在 §3 提的信任邊界與 §6 的腳本。
- 對只想把 `fetch` / `pull` / `status` 留在 sandbox、`push` 本來就要人工確認的工作流，C 或 D 更划算；這次選 A 是為了把三關各自看清楚，不是因為它最省。

**事件二：選了 A 之後，怎麼讓 key 在 agent 重啟後自己回來**

| 解法 | 改哪裡 | 重開機後 | 代價 |
|---|---|---|---|
| a. key 設 passphrase，`ssh-add --apple-use-keychain` 存一次（本篇採用） | 私鑰檔的加密層、鑰匙圈 | `.zshrc` 的 `--apple-load-keychain` 開 shell 就載回，不問密碼 | passphrase 要另外備份（鑰匙圈不給看）；順手補上「私鑰檔沒加密」這個洞 |
| b. `Host` 段加 `AddKeysToAgent yes` + `UseKeychain yes` | `~/.ssh/config` | 要先在終端機連過一次才會進 agent | 當天第一個 git 動作若在 sandbox 就失敗；不能同時設 `IdentitiesOnly` |
| c. `.zshrc` 多一行 `ssh-add -q <key>` | `.zshrc` | 開 shell 就載 | 為一把 key 開特例，「哪些 key 要載」散在兩個地方；key 維持沒 passphrase |
| d. `Host *` 全域 `AddKeysToAgent yes` | `~/.ssh/config` | 任何 key 在終端機用過就進 agent | 所有用過的 key 都留在 agent 到重啟，sandbox 內可簽的身分變多；缺口同 b |

- a 把這把 key 併進既有的那套機制（鑰匙圈記、開 shell 預載），不需要為它在 config 或 `.zshrc` 多寫任何東西；驗收方式是臨時 agent 列出它（t19）。
- b 與 d 靠的是懶載入，觸發點在終端機，所以補不了「重開機後直接從 sandbox 開工」這個情境。
- 採用 a 之後，config 裡為了載 key 而寫的 `Host` 段就可以整段拿掉，只留真正需要的設定（`HostName` 轉向、別名主機的 `User` / `IdentityFile`）。刪的時候要分清一個 `Host` 段裡哪些行是「載 key」、哪些是「怎麼連」：這次一度把另一台主機的整段一起刪了，連 `HostName` 都不見，得再加回去。

## 9. 陷阱清單

1. ❌【模型】sandbox 內 push 不行是因為讀不到 key 檔 → ✅ 是三關：網路、設定檔、簽名；key 檔只是第三關的其中一條路，而且是設計上要封死的那條 → 為什麼：三個 syscall 各自過 MAC；最先倒的（DNS）會遮住後面的，要分開測。
2. ❌【模型】`curl` 連得到 GitHub，所以 ssh 也連得到 → ✅ 通的是 HTTPS 經 proxy；sandbox 沒有直連，ssh 不認 `HTTPS_PROXY` → 為什麼：`network-outbound` 只開本機 proxy 一個 port；誰認 proxy 環境變數誰才有路。
3. ❌【模型】ssh-agent 每次開機都要重新手動 `ssh-add` → ✅ agent 記憶體每次歸零是真的，但 passphrase 可存 keychain（`--apple-use-keychain` 一次），開機後 `--apple-load-keychain` 自動載回、不問密碼 → 為什麼：「加密的 key 檔」「passphrase」「解開的 key」三樣東西住三個地方、三種壽命。
4. ❌【模型】`--apple-use-keychain` 就是「每次啟動自動載入」 → ✅ 它只存 passphrase；自動載入是 `--apple-load-keychain`（預載）或 `AddKeysToAgent` + `UseKeychain`（懶載入）→ 為什麼：存與載是兩個動作；懶載入要 client 讀 key 檔，在 sandbox 內走不了。
5. ❌【模型】把 launchd 的 socket symlink 到固定路徑，白名單寫固定路徑即可 → ✅ MAC 比對的是 `namei` 解析後的真實路徑；symlink 只是指路牌 → 為什麼：`connect(AF_UNIX)` 跟 `open` 一樣先走 VFS 解路徑，MAC hook 在之後。
6. ❌【模型】`allowUnixSockets` 寫 `/var/run/com.apple.launchd.*/Listeners` 就能涵蓋隨機目錄 → ✅ 不支援 glob，只比字面；且 `/var` 是 symlink，真身是 `/private/var` → 為什麼：三種寫法全沒中、字面真實 socket 一次就中（§4 推論：harness 翻成 `literal` 而非 `regex`）。
7. ❌【模型】`/private/var/run` symlink 到 `/var/run` → ✅ 方向相反：`/var → /private/var`，整個 `/var` 是 symlink，`/private/var/run` 才是真身 → 為什麼：macOS 從 NeXTSTEP 留下的佈局，`/etc` `/var` `/tmp` 都是根目錄的 symlink 指進 `/private`。
8. ❌【模型】`/private/tmp` 下沒有 `com.apple.launchd.*` 表示系統沒跑 ssh-agent → ✅ 這版 macOS 放在 `/private/var/run`；`launchctl getenv SSH_AUTH_SOCK` 才是真相來源 → 為什麼：從「可清的暫存」搬到「這次開機的執行期狀態」，分類變了、路徑跟著變。
9. ❌【模型】`if ! ssh-add -l` 就重起 agent → ✅ 只在 exit 2（連不上）重起；exit 1 是 agent 活著但空的，重起會切斷別的 shell → 為什麼：`ssh-add -l` 有三個 exit code，0 / 1 / 2 各是「有 key / 沒 key / 沒 agent」。
10. ❌【模型】symlink 版與自起版都是 per-user，差別在「for 任何 user」 → ✅ 兩版都用 `$HOME`，都 per-user；真正差別是「重用系統 agent」vs「多一個 agent」，影響 GUI 程式看不看得到 key → 為什麼：從 Dock 開的程式繼承 launchd 的 `SSH_AUTH_SOCK`，不經 `.zshrc`。
11. ❌【模型】為 SSH 開 proxy 隧道是多此一舉，應該直接走 public → ✅ sandbox 內沒有 public 這條路；要直連等於關掉網路 sandbox → 為什麼：白名單是 proxy 執行的，kernel 層只有「到 proxy」一條 `allow`。
12. ❌【模型】被攔下的那次 `ssh -o ProxyCommand=…` 是 kernel 擋的 → ✅ 是 harness 的 auto-mode 分類器，在指令執行前就拒絕；kernel 層沒看到那次 connect → 為什麼：note 05 §9 的兩層，第二層決定「要不要跑這個 tool call」，第一層才是 syscall 過濾。
13. ❌【用詞】「ssh-agent 讓 sandbox 讀不到 pem」 → ✅ sandbox 本來就讀不到 pem（`file-read*` deny）；agent 是讓「讀不到 pem 的程式」仍能完成認證 → 為什麼：agent 不是擋讀取的機制，是把「需要 key」改成「需要簽名」的代理。
14. ❌【模型】ProxyCommand 腳本「建好 TCP 通道後，根據指令做不同 HTTPS 應用」 → ✅ 通道建好後跑的是 SSH，git 再跑在 SSH 裡；整條鏈沒有 HTTPS，腳本只在 t3 發過一行明文 HTTP `CONNECT` → 為什麼：`CONNECT` 的設計就是「要一條 TCP、之後別管內容」，載什麼協定由兩端決定；「HTTPS」只是這個 proxy 平常載的東西，不是這次載的東西。
15. ❌【模型】`Proxy-Authorization` 的帳密是「從 OS 拿到的 token」 → ✅ 是 `HTTPS_PROXY` 這個 URL 的 userinfo 欄位，由環境變數沿 fork / exec 血緣繼承到腳本；`Basic` 是 base64 編碼不是加密 → 為什麼：Claude Code 每 session 起 proxy 時產隨機帳密、塞進子程序的 env，讓只有這棵 process tree 用得了 proxy；同 uid 其他程序 `ps -E` 仍看得到，所以它是 session 級臨時憑證，不是 secret。

事件二（2026-11-12）撞到的：

16. ❌【模型】config 的 `Match host` 有列這個主機，sandbox 內就應該連得上 → ✅ Match 段只有 `ProxyCommand`，只管第 ① 關，而且那一關已經過了；`Permission denied (publickey)` 是第 ③ 關 → 為什麼：隧道通、host key 比對過之後才輪到認證；「用哪把 key」要寫在終端機也會套用的 `Host` 段，或根本不用寫（交給 agent）。
17. ❌【模型】終端機的 git 能用，代表 agent 裡有那把 key → ✅ 終端機的 ssh 有兩條路：問 agent，或自己讀預設檔名的 key 檔；agent 沒有時它退到讀檔，所以無感。sandbox 只有問 agent 一條 → 為什麼：`IdentityFile` 有預設值，而 `AddKeysToAgent` 預設 `no`，用檔案認證過不會順手進 agent；sandbox 的 `file-read*` 把讀檔那條封死。
18. ❌【模型】`ssh-add -l` 列得出來，重開機後也會在 → ✅ `-l` 問的是這個 agent 此刻的記憶體；重開機後誰回來要用臨時 agent 問：`ssh-agent sh -c "ssh-add -q --apple-load-keychain; ssh-add -l"` → 為什麼：解開的 key 住在 process 的記憶體，鑰匙圈記的才是「下次要載誰」。
19. ❌【模型】`.zshrc` 有 `ssh-add --apple-load-keychain`，`~/.ssh` 底下的 key 就都會自動載回 → ✅ 它不掃目錄，只把鑰匙圈裡有項目的 key 載回來 → 為什麼：預載的清單來源是鑰匙圈；沒進過鑰匙圈的 key，它不知道存在。
20. ❌【模型】`ssh-add --apple-use-keychain <key>` 做過一次，這把 key 就進鑰匙圈了 → ✅ 前提是 key 有 passphrase；沒有的話這次只進了 agent 的記憶體。看它有沒有問 `Enter passphrase for …` 就知道 → 為什麼：鑰匙圈存的是 passphrase，沒 passphrase 就沒有東西可存。事件一的 t8 就踩在這裡，只是到重開機才發作。
21. ❌【模型】關掉 terminal 再開一個，agent 就是新的 → ✅ agent 是獨立的 process，壽命到它自己結束（重開機或被 kill）；`.zshrc` 是「連得上就沿用」→ 為什麼：agent 不是 shell 的一部分，新 shell 只是拿同一個 `SSH_AUTH_SOCK` 連回去。
22. ❌【用詞】設 passphrase 用 `ssh-add -p` → ✅ 是 `ssh-keygen -p -f <key>`；`ssh-add` 沒有 `-p`。而且新 passphrase 直接 Enter 等於設成空的，訊息照印 `saved with the new passphrase` → 為什麼：`ssh-add` 管的是 agent 裡有誰，`ssh-keygen` 管的是 key 檔本身；設完用 `ssh-keygen -y -P "" -f <key>` 驗，還印得出公鑰就是空的。
23. ❌【模型】passphrase 是鑰匙圈裡那筆項目的 label → ✅ label 是 key 的路徑，passphrase 是那筆的 secret；而 passphrase 本身是私鑰檔內容的加密密碼 → 為什麼：預載要靠 label 找到檔案、靠 secret 解開檔案，兩個欄位各有用途。
24. ❌【模型】私鑰檔 `cat` 得出來，表示 passphrase 沒在保護它 → ✅ `cat` 看到的是信封與 base64；有 passphrase 時裡面的私鑰區是密文。能不能讀由檔案權限與 sandbox 決定，讀到的能不能用由 passphrase 決定 → 為什麼：兩層保護方向不同，一個防「讀不到」，一個防「讀到了也沒用」；公鑰則本來就是明文。
25. ❌【模型】鑰匙圈裡的 SSH passphrase 可以打開「鑰匙圈存取」查到 → ✅ 這版 macOS 上 GUI 與 `security` 指令都列不出來；它只替你用，不給你看 → 為什麼：存放位置不是傳統的檔案型鑰匙圈（§3.6 推論）。後果是 passphrase 忘了就查不回來，要另外存進密碼管理工具。
26. ❌【模型】ssh-agent 是一種 ssh proxy → ✅ agent 站在 client 旁邊，只在認證那一刻回一個簽名；連線的 bytes 不經過它。站在路徑上的是 `ProxyCommand` 與 proxy → 為什麼：少了 agent 的錯誤是 `Permission denied (publickey)`，少了 proxy 的錯誤是 `Could not resolve hostname`，分屬第 ③ 關與第 ① 關。
27. ❌【模型】`ssh-keygen` 是產 key 的，`-y -P ""` 等於拿它來開鎖 → ✅ `ssh-keygen` 是 key 的工具箱（產生、改 passphrase、印公鑰、算 fingerprint）；`-y -P ""` 只是拿空 passphrase 試一次，開得了的只有本來就沒鎖的檔 → 為什麼：它不繞過加密；呼叫的人拿到的只有 exit code 這 1 bit，key 的內容沒有流出來。
28. ❌【模型】在 `Host` 段加 `IdentitiesOnly yes` 比較嚴謹，沒有副作用 → ✅ sandbox 內它會讓 agent 裡同一把 key 也被略過：`no such identity: …: Operation not permitted` 之後就是 `Permission denied` → 為什麼：ssh 要讀得到指名的 identity 檔才能把它對回 agent 的清單，而 sandbox 內讀不到。
29. ❌【模型】鑰匙圈只記得有 passphrase 的 key → ✅ 決定因素是鑰匙圈裡有沒有那把 key 的項目；盤點時就有一把檔案沒加密、卻照樣被載回的 key → 為什麼：這句話是從 t17 一次觀察歸納的，t22 出現反例；「沒 passphrase 的 key 現在加不進鑰匙圈」成立，「鑰匙圈裡的 key 一定有 passphrase」不成立。
30. ❌【模型】同一句 `Permission denied (publickey)` 就是同一個原因 → ✅ 這次出現三種：agent 裡沒有那把（t11）、伺服器登記的公鑰與本機私鑰不是同一把（t12）、`IdentitiesOnly` 把 agent 的 key 濾掉（t16）→ 為什麼：這句話只說「遞上來的公鑰沒有一把被接受」，不說是沒遞、遞錯、還是伺服器不認；要換變因重測才分得出來。
