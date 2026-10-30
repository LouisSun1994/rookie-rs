# Note 3 — 從應用程式到 GPU：引擎、SDK、driver、kernel 的分層

> 這份筆記記錄的是「一個語言要怎麼用到 GPU」這條線：從 PyTorch / Candle 這層引擎，往下穿過廠商 SDK、user-mode driver、`ioctl()` 邊界、kernel-mode driver，到硬體與 DMA；再拿儲存路（NVMe）當對照組。
> 核心收穫不是 CUDA API，而是**每一層是誰寫的、跑在哪個 mode、誰搬資料、誰通知誰**——以及 Rust 在這條鏈上實際只佔哪一層。
>
> 記錄日期：2026-10-30｜當時定位：🥚 小白（仍未動手寫程式），這條線概念上碰到 🐊 的 FFI / C ABI、driver 與 DMA。
> 標「補強」的 blockquote 是整理進 repo 時加註的延伸，不在原本這條線的對話裡。

---

## 0. 一句話版本

> **任何語言要用 GPU，中間一定有一層「引擎」（libtorch / Candle）負責排運算和管記憶體；引擎往下呼叫廠商的 C 函式庫，再進 user-mode driver，透過 `ioctl()` 跨過使用者／核心邊界進 kernel-mode driver，最後才碰硬體。**
> 「kernel」在這條鏈上有三個互不相干的意思；driver 不是 OS kernel 的一部分，是硬體廠寫好插進去的模組。

---

## 1. 分層模型（骨架）

```
層級                 NVIDIA 獨顯                    Intel iGPU                        Rust / 語言歸屬
──────────────────────────────────────────────────────────────────────────────────────────────────────
你的程式             Python / C++                   同左                               你的 crate
引擎                 libtorch（= PyTorch，C++）      libtorch XPU 後端 / OpenVINO       Candle、Burn（Rust 自刻）
                                                                                       tch-rs（綁 libtorch，仍是 C++ 引擎）
  ─── C ABI（FFI）────────────────────────────────────────────────────────────────── extern "C"、cudarc crate
廠商 SDK             cuBLAS、cuDNN、libcudart.so    oneDNN、oneMKL                     C/C++；NVIDIA 閉源
user-mode driver     libcuda.so                     Level Zero（libze_intel_gpu.so）    C/C++；跟 kernel driver 同套出貨
  ═══ 系統呼叫邊界：ioctl() 對 /dev/nvidia0 或 /dev/dri/renderD128 ═════════════════ std → libc → syscall
kernel-mode driver   nvidia.ko（+ uvm、modeset）     xe.ko / i915.ko                    C；Linux 6.1+ 允許 Rust，Nova 用 Rust
硬體                 GPU → VRAM                     Xe GPU → DRAM（統一記憶體）
GPU 上跑的程式       GPU kernel（CUDA C++ → PTX）    SYCL / OpenCL kernel               rust-gpu、cuda_std 仍實驗性
```

- 每一個「↓」都是一個 ABI（契約），ABI 本身不是一層。
- 對應用程式而言，責任止步於 invoke 引擎的 API；引擎止步於 invoke CUDA API；一層藏一層。
- 整條鏈只有最上面兩層可能是 Rust，中間是 C ABI 的邊界，往下全是 C/C++。

---

## 2. 引擎層：PyTorch 就是 libtorch

**核心規則**：PyTorch 的本體是 C++ 函式庫 libtorch，Python 只是前端外殼。tensor、autograd、算子排程、記憶體配置器全在 libtorch 裡。

**關鍵細節**
- `pip install torch` 裝進來的是幾百 MB 的 `.so`（libtorch）加一層薄薄的 Python 包裝。`torch.matmul(a, b)` 在 Python 裡做的事只有把指標傳進 C++ 然後等結果。
- 官方直接發行 libtorch 的 C++ 版：`#include <torch/torch.h>` 跑的是同一份二進位，不需要 Python 直譯器。
- 歷史：前身 Torch 是 Lua 前端 + C/C++ 底層，Facebook 把前端換成 Python 才叫 PyTorch。「Py」指前端換了，引擎從來都是 C/C++。
- 類比：SQLite、NumPy——核心是 C，各語言各包一層；沒人會說「SQLite 是給 Python 用的」。
- 「框架」與「推論引擎」：PyTorch 能訓練也能推論叫框架；llama.cpp / vLLM / OpenVINO 只推論叫引擎。在「誰管記憶體、誰知道下一步算什麼」這個角色上一模一樣。
- 為什麼一定要有這層：沒有人直接對 driver 寫程式跑模型。讀權重、排 tensor、決定每層算什麼、呼叫 GPU kernel——這些資訊只有引擎有，driver 和 GPU 都不知道。

**對照表：其他語言的兩條路**

| 路 | 做法 | 例子 | 底下有沒有 libtorch |
|---|---|---|---|
| A 綁 libtorch | 包一層外殼呼叫 libtorch 的 C++ 函式（做 Python 外殼做的事） | tch-rs（Rust）、DJL（Java） | 有 |
| B 自刻引擎 | tensor／算子／記憶體管理用自己的語言重寫，直接呼叫 cuBLAS | Candle、Burn（Rust） | 無 |

**對照表：Rust 生態**

| crate | 定位 | 底層 |
|---|---|---|
| Candle（Hugging Face） | 最接近「Rust 版 PyTorch」，Rust 跑 LLM 最常見 | 自刻；接 cuBLAS / Metal / CPU |
| Burn | 偏訓練、多後端抽象 | CUDA、wgpu、甚至 libtorch |
| mistral.rs | 建在 Candle 上的純推論引擎（定位近 vLLM / Ollama） | Candle |
| tch-rs | libtorch 的 Rust binding | libtorch（C++） |
| llama-cpp-rs 等 | llama.cpp 的 binding | llama.cpp（C++） |

> **補強**：vLLM 不是跟 libtorch 平行的另一個引擎——它建在 PyTorch 上（`pip install vllm` 會把 torch 拉進來），自己管的是 KV cache（PagedAttention）和請求排程，tensor 運算仍交給 libtorch 和它自帶的 CUDA kernel。定位跟 mistral.rs 之於 Candle 一樣是「引擎上的引擎」；上面那串「只推論的引擎」裡真正自帶 tensor 層的是 llama.cpp 和 OpenVINO。

---

## 3. 為什麼鏈的下半段全是 C

**核心規則**：不是因為「driver 是 C++」，是因為 GPU 廠商只開 C ABI 這一種門。

**關鍵細節**
- cuBLAS、CUDA runtime、libcuda.so 是 NVIDIA 給的閉源 `.so`，對外只暴露 C 函式簽名；Intel Level Zero / oneDNN、AMD ROCm 開源，但也是 C/C++。
- Rust 接法：Candle 透過 `cudarc` crate 做 FFI，本質是 `extern "C"` 宣告一堆 NVIDIA 的函式直接呼叫。Python、Go 走的是同一道門。
- 連引擎要送上 GPU 的程式也不是 Rust：Candle 的 matmul / softmax / attention 是用 CUDA C++、Metal Shading Language 寫的，獨立編成 PTX / metallib，Rust 只負責載上去呼叫。
- kernel driver：NVIDIA 開源核心模組、Intel xe 都是 C。Linux 6.1 起允許 Rust，NVIDIA 新的開源 driver Nova 用 Rust 寫（接替 Nouveau，發展中）。今天實際走的路徑仍是 C。
- Rust 在這條鏈上只佔最上面一層。這是 GPU 生態被 C ABI + 閉源二進位鎖住的結果，不是 Rust 的問題。

---

## 4. Driver：硬體廠寫的、分兩半的軟體

**核心規則**：driver = 讓 OS 和程式能跟某一塊特定硬體講話的軟體，由硬體廠寫，分 user-mode 與 kernel-mode 兩半，同套出貨、版本綁死。它不是 OS 的一部分，是裝進 OS 裡跑的。

**對照表：driver 的兩半**

| | user-mode driver | kernel-mode driver |
|---|---|---|
| 例子 | libcuda.so、libze_intel_gpu.so | nvidia.ko、xe.ko / i915.ko |
| 跑在哪 | 你的 process 裡，使用者態 | OS kernel 裡（`.ko` = kernel object，Linux 核心模組） |
| 做什麼 | 把 API 呼叫翻成 GPU 命令封包、排進命令佇列、JIT 把 PTX 編成該代 GPU 機器碼、管 context | 特權操作：把 GPU 記憶體映射進程式位址空間、設 DMA、接中斷、在多個 process 間切 GPU 並隔離 |
| 為什麼放這邊 | 翻譯不需要特權，放使用者態省 syscall | 做錯會讓整台機器當掉，只能在核心裡做 |
| 碰得到硬體暫存器？ | 只有映射給它的門鈴 | 是，唯一能碰的那層 |

**對照表：SDK 與 driver 是兩包不同的東西**

| | SDK / library | driver |
|---|---|---|
| 例子 | libcudart.so、cuBLAS、cuDNN；oneDNN、oneMKL | libcuda.so + nvidia.ko |
| 誰裝 | 跟 CUDA toolkit / `pip install torch` 一起 | 裝顯卡驅動時 |
| 幾份 | 每個環境可以各帶一份 | 一台機器一份，所有程式共用 |
| 版本跟誰走 | toolkit | 顯卡驅動 |
| 交界露出來的時刻 | `CUDA driver version is insufficient`——上面你能換，下面要裝驅動才能換 | |

**關鍵細節**
- libcudart.so（runtime API）不是 driver，是方便層：管 context、包 `cudaMalloc` 這類好用的函式，底下全部轉呼叫 libcuda.so（driver API）。
- user-mode ↔ kernel-mode 的切換點是 libcuda.so 對 `/dev/nvidia0` 的 `ioctl()`，不是 CUDA API。
- NVIDIA 實際裝的不只一個 `.ko`：nvidia.ko（主體）、nvidia-uvm.ko（統一虛擬記憶體，管 CPU/GPU 分頁遷移）、nvidia-modeset.ko（顯示輸出）。

> **補強**
> - 「同套出貨、版本綁死」是 NVIDIA 的情況：nvidia.ko 的 ioctl 介面是私有的、不保證穩定，所以 libcuda.so 必須跟同版的 `.ko` 配對。Intel 走相反路線——xe.ko / i915.ko 在 Linux 主線，對外是 DRM uAPI（公開、承諾不破壞使用者空間的 ABI），compute-runtime（UMD）獨立發版，新 UMD 配舊 kernel 通常能跑。這是「ABI 是契約」的兩個極端：契約公開 vs 契約私有。
> - libcuda.so 開的裝置節點不只 `/dev/nvidia0`：`/dev/nvidiactl`（控制、建 context）、`/dev/nvidia0..N`（每張卡一個）、`/dev/nvidia-uvm`（統一虛擬記憶體），各自是不同 `.ko` 的入口。

---

## 5. 「kernel」的三個意思

| 意思 | 是什麼 | 例子 |
|---|---|---|
| OS kernel | 作業系統核心：最底層有特權的程式，管 CPU、記憶體、所有硬體存取 | Linux kernel；driver 的 kernel-mode 那半是插進它的模組 |
| GPU kernel（compute kernel） | 一段送到加速器上、由幾千個執行緒同時跑的函式 | CUDA kernel、OpenCL kernel、SYCL kernel、Metal compute kernel、Vulkan / DX compute shader |
| 數學 kernel | 一段核心運算常式 | BLAS kernel、卷積 kernel、Intel MKL（Math Kernel Library） |

- 三者共用同一個字是歷史巧合，彼此無從屬關係；GPU kernel 這個名字是從數學 kernel 借來的（「把這段核心運算丟上去平行跑」）。
- 哪些硬體有 compute kernel：GPU、NPU、TPU、FPGA——會執行你寫的程式的加速器。
- 哪些沒有：SSD、網卡、USB 控制器——固定功能硬體，裡面跑的是廠商燒好的 firmware，你送不了程式進去。
- OS kernel vs driver：kernel 是本體，driver 是硬體廠寫好插進去的模組。沒 driver 的 kernel 不知道怎麼跟那塊硬體講話；沒 kernel 的 driver 沒地方跑。

---

## 6. ABI 是契約，不是層

- ABI = 兩層之間的契約：函式叫什麼名字、參數怎麼傳（calling convention）、記憶體怎麼排。堆疊裡每一個「↓」都是一個 ABI。
- user-mode driver「對外暴露一個 ABI（CUDA driver API）給上層呼叫」——它是元件，ABI 是它的門面。
- Rust 跨到 C 用的就是 C ABI：`extern "C"` 宣告函式、`#[repr(C)]` 讓 struct 排法跟 C 一致。
- 「硬體回呼」是中斷機制（硬體發 interrupt → OS kernel 接住 → 交給 kernel-mode driver），跟 ABI 是兩回事。

---

## 7. 使用者／核心邊界與回程：資料和通知是兩條路

**核心規則**：命令下去大多不需要 syscall；結果資料不經過 CPU，由 GPU / DMA 直接落在記憶體（VRAM 或釘住的 DRAM 分頁）；完成「通知」才走中斷 → OS kernel → kernel-mode driver → fence 旗標。應用程式那側只看旗標，不「解釋反饋」。

**關鍵細節**
- 門鈴優化：GPU 命令佇列是一塊映射進 process 的記憶體（ring buffer）。libcuda.so 直接寫命令進去，再寫一下門鈴暫存器（MMIO）通知 GPU——大部分 kernel 啟動不陷入核心態。需要 `ioctl` 的只有：配記憶體、建 context、同步等中斷。這是 GPU 吞吐能這麼高的原因之一：不是每個運算都經過 OS。
- CPU 不會同步等每個中斷：一串命令丟進佇列，只在需要結果的點等 fence，不然 GPU 會餓死。
- user-mode driver 不是「把指令轉成硬體廠的 binary」：GPU kernel 的機器碼早就編好了（或由它 JIT PTX）；它包的是「跑哪個 kernel、參數在哪、grid 多大」的命令封包。
- 中斷發生時跑的是 kernel-mode driver 的 ISR，在哪個 core 被打斷就在哪個 core 跑；它做的事是讀 fence / 狀態暫存器，然後 `wake_up()` 等在 wait queue 上的 thread。

> **補強**：「寫 ring buffer + 敲門鈴、不進核心」是 NVIDIA（以及 AMD ROCm 的 user-mode AQL queue）的設計，不是所有 GPU 的通則。Intel 在 Linux 上的預設路徑不同：Level Zero 把一串 kernel 收進一個 command list，整包透過一次 DRM exec `ioctl`（i915 `EXECBUFFER2` / xe `EXEC`）交給 kernel driver 提交——syscall 次數被攤平，但不是零；compute-runtime 另有可選的 direct submission 模式才接近門鈴那條路。預測差別：對 iGPU 跑 `strace`，每次 command list 提交都看得到一個 ioctl。

---

## 8. 對照：GPU 路 vs 儲存路

**核心規則**：GPU 沒有業界標準指令介面 → 每家廠商要自己出 user/kernel-mode driver；SSD 有 NVMe 標準 → Linux 的 `nvme.ko` 是社群寫的通用 driver，任何廠的盤插上去都是它服務；應用程式連 NVMe 這個字都不用知道，只呼叫 OS 檔案 API。

```
GPU 路：libtorch → CUDA API → libcuda.so ═ioctl═ nvidia.ko → GPU
        ← 廠商專屬 API 一路到底；程式要知道底下是哪家
SSD 路：程式 → read/write/pread/io_uring ═syscall═ VFS → 檔案系統 → block layer → nvme.ko → SSD
        ← OS 標準 API；程式不知道底下是 NVMe、SATA 還是網路硬碟
```

| | GPU | SSD |
|---|---|---|
| 標準介面 | 無（CUDA / Level Zero / ROCm 各自為政） | NVMe |
| 誰寫 driver | 硬體廠（libcuda.so + nvidia.ko） | Linux 社群（nvme.ko）；廠商差異藏在盤內 firmware |
| 應用程式碰的 API | 廠商 SDK → driver API | OS 檔案 API（`open/read/write/pread`；要快用 `io_uring`、`O_DIRECT`） |
| 使用者空間直驅 | — | SPDK（極端追求延遲的儲存系統在用），一般應用不走 |

**關鍵細節**
- Rust 對應：`std::fs::File` 的 `read` / `read_at` → libc `pread64` → syscall。std 在這裡只是 libc 的薄包裝。
- 資料搬運是 DMA：nvme.ko 設好目的實體頁後，SSD 控制器自己把資料寫進 DRAM，CPU 不搬。
- `mmap`：把檔案映射進位址空間，碰到不在 DRAM 的頁 → page fault → kernel 從 SSD 讀 4KB 回來 → 繼續。被動、顆粒小、kernel 不知道你下一步要什麼（llama.cpp 用 mmap 載權重走的就是這條）。
- 使用者空間程式沒辦法偷偷攔截別人的 `malloc` 導去別處——要嘛在 kernel 做，要嘛對方主動留插槽讓你被呼叫（PyTorch 的 `CUDAPluggableAllocator`、OpenVINO plugin、llama.cpp backend 介面都是這種插槽）。

> **補強**
> - mmap 那條不是「kernel 盲目一次撿 4KB」：kernel 有 readahead（偵測到循序存取會把預讀視窗放大到幾百 KB 甚至 MB），程式也能用 `madvise(MADV_WILLNEED / MADV_SEQUENTIAL)` 或 `MAP_POPULATE` 給提示。llama.cpp 載權重實際上就下了 `madvise(WILLNEED)`（Linux 上再加 `MAP_POPULATE`），所以它不是等 page fault 一頁一頁撿。「被動、kernel 不知道你要什麼」描述的是沒給提示的預設行為。
> - 「攔截別人的 malloc」在同一個 process 裡做得到：動態連結器的 symbol interposition（`LD_PRELOAD=libjemalloc.so`）在對方不配合下就能換掉 malloc，jemalloc / tcmalloc 就是這樣部署的。真正做不到的是跨 process——那才是 process 隔離在擋的事；「留插槽」則是對方主動開的乾淨門，不靠連結器的巧合。

---

## 9. 資料路徑：獨顯 vs iGPU

```
獨顯：  SSD ──DMA──▶ DRAM（暫存區）──cudaMemcpyAsync / PCIe x16──▶ VRAM ──▶ GPU 算
        權重落地兩次、走兩段 PCIe；DRAM 只是中繼；VRAM 頻寬獨立，算的時候不受 CPU 干擾

iGPU：  SSD ──DMA──▶ DRAM（統一記憶體）──直接讀（zero-copy）──▶ Xe GPU 算
        落地一次、少一段 PCIe、少一次 copy、少一份記憶體佔用；
        代價：GPU 跟 CPU、跟 SSD 寫入 DRAM 的流量搶同一條記憶體匯流排
```

- iGPU 沒有「搬進 VRAM」這步：配 GPU 記憶體 = 在 DRAM 劃一塊讓 GPU 也看得到。
- Intel 的 user-mode driver（compute-runtime）開源、kernel driver 在 Linux 主線裡；NVIDIA 要另外裝閉源套件。結構一模一樣，只是每層換了名字。

> **補強**：「權重落地兩次」是獨顯的預設路徑，不是硬體限制。GPUDirect Storage（cuFile API + `nvidia-fs.ko`）讓 SSD 控制器 DMA 直接寫進 VRAM，跳過 DRAM 暫存區；需要 driver 與檔案系統配合，多在資料中心卡上用。資料路徑的骨架（DMA 搬、CPU 不搬）不變，只是 DMA 的目的地從 DRAM 換成 VRAM。

---

## 10. 思維鏈：libtorch 下一個 matmul 到 GPU 算完、CPU 拿到結果

```
t    thread              mode             跑誰的程式碼                  發生什麼
───────────────────────────────────────────────────────────────────────────────────────────────────────
t0   app thread          user             你的程式                      model(x)，呼叫 libtorch
t1   app thread          user             libtorch（引擎）               決定第 N 層做 matmul；從配置器拿 VRAM 指標（cudaMalloc 早配好）
t2   app thread          user             cuBLAS（SDK）                  cublasGemmEx(...)，選一個 GEMM kernel
t3   app thread          user             libcudart.so（SDK）            包成 cuLaunchKernel，轉呼叫 libcuda.so
t4   app thread          user             libcuda.so（user-mode drv）    組命令封包（kernel 位址、參數、grid），寫進映射好的 ring buffer，
                                                                         寫門鈴暫存器（MMIO）。沒有 syscall
t5   —（GPU 硬體）        —                GPU 命令處理器 + SM            從 ring buffer 讀命令，調度幾千個執行緒跑 GPU kernel（CUDA C++ 編的
                                                                         機器碼），讀寫 VRAM
t6   app thread          user             libtorch → libcuda.so          不等。繼續往佇列塞第 N+1、N+2 層的命令（非同步）
t7   app thread          user             libtorch                       需要結果（.cpu() / 印出）→ cudaStreamSynchronize →
                                                                         libcuda.so 看 fence：還沒好
t8   app thread          user → kernel    libcuda.so → syscall           ioctl(/dev/nvidia0, WAIT_EVENT…)；CPU 切特權模式
t9   app thread          kernel           nvidia.ko                      把這條 thread 掛到該 event 的 wait queue，schedule()
t10  其他 thread          —                —                              CPU 跑別人的程式；app thread 不在任何 runqueue，只有 wait queue
                                                                         上有它的節點
t11  —（GPU 硬體）        —                GPU                            算完，結果寫在 VRAM（或 DMA 寫進釘住的 DRAM 分頁），寫 fence 值，
                                                                         發 MSI-X 中斷
t12  被打斷的任一 core    kernel（中斷上下文） OS kernel → nvidia.ko ISR    讀 fence / 狀態暫存器，wake_up() 那個 wait queue → app thread 進 runqueue
t13  app thread          kernel           nvidia.ko → syscall 返回       被排程器挑中，從 schedule() 返回，ioctl 回傳，切回使用者態
t14  app thread          user             libcuda.so → libcudart         看到 fence 完成，返回。不需要「解釋反饋」，資料已在記憶體裡
t15  app thread          user             libtorch                       若要在 CPU 讀：cudaMemcpy D2H → GPU copy engine DMA VRAM→DRAM
                                                                         （再走一次 t7–t14 的等法）
t16  app thread          user             你的程式                       讀到 tensor
```

- 整段只有 t8–t13 在核心態，而且是「等」才進去；t4 的發射不經過 OS。
- t5、t11 沒有任何 CPU thread 參與——硬體自己動。
- 「誰呼叫 wake_up」：t12 的 ISR，跑在任意一個被中斷打斷的 core 上，不是某條專門的 thread。

> **補強**：t8–t13 是設了 `cudaDeviceScheduleBlockingSync` 才會走的路。CUDA 預設是 `cudaDeviceScheduleAuto`——只要 process 裡的 CUDA context 數不超過邏輯 CPU 數，t7 之後 libcuda.so 就留在 user mode 自旋讀 fence：沒有 ioctl、不掛 wait queue、不讓出 CPU，直到 fence 值變了才返回。預測差別：GPU 在算、CPU 在「等」的時候 `top` 會看到一顆 core 100%，那是 libcuda.so 在自旋，不是 CPU 在幫 GPU 算。這也是「等待 ≠ 在 wait queue 上」的又一個例子——自旋是 running 狀態的等，wait queue 是 sleeping 狀態的等。

**附：一次 `pread` 的對照時間軸（儲存路）**

```
t    thread          mode              跑誰的程式碼             發生什麼
──────────────────────────────────────────────────────────────────────────────────────────────
t0   app thread      user              你的程式 / std::fs        File::read_at(buf, offset)
t1   app thread      user              libc                     pread64() → syscall 指令
t2   app thread      kernel            VFS → 檔案系統            把 (檔案, offset) 換算成 block 位址
t3   app thread      kernel            block layer → nvme.ko    組 NVMe read 命令，填 DMA 目的位址（buf 對應的實體頁，釘住），
                                                                寫進 submission queue，敲 SSD 門鈴
t4   app thread      kernel            nvme.ko → schedule()      掛到 wait queue，讓出 CPU
t5   —（SSD 控制器）  —                 SSD firmware             從 NAND 讀出，DMA 直接寫進那些 DRAM 頁，寫 completion queue，發 MSI-X
t6   任一 core        kernel（中斷）     nvme.ko ISR              讀 completion queue，wake_up()
t7   app thread      kernel            nvme.ko → syscall 返回    回傳讀了幾 bytes，切回 user
t8   app thread      user              你的程式                 buf 裡就是資料；CPU 從頭到尾沒搬過
```

- 上表是 `O_DIRECT` 的路；走 page cache 時 t5 的 DMA 目的地是 page cache 頁，t7 多一次 `copy_to_user`。
- 兩條時間軸的骨架一樣：user 發命令 → 必要時 syscall 進核心 → 硬體 DMA 自己搬資料 → 中斷只負責「通知」→ wake_up → 回 user。差別只在 GPU 路多了 user-mode driver 和門鈴優化，SSD 路由 OS 標準堆疊全包。

> **補強**：t6 的「任一 core」在 NVMe 上通常更具體——blk-mq 把每個硬體佇列的 MSI-X 向量綁到一組 CPU，完成中斷多半落回提交那顆 core。而且兩條路都有「不靠中斷」的模式：GPU 那邊是 libcuda.so 自旋讀 fence（上一個補強），SSD 這邊是 `io_uring` 的 `IOPOLL` 配 NVMe poll queue，由 CPU 主動輪詢 completion queue。中斷是「通知」的預設手段，不是唯一手段；追求延遲時兩邊都會拿 CPU 時間把它換掉。

---

## 11. 陷阱清單：錯在哪、正解是什麼、為什麼

1. ❌ PyTorch 只給 Python 用 → ✅ PyTorch 本體是 C++ 的 libtorch，Python 只是前端；C++、Rust（tch-rs）、Java（DJL）都能直接連結 → 名字裡的「Py」指前端從 Lua 換成 Python，引擎從來都是 C/C++。
2. ❌ 「PyTorch 底層使用 libtorch」、libtorch 是 libc → ✅ PyTorch = libtorch，同一個東西兩個名字；libc 是 C 標準函式庫（printf、malloc），完全無關 → 講專案叫 PyTorch、講檔案叫 libtorch，就像沒有「Chrome 底層使用 chrome.exe」這種關係。
3. ❌ libtorch 就是 driver → ✅ libtorch 是引擎，離 driver 隔兩層（cuBLAS、libcudart 在中間，到 libcuda.so 才是 driver） → 引擎只算 tensor，矩陣乘外包給廠商函式庫，函式庫再透過 driver 送 GPU；這也是引擎能跨廠商的原因。
4. ❌ 其他語言都得經過 libtorch → ✅ 兩條路：綁 libtorch（tch-rs）或自刻引擎（Candle）；Candle 從頭到尾沒有 libtorch → 所有引擎最終都呼叫 cuBLAS 這層 C 函式庫，libtorch 只是其中一個引擎。
5. ❌ 大家最後都走到 C 是因為 driver 是 C++ → ✅ 因為廠商只給 C ABI 的閉源二進位，GPU kernel 只能用 CUDA C++ / PTX 寫 → driver 是 C 只是其中一層的事實，不是整件事的因；想碰 GPU 沒有別的門。
6. ❌ ABI 是堆疊中的一層 → ✅ ABI 是兩層間的契約（函式名、傳參、記憶體排法），每個箭頭都是一個 → 元件是 libcuda.so，ABI 是它露出的門面。
7. ❌ kernel = OS kernel = driver → ✅ OS kernel 是本體，driver 是硬體廠寫好插進去的 `.ko` 模組；GPU kernel 又是另一個東西 → 把三個 kernel 當一個，driver 和 kernel 就會疊在一起分不開。
8. ❌ GPU kernel 是 NVIDIA 獨有 → ✅ 平行運算通稱（CUDA / OpenCL / SYCL / Metal / compute shader），GPU、NPU、TPU、FPGA 都有；SSD、網卡沒有，它們跑 firmware → 區分標準是「能不能送程式進去跑」。
9. ❌ driver 是作業系統的一部分 → ✅ 硬體廠寫的，裝進 OS 裡跑 → OS 不可能認識所有硬體，所以開模組介面讓廠商插。
10. ❌ user-mode driver 把指令轉成硬體廠的 binary → ✅ 它組命令封包（跑哪個 kernel、參數在哪）排進佇列；GPU kernel 機器碼早編好或由它 JIT PTX → 「翻譯」是封包格式層級，不是重新編譯程式。
11. ❌ CUDA API 是 user → kernel 的切換點 → ✅ cuBLAS、libcudart 全是使用者態；切換在 libcuda.so 對 `/dev/nvidia0` 的 `ioctl()` → 翻譯不需要特權；只有配記憶體、建 context、等中斷才需要進核心。
12. ❌ 每次 kernel 啟動都要陷入核心態 → ✅ 命令佇列映射在使用者態，寫完敲門鈴 MMIO 即可 → syscall 要幾百 ns 到 µs，每個運算都走 OS 的話 GPU 吞吐撐不起來。
13. ❌ 應用程式收到中斷後要用 user-mode driver 解釋硬體反饋 → ✅ 資料和通知分兩路：資料早由 GPU / DMA 直接寫進 VRAM 或釘住的 DRAM 分頁；中斷只更新 fence 旗標，user-mode driver 看旗標 → CPU 不搬資料，只被通知；而且 CPU 不同步等每個中斷，只在需要結果的點等 fence。
14. ❌ PyTorch 的工作是操作記憶體硬體 → ✅ 它算 tensor；要記憶體就 `malloc` / `cudaMalloc`，記憶體硬體對它透明，由 OS 和 driver 管 → 應用程式看到的是位址空間，不是 DRAM 晶片。
15. ❌ SSD 和 GPU 一樣需要廠商出 user/kernel-mode driver → ✅ NVMe 是跨廠商標準，`nvme.ko` 通用；廠商差異藏在盤內 firmware → 有標準就能有通用 driver，GPU 沒標準所以各家自己出。
16. ❌ 應用程式 call NVMe API → ✅ call OS 檔案 API（read/write/pread、io_uring）；檔案系統 → block layer → nvme.ko 全由 OS 包好 → 程式從頭到尾不知道底下是 NVMe、SATA 還是網路硬碟；直驅 NVMe 的 SPDK 是極端例外。
17. ❌ 使用者空間程式可以攔截別人的記憶體配置導去別處 → ✅ 做不到；要嘛在 kernel 做，要嘛對方主動留插槽（pluggable allocator）呼叫你 → process 隔離就是為了不讓這種事發生。
18. ❌ iGPU 要把資料「搬進 VRAM」 → ✅ 統一記憶體：配 GPU 記憶體 = 在 DRAM 劃一塊讓 GPU 也看得到，zero-copy → 沒有獨立 VRAM，代價是 GPU 跟 CPU 搶同一條匯流排。

> **補強**（對應第 12、13、17 條）：第 12 條的「不陷入核心態」是 NVIDIA / AMD 的 user-mode queue 設計，Intel 預設每次 command list 提交仍走一次 ioctl（§7）；第 13 條的「等 fence」預設是 user-mode 自旋，不是睡在中斷上（§10）；第 17 條的「做不到」只限跨 process，同一個 process 內 `LD_PRELOAD` 就能攔（§8）。
