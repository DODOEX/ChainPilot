# ChainPilot

[English](README.md) | 中文

面向 EVM 兼容网络的链上 DeFi 命令行工具。使用 Rust 构建，通过 [alloy](https://github.com/alloy-rs/alloy) 与链交互，通过 [DODO](https://dodoex.io) 聚合器 API 进行兑换路由。

## 功能特性

- 通过 DODO 路由引擎在 17+ 条 EVM 链上获取兑换报价
- 执行前模拟兑换（余额/授权检查、Gas 预估、回滚检测）
- 执行兑换，支持等待交易上链和状态轮询
- 管理 ERC-20 授权（approve / revoke）
- 查询链上代币元数据与钱包余额
- 对代币、钱包和授权额度进行风险分析
- 机器可读的 JSON 输出（`--json`），适用于脚本和 Agent 流水线

## 安装

**Homebrew（macOS / Linux）：**
```bash
brew tap DODOEX/chainpilot
brew install chainpilot
```

**AUR（Arch Linux）：**
```bash
yay -S chainpilot-bin
```

**Linux / macOS（推荐）：**
```bash
curl -fsSL https://raw.githubusercontent.com/DODOEX/ChainPilot/main/scripts/install.sh | bash 2>&1
```

脚本会自动下载适配当前平台的预编译二进制文件，安装到 `~/.chainpilot/bin` 并配置 `PATH`。

**Windows（PowerShell）：**
```powershell
powershell -ExecutionPolicy Bypass -Command "iwr https://raw.githubusercontent.com/DODOEX/ChainPilot/main/scripts/install.ps1 -UseBasicParsing | iex"
```

脚本会自动下载 Windows 预编译二进制文件，安装到 `%USERPROFILE%\.chainpilot\bin` 并配置用户级 `PATH`。

**从源码构建：**
```bash
cargo build --release
# 产物路径：target/release/chainpilot
```

在编译时将 DODO API Key 和 Project ID 内嵌到二进制：

```bash
DODO_API_KEY=your-key DODO_PROJECT_ID=your-id cargo build --release
```

也可以在项目根目录创建 `.env` 文件：

```
DODO_API_KEY=your-key
DODO_PROJECT_ID=your-id
```

## 配置

环境变量刻意收敛为少数几个。运行时只读取 `PRIVATE_KEY`、`KEYSTORE_PATH`、`KEYSTORE_PASSWORD_FILE`、`KEYSTORE_PASSWORD_ENV`、`KEYSTORE_PASSWORD`、`WALLET_ADDRESS`、`CHAIN_ID`、`DODO_API_KEY`、`DODO_PROJECT_ID`、`DODO_API_URL`；有对应 CLI 标志的情况下，CLI 仍然优先。

| 变量名                 | CLI 标志              | 默认值                       | 说明                               |
|------------------------|-----------------------|------------------------------|------------------------------------|
| `PRIVATE_KEY`          | `--private-key`       | —                            | 用于签署交易的私钥                 |
| `KEYSTORE_PATH`        | `--keystore-path`     | —                            | 用于签署交易的加密 JSON keystore   |
| `KEYSTORE_PASSWORD_FILE` | `--password-file`   | —                            | 从文件读取 keystore 密码           |
| `KEYSTORE_PASSWORD_ENV` | `--password-env`     | —                            | 从指定环境变量读取 keystore 密码   |
| `KEYSTORE_PASSWORD`    | —                     | —                            | keystore 密码的默认环境变量        |
| `WALLET_ADDRESS`       | `--wallet-address`    | —                            | 余额查询 / 模拟时使用的钱包地址    |
| `--rpc-url`            | 仅 CLI                | 链内置公共 RPC               | 显式覆盖 JSON-RPC 端点             |
| `CHAIN_ID`             | `--chain-id`          | `1`（以太坊主网）            | 当前链 ID                          |
| `DODO_API_KEY`         | —                     | 编译时内嵌默认值             | DODO 路由 API Key                  |
| `DODO_PROJECT_ID`      | —                     | 编译时内嵌默认值             | DODO 项目 ID，用于代币列表查询     |
| `DODO_API_URL`         | —                     | DODO 生产端点                | 覆盖路由 API 地址                  |

全局标志（`--json`、`--quiet`、`--private-key`、`--keystore-path`、`--password-file`、`--password-env`、`--wallet-address`、`--rpc-url`、`--chain-id`）适用于所有子命令，需放在子命令名称之前：

```bash
chainpilot --json --chain-id 42161 swap quote --from ETH --to USDC --amount 1.0
```

开启调试日志：

```bash
RUST_LOG=debug chainpilot ...
```

如果设置了 `--keystore-path`，密码读取优先级如下：

1. `--password-file`
2. `--password-env <NAME>`
3. `KEYSTORE_PASSWORD`
4. 在 TTY 终端里交互输入

## 代币解析

代币可以用符号（`ETH`、`USDC`）或 `0x` 合约地址指定，解析顺序：

1. 原生代币符号（如 `ETH`、`BNB`）
2. 原始 `0x` 地址 — 链上获取精度
3. DODO 代币列表缓存（1 小时 TTL）
4. 本地自定义 token 存储（`token add` 和 address 询价成功后自动记录）

自定义 token 行为：

- 可通过 `chainpilot [--chain-id <N>] token add <0xTOKEN>` 手动保存 token
- 如果用户在 `swap quote` 里直接把 `--from` 或 `--to` 传成 token address，且询价成功，CLI 会自动把该 token 的元数据写入本地
- 后续按 symbol 查询时，如果 DODO tokenlist 里没有，会回退到这份本地记录

## 支持的链

| 链名称              | Chain ID   |
|---------------------|------------|
| Ethereum Mainnet    | 1          |
| BNB Smart Chain     | 56         |
| Polygon             | 137        |
| Arbitrum One        | 42161      |
| Optimism            | 10         |
| Avalanche C-Chain   | 43114      |
| Base                | 8453       |
| Linea               | 59144      |
| Scroll              | 534352     |
| Manta Pacific       | 169        |
| Mantle              | 5000       |
| Aurora              | 1313161554 |
| OKChain (X Layer)   | 66         |
| Conflux eSpace      | 1030       |
| Taiko               | 167000     |
| Plume               | 98866      |
| Sepolia Testnet     | 11155111   |

不在列表中的链，请手动传入 `--rpc-url`。

## 典型兑换流程

推荐的完整端到端流程：

```bash
# 1. 获取报价并保存报价 ID
QUOTE_ID=$(chainpilot --json swap quote --from ETH --to USDC --amount 0.1 | jq -r .data.quote_id)

# 2. 模拟 — 检查余额、授权、Gas 及潜在回滚，不消耗 Gas
chainpilot swap simulate --quote-id "$QUOTE_ID" --wallet 0xYourAddress

# 3. 如有需要，授权代币（原生 ETH 兑换可跳过）
chainpilot --keystore-path ~/.chainpilot/main.json swap approve --quote-id "$QUOTE_ID"

# 4. 执行并等待确认
chainpilot --keystore-path ~/.chainpilot/main.json swap execute --quote-id "$QUOTE_ID" --wait
```

报价的本地有效期默认是 18 分钟，DODO 签发的路由本身有 20 分钟截止时间。`simulate` 和 `execute` 均会拒绝已过期的报价。

## 使用说明

### Swap（兑换）

**获取报价：**
```bash
# 以太坊主网基础报价
chainpilot swap quote --from ETH --to USDC --amount 1.0

# Arbitrum 上自定义滑点
chainpilot --chain-id 42161 swap quote --from ETH --to USDC --amount 1.0 --slippage 0.5
```

报价会保存在本地，通过 `quote_id` 标识，后续传给 `simulate`、`approve`、`execute`。

**模拟报价（预检，不消耗 Gas）：**
```bash
# 检查余额、授权、Gas 预估和回滚风险
chainpilot swap simulate --quote-id <QUOTE_ID> --wallet 0xYourAddress
```

模拟是只读操作，不消耗 Gas，不广播交易，用于确认报价可以安全执行。

**执行兑换：**
```bash
# 演习模式：构建并模拟交易，不广播
chainpilot swap execute --quote-id <QUOTE_ID> --dry-run --wallet 0xYourAddress

# 用裸私钥正式执行
chainpilot swap execute --quote-id <QUOTE_ID> --private-key 0x...

# 用 keystore 正式执行；如有需要会交互输入密码
chainpilot --keystore-path ~/.chainpilot/main.json swap execute --quote-id <QUOTE_ID>

# 非交互方式执行 keystore 交易
chainpilot --keystore-path ~/.chainpilot/main.json --password-file ~/.chainpilot/main.pass \
  swap execute --quote-id <QUOTE_ID> --wait

# 覆盖 Gas 参数
chainpilot swap execute --quote-id <QUOTE_ID> --private-key 0x... \
  --gas-limit 300000 \
  --max-fee-gwei 25 \
  --gas-buffer-pct 20

# 跳过 eth_estimateGas 预检，直接使用报价中的 Gas 估算
chainpilot swap execute --quote-id <QUOTE_ID> --private-key 0x... --skip-estimate
```

| 执行标志            | 说明                                                              |
|---------------------|-------------------------------------------------------------------|
| `--dry-run`         | 构建并模拟交易，不广播；使用 `--wallet` 代替私钥                 |
| `--wait`            | 阻塞直到交易上链，显示最终链上状态                               |
| `--gas-limit`       | 硬覆盖 Gas 上限                                                   |
| `--max-fee-gwei`    | 覆盖 EIP-1559 最大 Gas 费用（gwei）                              |
| `--gas-buffer-pct`  | 在 `eth_estimateGas` 结果上添加 N% 缓冲（如 `20` = +20%）       |
| `--skip-estimate`   | 跳过 `eth_estimateGas`，直接使用报价中的 Gas 估算                |

**查询交易状态：**
```bash
chainpilot swap status --tx-hash 0x...
chainpilot --chain-id 42161 swap status --tx-hash 0x...
```

**查看兑换历史：**
```bash
chainpilot swap history
chainpilot swap history --limit 50
chainpilot swap history --limit 50 --status confirmed
# --status 可选值：pending | confirmed | failed
```

**授权代币：**
```bash
# 从已保存的报价授权（自动推导代币和 DODOApprove 合约地址）
chainpilot swap approve --quote-id <QUOTE_ID> --private-key 0x...

# 同样支持 keystore signer
chainpilot --keystore-path ~/.chainpilot/main.json swap approve --quote-id <QUOTE_ID>

# 显式指定代币、授权方和额度
chainpilot swap approve --token USDC --spender 0x... --amount 1000 --private-key 0x...

# 省略 --amount 表示无限授权（U256::MAX）
chainpilot swap approve --token USDC --spender 0x... --private-key 0x...

# 演习模式，预览但不发送
chainpilot swap approve --quote-id <QUOTE_ID> --dry-run
```

**撤销授权：**
```bash
chainpilot swap revoke --token 0xTokenAddr --spender 0xSpenderAddr --private-key 0x...

# keystore signer
chainpilot --keystore-path ~/.chainpilot/main.json swap revoke --token 0xTokenAddr --spender 0xSpenderAddr

# 演习模式
chainpilot swap revoke --token 0xTokenAddr --spender 0xSpenderAddr --dry-run
```

### Token（代币）

```bash
# 元数据：名称、符号、精度、总供应量
chainpilot token info USDC
chainpilot token info 0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48

# 链上合约详情：代理、所有者、实现地址
chainpilot token contract USDC
chainpilot --chain-id 137 token contract USDC

# 本地保存自定义 token，供后续按 symbol 解析
chainpilot token add 0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48
chainpilot --chain-id 8453 token add 0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913
```

### Wallet（钱包）

```bash
# 原生代币和 ERC-20 余额
chainpilot wallet balance 0xYourAddress
chainpilot --chain-id 56 wallet balance 0xYourAddress

# 仅查询指定代币余额
chainpilot wallet balance 0xYourAddress --tokens 0xToken1,0xToken2
```

### Risk（风险）

```bash
# 代币风险分析（貔貅检测、所有权、流动性）
chainpilot risk token USDC
chainpilot --chain-id 1 risk token 0xSomeAddress

# 钱包风险概览（敞口、高风险授权）
chainpilot risk wallet 0xYourAddress

# 查看特定授权的当前状态
chainpilot risk approval 0xYourAddress --token USDC --spender 0xSpenderAddr
```

## 输出模式

默认以彩色表格输出。

**JSON 输出** — 添加 `--json` 以获取结构化输出，适用于 `jq` 或 Agent 流水线：

```bash
# 直接捕获报价 ID
QUOTE_ID=$(chainpilot --json swap quote --from ETH --to USDC --amount 1.0 | jq -r .data.quote_id)

# 查看完整报价内容
chainpilot --json swap quote --from ETH --to USDC --amount 1.0 | jq .

# 检查模拟是否通过
chainpilot --json swap simulate --quote-id "$QUOTE_ID" --wallet 0xAddr | jq .data.ok
```

JSON 响应格式：成功时为 `{ "ok": true, "data": ... }`，失败时为 `{ "ok": false, "error": "..." }`。

**静默模式** — `--quiet` 抑制所有输出（错误除外），适用于只关心退出码的脚本。

## 构建与测试

```bash
cargo build
cargo test
cargo build --release
```
