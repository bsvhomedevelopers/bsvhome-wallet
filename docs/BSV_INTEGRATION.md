# BSV 链接入方案：Rust Core 扩展与 iOS 集成

状态：Core 与 iOS 首期实现及本地验证已完成，主网实际收发与设备操作验收待进行。更新日期：2026-09-28。已执行确定性测试、独立签名向量核对和 WhatsOnChain 真实网络只读查询；主网实际收发、设备认证与完整操作回归仍待验收。用户已确认开始编码；第二轮回归修复同步更新了 Android 的共享接口调用和费用单位显示，尚未完成 Android BSV 功能接入或部署后端服务。具体结果与残余风险见第 11 节。

2026-10-05 后续范围：用户已确认 [BSV 多地址方案](BSV_HD_ADDRESS_PLAN.md) 并授权落地。本文中的单地址、原址找零、多地址延期及普通发送恢复窗口描述保留为首期基线；当前多地址实现以该方案末尾的实际接口与验证记录为准。BSV 助记词钱包保留稳定主账户，初始化 10 个收款子地址，找零按需派生，查询按钱包汇总，普通发送与 Portal 接入持久预留及 version 1 checkpoint。观察和单私钥钱包保留单地址能力。本轮只调整 Core 与 iOS；主网小额收发按用户要求待专用测试钱包就绪后执行。

## 1. 目标与范围

在现有 Rust Core 中扩展 BSV 原生币能力，iOS 通过 BsvHomeCore 使用统一账户、余额、收款和转账流程。密码学与私钥存储继续使用现有库及 Keystore；BSV 协议适配由 Core 承担。

本阶段实施范围为 Rust Core、iOS 及其本地生成产物和测试。不开发或部署后端服务，不修改 Android 业务代码。Core 共享模型变化对另一平台的影响需要在交付记录中明确，但不构成本阶段的验收目标。

| 功能 | 本阶段交付范围 |
| --- | --- |
| 链与资产 | BSV 主网原生资产、8 位小数、链能力配置、本地图标与浏览器链接 |
| 账户 | 新建钱包、默认派生路径的助记词导入、单地址观察钱包、已有多链助记词钱包补充 BSV 账户 |
| 余额 | 当前账户的真实链上余额、可用余额及刷新；明确未确认金额与可花费金额的区别 |
| 收款 | 当前账户真实地址、二维码、复制、分享，以及选定 BSV 后的地址扫描 |
| 转账 | P2PKH 原生币转账、手续费预估、找零、发送全部可用余额、认证、签名、广播与状态追踪 |
| 交易记录 | 本地发送记录、当前地址链上收发记录、详情、确认状态；不依赖新增后端索引 |
| 验证 | Core 确定性测试、真实网络只读测试、iOS 构建与操作回归、专用测试钱包实际收发 |

价格、行情、法币估值、汇率、兑换、Paymail、MNEE/其他代币、NFT、WalletConnect、硬件钱包、多地址 HD 管理及旧钱包自动路径扫描不在本阶段。WIF 私钥导入不作为基础验收项；如后续纳入，必须同时实现格式解析、网络校验、压缩公钥标识处理及导入测试。

本方案采用单地址账户：默认收款地址固定，找零返回同一账户地址。该方案不宣称可以恢复其他钱包所有地址中的历史资产。旧 BCH/BSV 分叉资产的拆币与恢复需另设兼容范围。

### 1.1 已确认的首期决策

| 事项 | 首期执行要求 |
| --- | --- |
| 数据与广播服务 | 使用 WhatsOnChain 完成接入开发及小规模验证，暂不自建节点或新增后端代理；后续按实际限流与服务表现选择付费服务、其他供应商或自建节点 |
| 节点管理边界 | 所有链通过 Core 公共节点管理统一配置默认地址、用户选择和节点检查；BSV 纳入同一机制，将 WhatsOnChain 配为默认服务。各链的 API 协议、手续费与广播差异由对应 provider 处理 |
| 手续费精度 | 开发中获取真实响应并核对供应商契约后冻结单位、精度、交易大小口径与取整规则；本方案不预设费率小数位数 |
| 不支持 BSV 的 API | 通过现有错误链路向界面反馈，复用本地化 toast；在实际未实现的调用或错误映射所有者处标记 `TODO(BSV-API)`，在本文登记具体缺口 |
| 交互与共享接口 | 保留现有交互，优先在 BSV 适配层处理协议、分页和状态差异；只有现有接口确实无法表达必要行为时才提出最小扩展 |
| 提交、记录与恢复 | 优先复用现有公共签名、提交、Store 与状态追踪；实现同一 txid 的超时查询、正常记录持久化和重启恢复，极端故障在回归时记录实际表现、改进建议和隐患，不提前重构公共恢复体系 |

金额守恒、收款人正确、私钥边界与认证仍是基础验收要求。若现有公共逻辑无法满足必要的 BSV 行为，先提交具体接口限制或失败证据及改动建议，等待用户确认，不自行猜测产品策略或扩大改造范围。

## 2. 接入前事实与改动所有者

| 当前所有者 | 已有能力或限制 | 接入要求 |
| --- | --- | --- |
| [Chain](../core/crates/primitives/src/chain.rs)、[ChainConfig](../core/crates/primitives/src/chain_config.rs) | 没有 BSV；能力由 Core 配置提供 | 增加 BSV，继续归入 `ChainType::Bitcoin` |
| [bsv_home_derivation](../core/crates/bsv_home_derivation/src/private_key/path.rs) | 已支持 Bitcoin 家族的 BIP32/BIP44/BIP84 派生 | 增加 BSV 默认路径、地址派生及账户恢复向量 |
| [bsv_home_bitcoin](../core/crates/bsv_home_bitcoin/src/signer/planner/mod.rs) | 已有 UTXO 规划、找零与金额校验；部分构件仅 `pub(crate)` 可见 | 开放有实际消费者的通用构件，供 BSV 组合 |
| [Bitcoin Cash 签名](../core/crates/bsv_home_bitcoin/src/signer/bitcoin_cash.rs) | 已实现 `0x41` 的 ForkID P2PKH 签名 | 提取可共享部分，用独立 BSV 向量验证 |
| [Bitcoin RPC](../core/crates/bsv_home_bitcoin/src/rpc/target.rs) | 使用 Blockbook `/api/v2/...` 协议 | BSV 数据服务单独适配，不能只替换域名 |
| [BsvHomeKeystore](../core/bsv_home_core/src/keystore/keystore.rs) | 私钥在 Rust 内部解密、派生与签名 | 保持现有私钥边界与清理机制 |
| [BsvHomeBalanceService](../core/bsv_home_core/src/services/balance/mod.rs) | 余额通过 Gateway 获取，资产初始化还涉及资产服务 | 验证 BSV 原生资产可本地初始化，不被服务器缺失资产元数据阻塞 |
| [BsvHomeAssetsService](../core/bsv_home_core/src/services/assets/mod.rs) | `ensure_default_assets` 已能本地保存默认资产；详情与部分同步仍调用服务器 | 优先复用本地初始化；服务器不支持 BSV 的调用按本期 toast 与待实现标记要求反馈 |
| [BsvHomeTransactionsService](../core/bsv_home_core/src/services/transactions/mod.rs) | 当前 `sync_wallet` 通过设备 API 获取历史 | 必须接通 Core 内的地址历史读取及现有 Store；仅新增 provider 不足以让 iOS 显示完整历史 |
| [iOS 签名适配](../ios/Packages/BsvHomeCoreServices/Sources/Signer/KeystoreTransactionSigner.swift) | 认证后调用 Keystore，返回签名结果 | 继续复用，避免 Swift 新建另一套私钥签名路径 |
| [WowPay BSV 展示](../ios/Features/WalletTab/Sources/Scenes/WowPayUnavailableScene.swift) | BSV 页面为暂未开放入口 | 接入真实资产及现有 Transfer/Receive 流程 |

此表记录接入前的调查基线及改动所有者；完成状态以第 11 节为准。改动仅覆盖 BSV 所需范围，保留当前工作区的其他工作。

## 3. 规范与组件边界

遵循 [Task Workflow](../skills/task-workflow.md)、[Engineering Principles](../skills/engineering-principles.md)、[Core 新链规范](../core/skills/new-chain-checklist.md)、[Core 架构规范](../core/skills/architecture.md)、[功能架构](ARCHITECTURE.md)、[Keystore v4](KEYSTORE_V4.md)、[安全规则](../skills/security.md) 和 [iOS 集成规则](../ios/skills/core-integration.md)。

### 3.1 分层职责

| 层 | 职责 | 封装约束 |
| --- | --- | --- |
| `primitives` | 链身份、资产、能力、金额与跨层模型 | 能力统一来自 `ChainConfig`；新增枚举需处理所有穷尽分支 |
| `bsv_home_bsv` | BSV 地址/脚本、网络请求、响应映射、手续费及签名适配 | BSV 特有规则集中于此，按现有 `chain_traits` 与 `ChainSigner` 集成 |
| `bsv_home_bitcoin` | 家族通用交易与密码学构件 | 提取必要接口，保持既有链默认行为；不添加 BSV 专属判断分支 |
| `bsv_home_derivation` / Keystore | 派生、账户、存储与安全签名 | 派生与地址能力经现有工厂分发；不在 Swift 暴露解密私钥 |
| `bsv_home_core` 服务 | 钱包、余额、历史、收款、确认与提交编排 | 扩展现有服务，持有服务自身的 Store；链规则不下放 iOS |
| Core 公共节点管理 | 各链默认请求地址、用户选择及其持久化、节点检查编排 | BSV 默认 WhatsOnChain；将所选地址交给对应 provider，链协议与检查请求由 provider 适配 |
| iOS | 服务注入、Store 实现、呈现、导航与本地认证 | 页面不直接发送 BSV RPC，不自行判断手续费或构造交易 |

### 3.2 `bsv_home_bsv` 的实际结构

```text
core/crates/bsv_home_bsv/
  Cargo.toml
  examples/read_only.rs
  src/
    lib.rs
    address.rs
    signer.rs / signer_tests.rs
    rpc.rs
    rpc/
      client.rs
      target.rs
      models.rs
      tests.rs
    provider/
      mod.rs
      chain_traits.rs
      transactions_mapper.rs
  testdata/
```

`BsvTarget` 集中维护 REST 路径和分页参数。Client 使用所选节点地址，负责共享节流、读取重试、分页及响应校验；provider 实现统一链 trait，BSV 地址、金额与输出规则在本 crate 内处理。输出投影复用现有 UTXO 逻辑，coinbase 及无法投影地址的输入在 BSV mapper 内按钱包输出处理，不把其它地址输出加到收款金额。

已实现 feature 为 `default = []`、`signer`、`rpc`、`reqwest`。真实网络只读验证由显式运行的 `read_only` example 完成，不在默认测试中自动请求网络或花费资金。fixture 通过现有 `bsv_home_client::testkit::MockClient` 与 Store testkit 使用；未增加无人消费的 `testkit` 或 `chain_integration_tests` feature。

### 3.3 Bitcoin 家族复用的实施边界

当前 `SpendRequest` 与 planner 接口依赖 `BitcoinChain`，仅把模块改成 `pub` 还不够。先将通用输入/输出、交易规划和 ForkID 签名所需构件整理成能接受已校验脚本、金额与策略的最小接口。旧链适配层继续提供其现有费率、sequence、dust 和地址规则，BSV 在自身 crate 提供对应策略。

优先复用现有类型并扩展其必要参数；当 BTC/BCH 与 BSV 确有共同规则时才提取接口。不得把 BSV 假装成 `BitcoinCash` 传入 planner，也不得复制整套 planner 或对现有 Bitcoin 家族做无关重构。

`BitcoinChain` 是否增加 BSV 变体取决于派生/家族分类的真实需要；不能让新增变体迫使 `bsv_home_bitcoin` 到处处理 BSV。若分类增加变体，工厂必须在通用 Bitcoin 转换之前分发 BSV，所有穷尽分支有明确行为，禁止用默认分支掩盖缺失支持。

## 4. 协议与网络决策

### 4.1 链身份、派生与地址

已冻结链枚举 `Chain::BitcoinSv`、序列化值和原生 AssetId `bitcoinsv`，展示符号 `BSV`。AssetId、Store、节点偏好与生成模型使用同一标识；WoC URL 中的 `/bsv/` 是供应商协议路径，不是另一套业务链标识。

已冻结新建主网账户默认路径为 `m/44'/236'/0'/0/0`，使用成熟 BIP32/secp256k1 能力。[SLIP-0044](https://github.com/satoshilabs/slips/blob/master/slip-0044.md) 登记 BSV coin type 为 236；这一登记不证明所有 BSV 钱包使用同一路径。

第一阶段仅支持约定的压缩公钥 P2PKH 账户与 P2PKH 收款脚本。地址生成、Base58Check 校验、主网/测试网前缀处理在 Core 完成。拒绝不支持的脚本、BTC SegWit/Taproot 地址及超范围输入，错误向上传递，不降级猜测。

BTC/BCH/BSV 的部分传统地址可能相同，不能通过地址字符串可靠识别资产网络。BSV 接收人输入必须绑定已选链；扫描裸地址出现多个匹配资产时保留现有资产选择步骤，不能默认为 BSV。不要把 `bitcoin:` URI 直接改解释为 BSV；本阶段先使用真实裸地址二维码，专用 URI 仅在协议、解析和互操作测试明确后增加。参见 [Payments](PAYMENTS.md)。

主网配置不能只通过共用创世块证明连接的是 BSV。节点检查需要能区分分叉后的 BSV 网络，例如供应商网络标识与明确的 BSV 检查点；测试网单独验证。测试配置不覆盖主网钱包配置，测试地址和余额不能进入生产 Store。

### 4.2 数据服务

[WhatsOnChain](https://docs.whatsonchain.com/) 已确定为首期查询、费用与广播服务，用于完成 BSV 接入开发及小规模验证，暂不自建节点或新增后端代理。后续根据实际限流和服务表现迁移至付费服务、其他供应商或自建节点。其官方广播接口面向小规模使用，见 [广播说明](https://docs.whatsonchain.com/transaction#broadcast-transaction)；不能把首期验证结果直接当作规模化上线容量证明。

余额、UTXO、历史、详情、状态、费用和广播通过 Core 的 BSV provider 统一封装，RPC Client 继续只负责发送请求与解码。iOS 消费现有统一接口，不感知 WhatsOnChain 的端点、字段、费用政策或认证方式，后续服务替换限定在 Core provider 与配置。

免费额度按官方公开的 3 请求/秒作为首期节流依据，但计数范围尚未确认，不假定每台设备独享额度，也不假定本地节流能够保证共享出口不会限流。IP、凭据、接口或其他聚合维度的实际计数方式及适用限制列为上线前核实项。

开发第一步请求真实数据、核对以下契约并保存脱敏 fixture；未确认的具体端点、费率与请求额度口径不作为稳定协议。

| 契约 | 开发时必须确认 |
| --- | --- |
| 余额 | confirmed/unconfirmed 字段、单位、负未确认余额含义及可用金额规则 |
| UTXO | txid、vout、satoshi、脚本、确认状态、分页、是否包含未确认或已花费输出 |
| 历史与详情 | 分页游标、交易排序、输入来源、输出脚本、费用、同地址找零、自转账和确认状态 |
| 费率 | WhatsOnChain 实际响应、单位、数值类型、精度、交易大小口径、适用交易类型、最低接收要求与报价失效策略；获取真实数据后冻结 |
| 广播 | 请求与响应、返回 txid、重复提交、超时、已知交易、冲突和费用不足错误 |
| 节点 | 主网/测试网身份、区块高度、健康状态及 BSV 分叉后身份依据 |
| 网络调用 | TLS、限流计数范围、接口特殊限额、认证、读取重试与超时；付费凭据不能硬编码到 App |

在 provider 使用的共享请求路径中实现节流，让查询、费用、状态轮询和广播共同受控；复用已有可用构件，不在每个页面建立独立限速器。读取仅对已识别的暂时错误执行有次数与时间上限的退避重试，根据 HTTP 状态处理限流；当前公共响应模型不携带响应头，使用有界退避，尚未读取 `Retry-After`；协议错误、不支持的 API 和无效请求不反复重试。具体节流、超时和重试参数在开发验证中记录，不虚构供应商保证。

广播超时先用签名交易本地计算的同一 txid 查询状态，复用公共提交与记录流程，不自动创建另一笔支付；查询未找到不等于广播肯定失败，也不能据此重新构造交易。交易记录持久化、pending 追踪和重启恢复纳入首期验证；公共逻辑的故障边界按第 5.3 节处理。网络错误不能映射成零余额、空历史或成功转账。

小规模验证记录限流次数/比例、响应时间分布和广播失败率，区分明确拒绝与超时结果未知。优先复用现有诊断能力，不在本阶段新增监控后端，也不上传钱包秘密或完整签名交易。上线前明确生产监测出口、统计窗口、迁移触发阈值与负责人，在服务持续影响用户前启动迁移；没有真实指标时不能声称监测或容量验收已完成。

迁移只替换数据、费用或广播适配及服务配置，保持已冻结的链标识、AssetId、派生路径、公钥压缩与地址规则一致，复用已有账户、Store 和交易记录，不要求用户重新导入钱包或转移资金。候选服务先通过相同 fixture、签名契约与小额收发回归；不在首期增加多供应商自动切换体系。

节点管理方案已确认：所有链通过 Core 公共节点管理统一配置和选择请求地址，BSV 纳入该机制，将 WhatsOnChain 配为默认服务。统一执行链路为 `iOS → Core 公共节点管理选择地址 → 对应链的 provider → 发出请求`。公共节点管理负责默认地址、用户选择及其持久化、节点检查编排；provider 负责各链 API 协议、检查请求与响应映射、手续费和广播差异。BSV provider 使用公共节点管理交付的地址，不在 provider 或工厂内另设绕过用户选择的固定端点。

公共节点配置和选择规则已为 BSV 接入 WhatsOnChain 默认项，并贯通现有 Gateway、工厂和 AlienProvider/Client；其他链保持原有区域代理默认行为。验证首次使用、用户改选、重启恢复、节点检查及后续请求确实使用同一所选地址；不只把 WhatsOnChain 加到候选列表，也不假定区域代理的 `/bitcoinsv` 路由已部署。范围限于完成 BSV 接入所必需的公共节点能力，保留其他链既有默认行为。

统一地址管理不意味着任意地址都兼容同一 API 协议。首期 BSV provider 适配 WhatsOnChain；用户选择的服务必须符合当前 provider 支持的协议，不兼容时由公共节点检查反馈。后续供应商或自建节点若使用不同协议，按服务迁移要求扩展或替换 BSV provider，继续沿用公共地址选择机制。

### 4.2.1 接入条件与待确认项

| 项目 | 状态与确认时点 |
| --- | --- |
| 首期供应商及规模 | 已确认：WhatsOnChain，接入开发与小规模验证 |
| 服务封装与替换边界 | 已确认：Core provider、现有统一接口，不要求用户迁移钱包或资金 |
| 实际请求 URL 接线 | 已确认：公共节点管理为 BSV 默认选择 WhatsOnChain，将所选地址传给 BSV provider；实施时验证默认、改选、检查与持久化 |
| API wire、错误与分页契约 | 开发时通过真实响应与官方契约核对，进入 fixture 和测试 |
| 费用单位、精度及取整 | 开发取得实际数据后冻结，完成前不开始真实转账验收 |
| 未确认 UTXO、找零与状态政策 | 开发时核对实际服务政策；无法确定则提出具体问题，不自行猜测 |
| 免费 3 请求/秒的计数范围 | 上线前核实，不假定每台设备独享 |
| 生产监测及迁移阈值 | 上线前明确监测出口、统计口径、负责人和触发条件；首期先记录验证指标 |
| 公共恢复逻辑的极端边界 | 回归时验证并登记失败窗口、改进方案和残余风险；必要行为受阻时反馈用户 |

### 4.3 交易与费用

普通 P2PKH 转账采用 BSV 对应的 ECDSA/ForkID 签名摘要。[BSV 签名文档](https://hub.bsvblockchain.org/bsv-skills-center/guides/sdks/ts/low-level/tx_sig) 提供 `SIGHASH_ALL | SIGHASH_FORKID` 示例。现有 BCH `0x41` 实现仅作为共享候选，必须用独立 BSV 原始交易与摘要向量验证；仅检查签名最后一个字节不足以证明正确。

普通原生币转账的输入须属于本账户，前序输出的金额和锁定脚本必须与签名使用的数据一致。当前 `UTXO` 模型主要保存地址与金额；适配边界应保留或验证来源脚本与确认信息，需要扩展模型时同时处理 FFI 映射。校验交易输入、收款输出、找零输出与费用守恒，不允许额外未知输出。

BSV 原生币的 8 位小数与费率的小数精度分别处理。费率单位及小数位数暂不预设，开发中获取 WhatsOnChain 的真实费用响应，结合其公开契约冻结数值类型、允许范围、sat/kB 或其他单位、每 kB 字节数、交易大小口径、最小接收费用、报价有效期和最终取整规则；单次返回整数费率不能证明服务永远只返回整数。冻结记录包含非秘密响应、来源、核对日期、换算规则及边界用例，不能把样本费率硬编码为长期政策。参考 [BSV 费用模型](https://hub.bsvblockchain.org/bsv-skills-center/guides/sdks/concepts/fee)。

费用采用明确单位的整数/有理数或十进制定点表示，最终 satoshi 费用向上取整，不使用二进制浮点数或中间截断。报价解析、UTXO 规划、预估、确认、签名与 iOS 展示消费同一费用规则，展示舍入不回写交易计算。当前 `FeeUnitType::SatVb` 仅一位小数，provider 部分换算还有整数截断；优先由 BSV 适配层提供准确换算，只有现有模型不能无损表达实际费率时才提出 Core 所有者处的最小模型及展示扩展，不偷偷提高最低值或四舍五入为零。规则未确定或报价无法准确表示时阻止提交并反馈具体阻塞。

BSV adapter 明确提供 sequence、输入大小、找零阈值、可花费 UTXO 与费用策略。不继承 BTC/BCH 的最低费率、RBF 语义、优先级档位或 dust 常量。报价失败且无已定义安全费用策略时阻止提交；用户明确确认过的金额、费用或收款人变化后必须重新确认。

第一阶段不开放任意脚本、上链数据或 memo 扩展，现有 80 字节 OP_RETURN 工具不代表 BSV 数据功能已支持。若界面可输入此类字段，需明确禁用或返回不支持错误。

未确认输出的可花费条件由实际服务政策与 Core 决定并测试；余额不等于可花费余额。同一输出不能在未结束的发送流程中重复使用，优先扩展现有确认/提交协调与状态机制，不新增无消费方的全局锁或缓存。

旧 BCH/BSV 未拆分输出存在跨链重放风险，参见 [ElectrumSV 拆币说明](https://electrumsv.readthedocs.io/en/sv-1.3.15/problem-solving/coin-splitting.html)。`FORKID` 不构成 BCH/BSV 自动隔离的证明。本阶段不支持自动拆币或宣称完整恢复；遇到已识别的历史分叉币兼容需求需扩展范围后验证。

## 5. Core 接入清单

### 5.1 基础模型与工厂

修改计划覆盖 `primitives` 中的 `chain.rs`、`chain_config.rs`、`asset.rs`、`node_config.rs`、`node_check.rs`、`block_explorer.rs`，以及需要变动的金额/UTXO 模型。能力按真实实现设置，Swap/NFT/DeFi 等本期不开放。

公共节点管理接入覆盖 [节点服务](../core/bsv_home_core/src/services/node/mod.rs) 与 [选择规则](../core/bsv_home_core/src/services/node/rules.rs) 的实际所需变更：BSV 默认 WhatsOnChain，复用用户选择及持久化，节点检查通过 BSV provider 执行并返回公共结果。默认地址与有效选择只在公共管理所有者处决定，provider 不静默改回默认服务；现有接口确有不足时按最小扩展原则处理。

在 [settings_chain](../core/crates/settings_chain/src/lib.rs)、[Gateway 工厂](../core/bsv_home_core/src/gateway/chain_factory.rs)、[签名工厂](../core/bsv_home_core/src/signer/chain.rs) 注册 BSV。必要的 Core 客户端 settings 可调整，但不改 `core/apps` 下后端或 dynode 部署代码。审查 `chain_type() == Bitcoin`、`is_utxo()` 及全链枚举消费者，防止家族能力误继承和 `from_chain(...).unwrap()` 失败。

### 5.2 账户与 Keystore

扩展 `bsv_home_derivation` 的默认路径、scheme、地址分发及必要的扩展公钥能力，保持既有链输出不变。扩展公钥可否恢复多地址不属于本阶段能力承诺。

新建/导入与已有钱包补链沿 [BsvHomeWalletService](../core/bsv_home_core/src/services/wallet/mod.rs) 现有入口。补链只针对符合现有条件的助记词钱包；观察钱包不生成密钥，其他单链/私钥钱包不自动转为多链钱包。补链失败保留原钱包，按钱包报告失败，不能吞掉错误或重复增加账户。

私钥仍由 `BsvHomeKeystore.sign` 内部读取、派生与签名。iOS 传入钱包标识、链、准备好的交易与认证数据，只接收签名结果。继续执行临时密钥清理和既有认证；不在 Swift/JS 暴露私钥，不新增明文存储。

### 5.3 服务编排：无需后端改动的必要扩展

`bsv_home_bsv` 实现 `ChainBalances`、`ChainState`、`ChainTransactionLoad`、`ChainTransactions`、交易详情/状态与广播等当前使用的 trait。无需本期功能的 trait 保持现有明确的不支持语义，不伪造成功响应。

资产初始化优先复用 `BsvHomeAssetsService.ensure_default_assets` 的本地元数据路径；审查钱包初始化、资产显示标志和缺失资产同步。真实 BSV 数量、收款与发送由 Core 接入实现。资产详情、发现、订阅或其他 API 如不支持 BSV 链或币种，在实际调用/错误映射所有者处登记 `TODO(BSV-API)` 并向界面反馈本地化 toast，不在本期新增后端支持或伪造成功。没有价格时保留未知/不可用状态，不能显示伪造价格、零估值或固定兑换率。若不支持的请求实际上阻断必要收发流程，报告调用链与所需最小改动，不以 toast 替代核心能力完成。

历史同步保留现有交互与 Store，优先由 BSV 适配层处理分页、排序、状态和领域交易映射，再通过现有服务接入地址历史。当前 `TransactionsRequest/Result` 不含分页游标，先验证 provider 内部遍历或已有时间过滤能否表达所需行为；确实不能时再提出 Gateway、trait 或同步进度的最小扩展，不预先增加公共分页体系。BSV 使用 WhatsOnChain 数据，其他链保持现有来源。钱包级刷新需要合并各来源、去重并正确分页；直接链历史不能推进服务器历史游标导致其他链漏记录。

链上入账、出账、找零与自转账需要按账户归属计算净变化，不能把每个输出都显示为新收款。后台推送通知不在本期，前台刷新与发送后的状态轮询必须实际可用。

当前 `BsvHomeConfirmService.store_pending` 使用 `record(...).await.unwrap_or_default()`，存在广播成功但本地记录失败被隐藏的分支。首期按现有公共提交、Store 与状态机制接入，正常交易记录持久化、重启读取与状态恢复必须实际验证。Store 写入失败、广播后进程退出等极端窗口在回归中故障注入并登记实际表现、改进建议及残余隐患；本期不预先重构公共提交服务，也不承诺现有逻辑在这些窗口下保证零丢失。若现有逻辑无法满足同一 txid 查询或正常重启恢复，提交具体证据及最小改动方案给用户确认。未修复/未验证的分支明确列入未完成项，不能丢记录后诱导重新签名支付。

广播返回 txid 只代表提交/接收结果。优先由 BSV 适配层映射至现有状态与错误模型，保留交互并准确表达确认与失败；广播超时或查询暂未找到不能直接标记为确定失败。现有模型确实无法表达必要状态时，提出具体限制与最小扩展方案再确认。超时先用本地计算的 txid 查询状态，已知交易按相同交易处理，不直接再构造不同交易。

## 6. iOS 集成与组件复用

按 [组件目录](UI_COMPONENTS.md) 与 [样式 Token](UI_STYLE_TOKENS.md) 复用当前实现。`Components` 承担呈现，`PrimitivesComponents` 承担领域显示组件，Feature 承担页面编排；不新增并行 BSV 收发 UI 框架。

| iOS 所有者 | 开发事项 |
| --- | --- |
| `Primitives` / `BsvHomeCorePrimitives` | 接收生成的 BSV 模型，处理枚举映射、金额精度和转换 |
| `Style` / `ChainImage` | 配置本地 BSV 链图标，处理穷尽 switch；复用已有 WowPay 资源前确认用途 |
| `BsvHomeCoreServices` | 注入 Core 服务，复用 `LocalKeystore`、`KeystoreTransactionSigner`、余额与交易 Store |
| Onboarding / ManageWallets | 增加 BSV 单链选择、默认路径助记词导入、观察地址及已有钱包补链 |
| WalletTab / Assets | 用真实 BSV AssetData 替换占位入口，保留加载、空值、错误与观察钱包限制 |
| Transfer | 复用 `RecipientScene`、`AmountScene`、`ConfirmTransferScene`、`ReceiveScene` 和现有 ViewModel |
| Transactions | 接收统一 Store 的 BSV 记录，详情展示金额、费用、地址、txid 与状态 |
| 扫描与分享 | 调用现有 Core Payment/Receive 服务，二维码与剪贴板严格使用真实账户地址 |

发送流程为：选择 BSV → 收款人校验 → 金额 → 加载 UTXO/费用 → 展示收款人、链、金额和实际费用 → 认证 → Keystore 内部签名 → 广播 → 保存/恢复 pending → 查询确认状态。保留返回、取消、重试与重复点击保护。

BSV 真实收款可用后，Paymail 邮箱仍只是独立占位功能，不能进入 QR 或复制地址。原型多地址列表、MNEE 和兑换入口仍保持暂未开放。仅移除确实实现的 `TODO(WOWPAY-BSV)` 和对应文档记录，不整体解除所有延期功能。

用户可见字符串来自 Fluent 源及生成的本地化访问器。若复用组件 API 或样式有新增，实施时同步更新组件/Token 目录；已有组件够用则不新增抽象。

API 不支持 BSV 时复用现有 toast 呈现，错误由 Core 返回，iOS 只映射展示；不把断网、限流、未知错误一律解释为不支持 BSV，不让周期刷新反复弹出同一提示。按用户要求允许在实际缺口所有者处使用 `TODO(BSV-API)` 待实现标记，本文同步登记文件、API、受影响功能和处理现状，完成后同时移除标记与缺口记录。此标记要求只用于本期未支持的 BSV 请求，不扩展为通用代码注释例外。

## 7. 开发顺序与里程碑

| 阶段 | 实施内容 | 阶段退出条件 |
| --- | --- | --- |
| A：协议核对与冻结 | 确定链标识、网络、地址类型、默认路径与 WhatsOnChain 接线；请求真实数据后冻结费用与未确认策略，抓取脱敏 fixture | 开发所需契约有依据；费用规则准确可表达；限流计数范围与生产监测仍作为上线前待确认项 |
| B：Core 基础 | 注册链、地址与派生、公共节点默认/选择/检查、组件复用边界、Keystore 路由 | 派生/地址向量通过；BSV 默认 WhatsOnChain，选择与持久化生效；旧链输出无变化 |
| C：查询与交易 | provider/mapper、节流与读取重试、UTXO 规划、冻结后的费用、签名、广播与同一 txid 查询 | 确定性与真实网络只读检查通过；BSV 签名向量独立验证；记录服务指标 |
| D：服务与绑定 | 复用本地资产初始化、历史来源、公共 pending/重启恢复、生成 iOS 模型和 Rust 静态库 | iOS 能创建/恢复 BSV 账户并调用实际 Core 实现；共享接口扩展有实际限制依据 |
| E：iOS 集成 | 保留交互，接入真实资产、收款、转账与记录；不支持的 API 通过 toast 和待实现标记反馈 | 核心流程可用；API 缺口有明确反馈，不显示伪造结果 |
| F：回归验收 | BTC/BCH 等旧链回归，实际收发与异常测试，极端故障说明和两轮清理 | 基础功能有证据；未修复隐患逐项说明，不把小规模验证称为规模化上线验收 |

每阶段保持可检查的改动范围。先 Core 及其测试，再生成产物，最后 iOS 集成。验证前清理重复路径、无消费者的公共 API 与 fixture，验证后逐项复审；清理影响源代码时重跑受影响检查。

基础范围初步按 2～4 周估算，前提是开发者熟悉仓库及 UTXO；该估计不是工期承诺。历史同步与原生资产初始化涉及服务扩展，阶段 A 完成后需按实际复用边界重新估算。

## 8. 回归测试矩阵

### 8.1 Core 确定性测试

| 编号 | 场景 | 必须验证的独立结果 |
| --- | --- | --- |
| C01 | 默认路径派生与重新导入 | 与独立公开向量的公钥/地址一致，重复导入得到同一账户 |
| C02 | 地址与网络 | 错误校验和、网络、脚本类型被拒绝；共享传统地址不被误判为唯一链 |
| C03 | provider wire 与 mapper | 路径、请求体、单位、金额与分页符合 fixture；非法金额、溢出、未知状态显式失败 |
| C04 | 余额与 UTXO | 确认/未确认/可用区别明确，分页无遗漏，重复 outpoint 不被计入两次 |
| C05 | 单输入、多输入和找零 | 收款准确，找零属于账户，输入=收款+找零+费用，边界不溢出 |
| C06 | 发送全部余额 | 扣除真实费用，输出大于零且满足政策，无意外剩余或额外输出 |
| C07 | 手续费边界 | 低于 1 sat/byte 的有效报价、精度、向上取整、报价失效及不足付费都有确定性用例 |
| C08 | 独立 BSV 签名向量 | 原始签名交易字节、txid 与参考一致；金额/脚本/输出改变后签名不再验证通过 |
| C09 | 无效与被花费输入 | 空 UTXO、重复输入、错误私钥归属、金额不符、花费冲突明确失败 |
| C10 | 广播异常 | 已知交易、超时后实际已接收、拒绝、费用不足与查询未找到分别处理 |
| C11 | pending 持久化与故障边界 | 正常保存、重启重建与状态恢复通过；Store 写入失败/进程退出记录现有行为、恢复依据是否保留和未修复风险，不宣称未实现的零丢失保证 |
| C12 | 历史同步 | 分页、重复刷新、自转账、找零、跨来源合并、游标隔离与错误传播正确 |
| C13 | 钱包补链与权限 | 补链幂等，失败保留钱包，观察钱包不能签名，认证取消不能广播 |
| C14 | 无服务器 BSV 元数据/行情 | 本地资产初始化、余额、历史和发送仍可用，不显示虚构估值 |
| C15 | 原有链回归 | BTC/BCH 签名向量、地址派生、费用与付款路由不变；受影响 LTC/DOGE/ZEC 一并检查 |
| C16 | 请求节流与读取重试 | 所有 BSV 请求共享节流，读取重试有上限，限流反馈正确，广播超时查询同一 txid 且不自动创建另一笔支付 |
| C17 | 公共节点管理 | BSV 默认 WhatsOnChain；持久化选择传递给 provider 与 Client；节点检查使用所选地址，不兼容协议有错误；其他链默认选择不变 |

单元测试通过 fixture 或注入的 Client/Store 执行，不启动临时 HTTP 服务。向量来自公开测试材料或明确为测试生成的数据，不包含真实钱包秘密。对新增金额/安全规则按规范临时反转规则确认测试失败，再恢复；签名字节测试与跨实现验证保护协议契约。

### 8.2 真实网络只读测试

通过显式只读 example 和公开 API 查询对主网公开测试地址检查余额、UTXO、历史分页、已知交易、费用来源和节点身份。只读测试不加载真实私钥，不自动发送资金。公开地址 fixture 标注所属网络、用途与核对日期。

断言稳定协议约束，不对实时余额/区块高度写死数值，也不比较不同时刻查询的易变金额。实际网络错误、限流和 fixture 失效均如实记录。`--no-run` 只能证明可编译，不能证明服务可用。

### 8.3 iOS 操作回归

| 编号 | 操作 | 预期结果 |
| --- | --- | --- |
| I01 | 新建、默认路径导入、旧多链钱包补充 BSV | 有真实账户；重启后账户一致，失败有反馈 |
| I02 | 观察钱包及切换钱包 | 观察账户可查询/收款，不能签名；显示的数据与当前钱包一致 |
| I03 | 余额刷新、断网、恢复网络 | 成功显示真实数量，失败不显示假零值，恢复可重试 |
| I04 | 收款 QR、复制与分享 | 三者为同一真实账户；扫码仍需明确网络，Paymail 不混入 payload |
| I05 | 普通发送、多输入、全部发送 | 确认值与签名交易一致；有真实 txid 和状态 |
| I06 | 错误地址、零金额、超 8 位小数、费用不足 | 明确拒绝或进入现有补全流程，不静默舍入后提交 |
| I07 | 认证失败/取消、反复点击、返回 | 不广播，或只提交同一已确认交易；不创建重复支付 |
| I08 | 预估后 UTXO/费用改变 | 重新加载并明确重新确认，不能偷偷改变已确认金额 |
| I09 | 广播超时、写入失败、App 重启 | 正常重启可恢复记录与状态；超时查询同一 txid；写入失败等极端窗口如实登记隐患，不诱导重复转账 |
| I10 | BTC/BCH 等现有收发与扫描 | 原有地址、费用、确认/认证流程和链选择正常 |
| I11 | 无 BSV 行情、无后端链支持 | 数量收发功能可用，价格保持未知/不可用 |
| I12 | 现有 WowPay 延期入口 | 多地址、Paymail、MNEE、兑换仍无虚假成功操作 |
| I13 | API 不支持 BSV | 出现本地化 toast，具体调用/错误映射有待实现标记；不支持、限流和断网分别呈现，不反复弹出同一提示 |
| I14 | BSV 公共节点选择 | 默认服务、用户改选、节点检查和重启恢复沿用现有交互；后续查询、费用与广播使用所选服务 |

先在模拟器检查界面、导航和注入错误分支，再在实际设备检查真实认证与收发。模拟器通过不代表生物识别或设备安全路径已验证。

### 8.4 实际转账验收

使用独立专用测试钱包 A/B，确认网络、地址、资金来源和测试金额上限。测试网使用隔离配置及测试币；它验证的结果单独记录。生产 BSV 主网支持还需要在限定测试资金下完成主网小额收发，不能用测试网或本地签名代替主网兼容性结论。

1. A/B 都准备实际地址；从已知来源向 A 收款，确认 A 的余额、UTXO 与入账记录一致。
2. 从 iOS A 向 B 发出普通 P2PKH 转账，保存确认页面的链、地址、金额和费用，核对实际交易输出一致。
3. 记录本地计算与广播返回的 txid，确认相符；核对独立浏览器/节点能查到交易及 B 的目标输出。
4. 观察 pending → 链上确认，核对 A 找零、B 入账和两边记录。等待不足、供应商未索引或失败均记为待验收/失败，不能记为通过。
5. 在足够的受控 UTXO 下完成多输入、找零后再次发送，以及发送全部可用余额。精确核验固定测试交易的费用守恒，不只看弹窗提示。
6. 重启 App、重新导入同一测试助记词的默认账户，确认地址、余额与链上历史可恢复；涉及秘密的页面禁止截图、录屏或写入报告。
7. 异常广播、Store 失败、认证取消和已花费输入优先用注入测试复现，避免通过真实资金制造双花或重复支付。

正式回归报告只保留命令、环境、非秘密测试地址、txid、结果、必要脱敏截图和失败原因。助记词、私钥、密码、解密数据及可复用凭据不得进入仓库、日志或截图。

## 9. 验证命令与记录

以下命令用于实现与回归。实际执行结果见第 11 节；须确认命令运行了预期测试而不是零用例。

在 `core/` 执行：

```sh
just test bsv_home_bsv
just test bsv_home_bitcoin
just test bsv_home_derivation
just test bsv_home_core
cargo clippy -p bsv_home_bsv -p bsv_home_bitcoin -p bsv_home_derivation -p bsv_home_core --all-features --all-targets -- -D warnings
just format
cargo run -p bsv_home_bsv --example read_only --features reqwest
```

`just test <CRATE>` 当前 recipe 使用 `--all-features` 并跳过名字包含 `integration_tests` 的用例，所以真实网络测试必须另外执行。改动 `primitives`、`bsv_home_keystore`、`signer` 等 crate 时也运行其对应测试与 clippy。格式化后检查是否产生无关改动，只保留本任务范围。

在仓库根目录执行 iOS 定向生成、构建和测试：

```sh
just ios generate-models
just generate-core
just ios build
just ios test TransferTests
just ios test BsvHomeCoreServicesTests
just ios test
```

这些命令分别生成 iOS TypeShare 模型、UniFFI 及 Rust 静态库，再编译和测试实际 App。生成文件不能手工修改。当前根 `just generate` 同时生成另一平台，本阶段选择上述 iOS 定向命令；本地化输入变化时执行 `just ios localize`，记录共享输入对另一平台未同步的差距。

目标测试需检查 Xcode 测试计划确实包含该 target。只编译、零用例、过滤错误、环境失败和通过分别记录。操作回归按第 8 节执行；若后续采用 Maestro，先遵循仓库 [UI 测试规范](../skills/testing-maestro.md)，自动化流程不携带真实秘密或自动花费生产资金。

实施交付时更新 [FEATURES](FEATURES.md)、[WowPay 接入状态](WOWPAY_UI_REFACTOR.md) 以及实际改变的组件/Token/Payment 文档，仅记录已落地能力。

本文件作为文档的校验：`git diff --check`、本文件局部 whitespace 检查、相对文件链接与命令路径检查。文档通过不代表任何上述运行时验收通过。

### 回归记录模板

| 项目 | 填写内容 |
| --- | --- |
| 基线 | 分支、提交、工作区改动、日期 |
| 环境 | Rust/Xcode、设备与 iOS 版本、网络、provider、fixture/向量来源与版本 |
| Core 确定性 | 具体命令、feature、用例数量、成功/失败及日志位置 |
| 网络只读 | 具体命令、地址/已知交易、结果与异常 |
| iOS | 生成、构建、测试命令及实际设备操作结果 |
| 实际转账 | 网络、A/B 地址、金额、实际费用、txid、广播与确认时间、链上核验依据 |
| 故障注入 | 广播超时、记录失败、认证取消、冲突输入与恢复结果 |
| 服务表现 | 限流次数/比例、响应时间分布、广播失败率及超时未知结果；计数范围核实进度、生产监测与迁移待定项 |
| API 缺口 | 待实现标记所在文件、API、受影响功能、toast 行为和未完成原因 |
| 极端故障隐患 | 现有公共逻辑的失败窗口、实际恢复结果、建议改动及待确认范围 |
| 旧链回归 | BTC/BCH 和实际受影响家族成员的测试结果 |
| 未完成项 | 跳过的用例、未测试分支、环境阻塞、历史记录范围与另一平台差距 |
| 判定 | 可继续开发 / 待验收 / 验收通过；依据与仍存风险 |

## 10. 完成标准与当前状态

- [x] BSV 注册与能力由 Core 统一声明，iOS 能展示真实原生资产且不依赖行情。
- [ ] BSV 纳入公共节点管理，默认 WhatsOnChain；用户选择、节点检查、持久化及 provider 实际请求地址一致。
- [ ] 默认派生/恢复、观察钱包及既有助记词钱包补链可用，错误不丢失。
- [ ] provider 协议、余额、UTXO、费用、历史分页与交易状态有确定性及真实网络证据。
- [x] 费用精度根据真实响应与供应商契约冻结，解析、规划、确认、签名及展示使用一致规则。
- [ ] WhatsOnChain 请求节流、读取重试和广播超时同一 txid 查询可用；小规模服务指标有记录，上线前额度计数与监测迁移条件明确列项。
- [x] 单输入、多输入、找零、全部发送与 BSV 独立签名向量通过，费用和金额守恒。
- [ ] 私钥仍留在 Keystore 内部，认证/取消/错误路径与临时秘密清理保持有效。
- [ ] iOS 的收款、确认、发送、pending 恢复和历史重建通过实际操作回归。
- [ ] 限定资金的主网实际转账达到链上确认，收款输出、找零及记录均已核验。
- [ ] BTC/BCH 等受影响旧链通过回归；两轮清理和必要检查完成。
- [ ] 超时、记录失败、被花费 UTXO、未确认策略等高风险分支已测试或明确标为未完成。
- [ ] 不支持 BSV 的 API 有本地化 toast 和具体待实现标记；极端故障的公共逻辑隐患、改进方案与未完成项如实记录。
- [ ] 行情、兑换、Paymail、多地址等延期能力仍保持明确边界，文档与实现一致。

上述清单是整体验收标准，未勾选项包含尚未执行的操作回归。第 11 节区分已实现、已验证与未完成；本次交付不宣布主网收发或上线容量验收通过。

## 11. 首期实现与验证记录（2026-09-28）

### 11.1 已接入的执行链路

- `primitives` 注册 `BitcoinSv/bitcoinsv`、8 位原生币、Bitcoin 家族能力、WoC 浏览器及节点配置；原生收发可用，Token/Swap/NFT/DeFi/Memo 不开放。
- `bsv_home_derivation` 使用 `m/44'/236'/0'/0/0` 和压缩公钥主网 P2PKH；公开测试助记词向量由独立 `@bsv/sdk@2.0.0` 核对，地址为 `1K6LZdwpKT5XkEZo2T2kW197aMXYbYMc4f`。签名分发仍在 Keystore 内，未给 Swift 增加私钥接口。
- 公共节点规则默认 `https://api.whatsonchain.com/v1/bsv/main`，已保存的用户选择保持优先；Gateway 工厂将所选地址交给 `BsvClient`。节点检查读取主网标志及分叉后高度 556767 的 BSV 检查点，不只依赖共用创世块。
- 查询、UTXO 预加载、费用、ForkID `0x41` 签名、广播与状态查询由 `bsv_home_bsv` 实现。复用 `bsv_home_bitcoin` 的策略式规划和 ForkID 签名构件，没有把 BSV 当成 BCH。共享 planner 修复了“输入足够支付无找零费用、但不足支付带找零费用”时过早报余额不足的边界，旧链测试继续通过。
- Core 原生资产元数据本地生成，不依赖服务器提供 BSV；没有编造行情。`BsvHomeTransactionsService` 通过 Gateway 读取 BSV 地址历史、保存现有交易 Store 并登记 pending 追踪。混合钱包仍使用原有设备 API 获取其他链历史，BSV 同步不推进服务器历史游标。
- BSV 原生资产与官方 TRON TRC20 USDT 的美元单价通过 DefiLlama `prices/current/coingecko:tether,coingecko:bitcoin-cash-sv` 查询，在既有资产刷新流程写入行情 Store，用于单价与美元资产估值。保留供应商时间戳；网络失败、缺失币种或无效报价不清空缓存，旧服务器行情不能覆盖这两个资产。该接口不提供 24 小时涨跌幅，iOS 详情显示占位；图表和 BSV 行情提醒仍不开放。
- iOS 生成模型、链图标、服务注入及 `sat/KB` 展示已接入；钱包使用真实资产行进入现有详情、Receive 与 Transfer 流程，移除重复 BSV 占位资产行。原型 Paymail、多地址、MNEE 和兑换保持延期。
- WoC 广播是 JSON 请求体加文本 txid 响应，公共 JSON-only client 原先无法表达。最小扩展为内部 `Client::get_text/post_text`，由 Reqwest/RpcClient/MockClient 实现，保留现有 JSON 方法。BSV 读取也使用严格 HTTP 状态检查的文本解码后解析 JSON，避免 HTTP 失败被有效 JSON 数据掩盖；未增加新的 iOS RPC API。
- Core 定时等待提取自已有 `services/clock.rs` 到 `bsv_home_client::sleep`；共享调度器使用 executor-independent 的 futures mutex，iOS 不需要额外 Tokio runtime。非 Tokio 的历史服务测试覆盖真实 Core 调用链。

### 11.2 已冻结的费用与数据规则

2026-09-28 真实 `/feerecommendation` 返回 `{"fee_unit":"sat/KB","fee":100,"mempool_min_fee":100}`，已保存为 [fixture](../core/crates/bsv_home_bsv/testdata/fee.json)。结合 [WoC 官方契约](https://docs.whatsonchain.com/chain-info)，首期接受正整数 `sat/KB`，1 KB 按 1000 字节；选择报价与 mempool 最低费率中的较大值。此样本不证明未来永远只有整数，未来出现小数或新单位时显式拒绝，不截断、不变成零费率，也不擅自接受新的精度规则。

规划与签名共用 `ceil(rate × estimated_bytes / 1000)`。金额使用整数 satoshi；JSON 金额先保留十进制原文，再严格转换 8 位精度，科学计数法也可精确处理，超精度与溢出拒绝。P2PKH 输入按保守 148 字节估算，输出按实际序列化长度，计入版本、locktime 与 CompactSize 计数；签名长度差异可能使实际支付费用比精确 raw bytes 的最低费用多少量 satoshi，确认页显示的是实际规划费用。只提供供应商确实支持的 Normal 档，没有虚构 Fast。签名再次验证已确认费用、原生费用资产、费率与 Max 净金额一致；每次加载查询费用，不使用跨流程报价缓存。

余额与转账预加载共用 `native_unspent` 校验原生输出。首期仅花费已确认、未被 mempool 花费的 P2PKH UTXO；未确认原生输出计入 pending，暂不花费。前序金额、脚本和正确认数从所选服务的交易 JSON 核对，发现已确认输出的确认数丢失/为零时拒绝继续；coinbase 不足 100 确认同时排除于余额与可花费列表。已被 mempool 花费、1 satoshi、已识别铭文及共签 token 输出不计入原生余额和转账输入，未知或矛盾输出显式失败。sequence 为 `0xffffffff`，最小输出 1 satoshi；不继承 BTC 的 546 sat dust 或 RBF 策略。矿工接收策略可能更严格，需在受控小额主网验收核实。

UTXO 按 [WoC 官方合并查询契约](https://docs.whatsonchain.com/un-spent-transaction-outputs#get-unspent-utxos-by-address)调用 `GET /address/{address}/unspent/all`，取得 confirmed 与 unconfirmed 输出；实际响应游标为 `nextPageToken`，下一页使用同一 `/unspent/all?token=...` 端点。每条输出必须提供 `status`，只接受 `confirmed` 或 `unconfirmed`，不能根据请求路径补确认状态。已确认输出还必须提供正高度。重复 outpoint 显式失败，不静默去重或重复累计余额。

历史按 [WoC 官方 Bulk History 契约](https://docs.taal.com/core-products/whatsonchain/address#bulk-history)调用 `POST /addresses/history/all`，请求体只包含当前地址，分别取得 confirmed 与 unconfirmed 首批记录及独立游标。只接受一个与当前地址相符的响应条目，两组 `error` 均须为空；每组有游标时从对应 `GET /address/{address}/confirmed/history` 或 `GET /address/{address}/unconfirmed/history` 继续，使用 `token` 参数，不重新读取旧端点首页。合并去重 txid 后继续现有交易详情映射及 Store 保存。

UTXO 与历史每组的第一页均计入 100 页上限；重复游标、超过页数上限、未确认响应达到供应商 100000 条边界、协议错误及不完整结果均显式失败。HTTP 404 不解释为零余额或空历史；仅成功且校验通过的空数组代表没有对应记录，刷新失败保留旧缓存。当前全量刷新在长历史账户上可能较慢，未实现增量游标或跨交易来源缓存。

2026-10-03 公开只读探针验证了 `/unspent/all` 的同端点分页：官方样例地址连续 6 页各返回 200 与 1000 个 confirmed 输出，游标逐页变化，6000 个 outpoint 无重复；第 6 页仍有游标，本次仅验证有界分页，没有遍历完整地址。另一公开 mempool 地址首页返回 1000 confirmed 与 2 个 unconfirmed，随后两页各返回 1000 confirmed，没有重复未确认输出；续页后重查首页，原 2 个 unconfirmed 仍存在。该未确认样本均为 1 satoshi，不证明普通非 1 satoshi 未确认金额或主网花费策略已经完成实际验收。探针期间遇到过 429，退避后成功，不代表持续服务或容量保证。

曾实测一个公开前序交易 `/hex` 返回约 19 MB 文本，而 JSON 约 201 KB；因此不下载整笔大型前序 raw transaction，只请求 JSON 金额/脚本，并检查返回 txid。此处信任所选服务的数据完整性，不构成 SPV 或对前序完整交易重算 txid 的证明；用户换节点不会改变这个信任边界。被供应商截断的自身前序脚本拒绝花费；历史中非地址输出及被截断的大脚本不投影成地址，金额仍计入总额/费用并记诊断，不生成虚构收款地址。

### 11.3 请求、广播与恢复边界

同一 Core 进程的所有 BSV provider 共用调度器，每次请求预留至少 400 ms 间隔，约 2.5 请求/秒；切换或新建 provider 不重新获得独立额度。免费 3 请求/秒的实际聚合范围仍未确认，此本地节流不能保证共享出口额度。只读 GET 与历史查询 POST 共用读取重试，仅对 timeout/network/429/5xx 最多执行 3 次，退避 1/2 秒；永久 HTTP 与协议解析错误不重试。底层 BSV Reqwest 自动重试关闭；广播 POST 仍只执行一次，不使用只读历史 POST 的重试路径。

广播前由签名 bytes 计算 txid；错误后查询同一 txid，查到则返回原 txid 并进入公共 pending 保存/追踪，未查到则保留错误，不另造交易或重播。成功响应必须等于本地 txid。请求诊断通过现有 `bsv_home_tracing` 记录 operation、attempt、elapsed_ms、success、limited，不记录密钥、完整交易、地址或请求体。生产统计出口、窗口、阈值及迁移负责人仍需上线前确定，当前日志不是完整生产监控系统。

正常提交仍复用现有 `BsvHomeConfirmService`、Store 和 iOS 追踪/重启恢复路径。以下窗口尚未修复或进行完整 BSV 故障回归，按用户约定保留公共逻辑并明确登记：

| 边界 | 当前行为与待验证事项 |
| --- | --- |
| 广播成功但 Store 写入失败 | 公共 `store_pending` 的 `unwrap_or_default` 可能隐藏记录错误，记录和追踪可能缺失；建议后续在公共所有者增加持久化失败反馈及恢复依据 |
| 广播超时且多次查询仍未索引 | 当前错误模型只返回广播错误，未持久化“结果未知”的 txid/签名恢复记录；不能据此断言支付失败或鼓励再次支付 |
| 广播后、记录前退出进程 | 未增加广播前持久化意图；需受控故障测试，不能宣称零丢失 |
| 正常 pending 重启 | 使用既有持久化和状态追踪机制，公共回归测试通过；完整 BSV App 退出/重启操作尚未执行 |
| coinbase 与余额 | 余额与转账输入选择共用 `native_unspent`，不足 100 确认的 coinbase 同时排除；成熟与未成熟 coinbase 的真实主网操作回归仍待验收 |
| 任意脚本与多收款人历史 | 只开放原生 P2PKH 收发；大脚本不能映射地址时不伪造参与人，空输入地址的收款按本钱包输出投影；多收款人出账仍沿用公共 UTXO 展示规则，未宣称完整导入任意 BSV 钱包历史 |
| 混合钱包设备订阅/历史 | BSV 不依赖后端历史；其他链和设备订阅继续现有链路，本次未部署后端 BSV 支持，混合钱包真实后端兼容仍需操作回归 |

### 11.4 不支持的 API 登记

| 所有者与标记 | 缺口 | 本期反馈 |
| --- | --- | --- |
| `core/bsv_home_core/src/services/assets/mod.rs`，`TODO(BSV-API)` | BSV token 元数据 | `Unsupported`，对应 UI 用既有本地化 toast；原生 BSV 本地元数据可用 |
| `core/bsv_home_core/src/services/transactions/mod.rs`，`TODO(BSV-API)` | BSV token 历史 | `Unsupported`，交易页已有错误 toast；原生历史使用 WoC |
| `core/bsv_home_core/src/services/chart/mod.rs`，`TODO(BSV-API)` | BSV 行情图 | `Unsupported`，图表状态及 toast；不显示假图表 |
| `core/bsv_home_core/src/services/price_alert/mod.rs`，`TODO(BSV-API)` | BSV 行情提醒、提醒同步 | `Unsupported`，资产操作/同步错误 toast；不向不支持的设备 API 发起该链提醒请求 |

`BSV_API_UNSUPPORTED:` 只用于这些已知缺口，经 iOS `BsvHomeServiceError` 映射到既有 `Localized.Errors.Connections.unsupportedChain`。断网、429 和未知错误保留原类别。原生资产的自动价格 stream 请求跳过；用户明确请求图表/提醒才显示不支持，避免仅打开资产就反复弹无行情提示。新增缺口应在实际所有者及此表登记，不能用一个通用“不支持”遮盖所有网络故障。

### 11.5 验证结果与剩余验收

本次执行结果如下；Core 数量按各 crate 最新有效结果列出，不重复累计复跑次数。

| 检查 | 实际结果 |
| --- | --- |
| `cd core && cargo test -p bsv_home_bsv -p bsv_home_bitcoin -p bsv_home_derivation -p primitives -p bsv_home_client -p bsv_home_jsonrpc -p chain_traits -p bsv_home_core --lib --all-features -- --skip integration_tests`，及清理后相应定向复跑 | 全部通过：`bsv_home_bsv` 最新 17、`bsv_home_bitcoin` 58、`bsv_home_derivation` 14、`primitives` 260、`bsv_home_client` 6、`bsv_home_jsonrpc` 12、`chain_traits` 9、`bsv_home_core` 668，共 1044 个确定性用例；9 个既有 Bitcoin live integration 用例按命令跳过 |
| `cd core && cargo clippy -p bsv_home_bsv -p bsv_home_bitcoin -p bsv_home_derivation -p bsv_home_client -p bsv_home_jsonrpc -p chain_traits -p primitives -p bsv_home_core -p settings_chain --all-features --all-targets -- -D warnings` | 通过；最后 provider 校验/投影变更后对 `bsv_home_bsv`/`bsv_home_core` 定向复跑通过 |
| `cd core && cargo test -p bsv_home_core --lib services::transactions::tests::bsv_history_reaches_existing_store_without_backend_indexing` | 通过，1 个用例；`futures::executor::block_on` 下默认节点 → provider → 交易 Store，不依赖 Tokio runtime 或后端 BSV 索引 |
| `just ios generate-models`、`just generate-core` | 通过；TypeShare/UniFFI 按源生成，模拟器及设备 debug 静态库均生成，未手写生成文件 |
| `just ios build` | 通过，最后 Core 源变更后再生成静态库并重建 |
| `just ios test TransferTests` | 140 通过，0 失败/跳过 |
| `just ios test BsvHomeCoreServicesTests` | 42 通过，0 失败/跳过；包括补齐 BSV 独立预期地址后的实际 Keystore 多链导入/派生 |
| `just ios test PrimitivesComponentsTests` | 177 通过，0 失败/跳过；包括整数 `sat/KB` 展示、BSV 不支持错误本地化及 429 类别保留 |
| `just ios test MarketInsightTests` | 4 通过，0 失败/跳过；现有市场字段映射，不代替 BSV 图表 toast 操作回归 |
| `cd core && cargo run -p bsv_home_bsv --example read_only --features reqwest` | 真实 WoC 查询通过；最新运行读取主网检查点、余额和 fee，5 次查询，单次响应约 378～1052 ms，0 重试/限流；其它协议核对中遇到过 429，不代表长时间服务稳定 |
| scoped Rustfmt、SwiftFormat 与 `git diff --check` | 通过；只格式化本任务文件，未执行会重写已有无关改动的整仓格式化；本文件相对链接均存在，两轮清理完成 |

初次 iOS 测试编译暴露 `LocalKeystoreTests` 穷尽枚举缺少 BSV，已补独立 SDK 预期地址后重跑通过；初次 Clippy 暴露新增模块布局和测试模块之后的定义位置，已修正重跑。新增 BSV signer 使原有 `bsv_home_cardano` 无 RPC feature 编译路径暴露其无条件使用 `num_bigint` 却将依赖设为可选的问题，做了最小 Cargo 依赖修正，未改变 Cardano 业务行为。

已独立核对 `@bsv/sdk@2.0.0` 的 raw bytes/txid，测试 fixture 的 txid 为 `9d538f86210d2f28106524d607c6fbc3ae4632af8dc244e04452de906628caab`；该向量使用公开静态测试数据，未广播。临时将向上取整改为向下取整后，费用边界测试按预期失败（0 对 1），随后恢复原规则并重跑。

真实只读 example 已验证 BSV 主网检查点、余额和实际 fee quote；公开地址为 `1L2F8wYxTRagCZLnsm2engg8ngGECSeuE5`，一次查询 available 为 16390980 satoshi、pending 为 0，数值仅记录当次结果。外部只读契约核对还覆盖历史/UTXO 实际分页，期间观察到 HTTP 429。未发送资金，未验证矿工实际接收或 pending 到确认的主网全链路。

未完成：第 8.3 节完整 iOS 操作矩阵、真实设备认证/取消、BSV 普通 pending 重启及上述故障窗口、小额主网 A/B 收发与重新导入核对、生产限流计数和监测迁移条件。首次实现未同步 Android 模型/绑定和业务入口，也未执行 Android 构建；第二轮回归修复中的共享兼容适配与实际检查见第 11.6 节，不声称 Android BSV 已可用。所有变更留在当前工作区，未提交、发布或部署。

### 11.6 第二轮回归问题修复（2026-09-28）

第二轮模拟器回归发现的三个问题已完成代码修复，修复版本的界面复核在该阶段尚未完成（后续复核见第 11.7 节）：测试结束后原 iPhone 17 设备处于 Shutdown，安装返回 CoreSimulator code 405，自动控制选择 Simulator 连续超时；用户确认暂时无法打开窗口。最终 Debug App 已构建，未清空模拟器或删除原钱包数据，未执行实际签名或广播。

| 问题 | 修复及范围 |
| --- | --- |
| R2-01：原生金额超精度被截断 | Core 金额输入对 BSV 原生数量使用现有精确转换，不能表示为整数 satoshi 时返回 `InvalidNumber`；iOS 使用现有本地化“数额无效”，继续禁用。有余额的 Core/iOS 测试覆盖 `0.000000019` 拒绝与 `0.00000001` 接受。多余的末尾零不改变金额，例如 `1.000000000` 仍精确等于 1 BSV。BTC 等已有链的截断行为保留并有对照测试；BSV 费率与签名规则未改变 |
| R2-02：无行情仍可切到法币并显示假零 | `BsvHomeAmountType.can_switch_input_type(price, input_type)` 在 Core 判断；只有有限正价格允许从原币切到法币。iOS 缺少 equivalent 时显示现有本地化 Not Available，不显示 `¥0.00`；动作入口不再展示，直接调用也不会清空原币输入。价格在法币输入期间消失时仍允许返回原币，不把用户困在无价格模式 |
| R2-03：空历史提示未实现的购买/兑换 | 既有空状态组件在 buy/swap 动作均为空时使用本地化 Receive 标题；BSV 因 Core 能力声明得到收款提示，不新增 iOS 链名单。观察钱包描述保留 |

该共享切换接口的 Swift/Kotlin 调用已同步。生成 Android 类型时补齐了新增 `SatKb` 在费用显示中的映射，单位标签在 31 种语言的 Fluent 源统一增加 `fee_rate_satkB = sat/KB`，通过 `just localize` 生成两端资源，iOS 同样复用该标签。此处仅保证共享接口/类型兼容，未宣告完整 Android BSV 接入。

| 本轮实际执行 | 结果 |
| --- | --- |
| `cd core && just test bsv_home_core` | 669 通过，0 失败/跳过；精度和价格切换规则分别临时反转时，对应定向测试确实失败，恢复后全量通过 |
| `cd core && cargo clippy --locked -p bsv_home_core --all-features -- -D warnings` | 通过 |
| `cd core && just format`；scoped SwiftFormat | 通过；清理了整仓格式化引入的 38 个此前干净文件的无关格式变化，只在确认 rustfmt 输出与当前内容完全一致后恢复它们 |
| `just generate` | iOS/Android TypeShare、Swift/Kotlin UniFFI 及 iOS simulator/device 静态库已生成；整条命令在 Android native build 阶段因缺少 `cargo-ndk` 失败，本机 NDK 目录亦不存在，不能记为整条生成通过 |
| `just localize` | 通过，两端资源已重新生成 |
| `just ios test TransferTests` | 143 通过，0 失败/跳过，包括有余额的超精度拒绝、无价格保留原币输入、价格消失后返回原币 |
| `just ios test PrimitivesComponentsTests` | 177 通过，0 失败/跳过 |
| `just ios test PrimitivesComponentsTests/FeeUnitViewModelTests` | 标签生成与复用后再跑：4 通过，0 失败/跳过；这 4 项是上述组件套件的定向复查，不额外累加唯一测试数 |
| `just ios build` | 最终本地化/费用标签更新后再次构建通过 |
| Android 金额/费用模块定向编译 | `transfer_amount:presents` 和 `confirm:presents` 的 `compileDebugKotlin` 通过 |
| Android `transfer_amount:viewmodels:testDebugUnitTest` | 40 通过，0 失败/跳过 |
| Android `confirm:viewmodels:testDebugUnitTest` | 原环境两项失败，差异为 `$` 与 `US$`；固定 `JAVA_TOOL_OPTIONS='-Duser.language=en -Duser.country=US'` 后 11 通过，0 失败/跳过，包括整数 `sat/KB` 展示用例。未修改两项失败的断言 |
| `just android build` | 失败：缺少 `cargo-ndk`；初次并行检查还触发 Kotlin 缓存冲突，后续定向检查已串行重跑，不能把定向通过当成整 App 构建通过 |
| `git diff --check` | 通过；两轮检查已核对改动范围、生成资源及错误/金额路径，无新增秘密存储或认证绕过 |

该阶段修复版模拟器操作尚未完成，后续可操作路径复核见第 11.7 节；真实设备和限定资金的主网收发仍待验收。第 11.3 节已有的 Store 写入失败、广播后退出与非空 pending 重启隐患仍然存在，未在本轮修复，也未跳过或吞掉新错误来宣称通过。


### 11.7 修复版模拟器补充复核（2026-09-28）

原 iPhone 17 恢复 Booted 后，已成功安装最终 Debug App，并比较安装版与本地构建的 `BsvHome`、`BsvHome.debug.dylib` SHA-256，两者均一致。使用 native CUA 复核 R2-01～R2-03：非零第九位小数显示“数额无效”并禁用继续；1 sat 通过精度检查后因零余额显示余额不足；无行情显示“无法使用”，点击原副值区域保留 BSV 原币输入；空历史显示“接收 BSV”。三项可操作 UI 复核通过，没有发现新的 BSV 功能问题。

收款 QR 独立解码与屏幕地址、复制后原生粘贴出的完整地址一致，仍为原账户；分享仅检查截断预览，没有实际发送。行情图及此前未覆盖的铃铛入口均显示“不支持该区块链网络” toast，Paymail 保持“暂未开放”。默认 WhatsOnChain 节点检查成功，本次区块 968,753、1,203 毫秒；BTC 有行情时仍能切换法币并返回原币。结束 App 进程后从桌面冷启动，首页收款仍可取得同一 BSV 地址；没有重启或清空设备。

本轮没有业务源码变更，也没有重跑此前自动化套件。零余额环境没有进入确认/认证页，真实费用、签名广播、非空历史/pending 恢复、断网/限流故障、自定义节点路由、观察/多钱包、真实设备与 Android UI 仍未验证；第 11.3 节恢复窗口继续保留。详细步骤、截图、安装哈希和边界见[补充复核报告](../ios/build/bsv-regression-round2/fixes/recheck/REPORT.md)。

### 11.8 详情页 404 查询修复（2026-10-03）

部分空地址的 confirmed UTXO 与 confirmed history 拆分查询返回 404，而官方合并查询成功返回明确空结果。修复集中在 `bsv_home_bsv` 的端点、响应解码与分页，基础节点地址、账户、派生、签名和钱包 Store 保持现有契约；没有把未知 404 转为空数据，也没有用聚合余额替代校验后的原生可用余额。模拟器还暴露出公共余额服务同时查询质押余额，而 BSV provider 未声明无质押余额的缺口；现与 Bitcoin provider 一致返回 `Ok(None)`，避免成功取得原生余额后仍被默认 unsupported 错误阻止写入。协议与真实多页证据见第 11.2 节。

`cd core && cargo test --locked -p bsv_home_bsv --lib --all-features` 最终通过 33 个测试、0 失败，包含 RPC、signer 与 mapper；查询回归覆盖合并接口空结果、余额与转账输入共享规则、历史两组独立分页、首批页计数、字段与错误边界、只读 POST 重试及无质押余额。`cd core && cargo test --locked -p bsv_home_core --lib bsv --all-features` 通过 12 个 BSV 服务/Gateway 测试，覆盖空历史、错误不覆盖交易 Store、跨确认列表去重、Pending 跟踪、本币余额刷新和查询错误不写余额。`cd core && cargo test --locked -p bsv_home_core --lib services::balance::tests --all-features` 通过 13 个余额服务测试；此检查与上述 BSV 检查有重叠，数量不累计。余额 mock 只记录写入次数，未验证既存非零余额的完整数值保持。

`cd core && cargo clippy --locked -p bsv_home_bsv -p bsv_home_core --all-features --all-targets -- -D warnings` 最终通过；期间修正本次触及的余额测试文件中一处既有布尔断言 lint。`cd core && cargo run --locked -p bsv_home_bsv --example read_only --features reqwest` 通过，验证主网身份、公开空 fixture 的 available/pending 均为 0、历史记录数为 0，以及当次费率为 100 sat/KB。新增空余额端点回归在旧实现上按预期失败；历史地址校验变异测试按预期失败，恢复后通过。

`just ios generate-core` 与 `just ios build` 最终通过，重新生成 Swift 绑定并构建 iOS 模拟器/设备静态库，将模拟器 App 更新为本次 Core。既有 iPhone 17（iOS 26.4）账户未清空：两次从资产列表进入 BSV，检查余额概览及交易记录空状态，并执行下拉刷新。最终 App 进程运行日志出现 4 次 `/unspent/all` URL、没有旧 confirmed UTXO/history URL、没有资产详情 refresh failure 或 HTTP 404 响应。该环境余额与历史为空，因此真实非空历史/pending 的 App 显示、非零旧余额完整保持、签名广播及资金操作未验证；非空记录与 pending 的解析/保存/跟踪由确定性测试覆盖。

本次涉及文件的 `rustfmt --edition 2024 --config skip_children=true --check` 与 `git diff --check` 通过；`cd core && cargo fmt --all --check` 因本轮未修改的 `core/bsv_home_core/src/models/remote_types.rs` 和 `core/bsv_home_core/src/services/transaction_state/mod.rs` 既有格式差异失败，不能记为全仓格式检查通过。未操作 Android；共享 Core 对该平台的影响未验证。主网收发及第 11.3 节恢复窗口继续保持待验收。
