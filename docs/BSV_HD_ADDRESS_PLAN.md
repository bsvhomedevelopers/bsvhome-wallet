# BSV 多地址钱包调整方案

状态：Core 与 iOS 已实施，自动测试、App 构建及下述模拟器验收通过；主网收发验收待专用钱包。实施日期：2026-10-05 至 2026-10-06。

本文记录经用户确认的 BSV HD 钱包方案：可恢复、汇总钱包余额、跨地址转账，并在 iOS 的 BSV 详情页展示地址记录。下文保留实施前的缺口与设计依据；实际接口和验收记录见末尾。用户已确认本轮先完成自动测试与模拟器验收，主网小额收发待提供专用钱包后执行。

调查基于 `codex/wowpay-ui-refactor` 当前本地工作区，HEAD 为 `cbe66705b02d1142a813fc5170033c6625254ee1`。工作区含未提交的 BSV、Portal 及其他调整，因此本文以实际本地源码为准，不能仅从 HEAD 推断功能。既有首期接入范围见 [BSV 接入文档](BSV_INTEGRATION.md)，首期单地址实现作为调查基线保留，当前助记词钱包契约以本文末尾的实际实施记录为准。

## Android 对齐阶段 3 的当前变更

2026-11-15 的 [Android 对齐方案](WOWPAY_ANDROID_ALIGNMENT.md) 在阶段 3 调整普通钱包的收款选择：新建地址池正式发放 10 个收款地址，默认选择索引 0；用户可以切换到任一已发放地址，包括已使用地址，新增地址后选择新发放的末尾地址。刷新或收款事件不自动改变显式选择。钱包主账户、Portal 身份、余额归集、找零预留和恢复数据保持原有所有者。旧序列化数据没有显式选择字段时保留原自动轮换，首次手动选择或新增后进入显式选择模式，不清理旧地址或历史。

地址页仅展示已正式发放的收款记录，不能把恢复探测的空候选、备用地址或找零记录误当作可选收款地址。事件订阅覆盖普通及冷钱包已发放收款范围的并集；历史及余额仍覆盖全部拥有记录。无公开派生能力的钱包保持单一地址。尚未准备公开地址池的旧 HD 钱包不会因后台订阅而读取密码或触发认证，须通过既有显式准备流程建立地址池后订阅。

用户于 2026-11-15 明确选择按 Android 对齐：手动新增收款地址时，未确认历史也支持扩大 gap 20 的发放范围。Android 的已使用收款范围为持久化、单调增加的数据，未确认交易撤回或重组不回退它；iOS 的显式新增按相同语义使用已验证历史留下的已使用记录。没有历史时仍以 20 个为边界，查询失败不当成历史。该规则仅支配手动新增能力，旧自动补足以及找零和签名预留窗口继续使用原已确认历史限制。旧池第一次手动新增也适用新边界，并由该动作转入显式选择模式，无须先切换地址。

手动新增还采用 Android 的明确数量上限：`count < min(used_count + 20, 9980)`，最多发放 9980 个收款地址（索引 0 至 9979）。这仅限制新增动作，不将 Android 的全扫描限制搬入既有恢复服务；已存高索引记录仍可加载、展示、归属查询和订阅，选择入口的既有边界不变。本轮通过合法旧格式的高索引稀疏池验证数据保留，但未运行会物化近万个公开子地址的高索引选择。对应较低索引的真实选择、重启及主身份回归已执行。

已准备的 HD 钱包继续保留手动扩大扫描入口，即使自动扫描已完成 20/20、当前选择索引 0 且允许新增地址。当前窗口完成与完整资金恢复不是同一状态；该入口不受 `canAdd` 或 `currentAddress == nil` 门控，非 HD 钱包不展示。准备/重试期间继续扫描仍禁用，避免抢占认证或旧任务。此项保留原恢复合同，不复制 Android 简化页面造成的恢复能力缺口；相关 Asset/Receive 回归先红后绿，47 项逻辑测试 / 60 个展开用例通过。

以下自动轮换、备用数量及找零展示记录为旧方案和历史验收，不代表阶段 3 的显式选择行为已经通过新验收；阶段 3 状态以对齐方案的执行记录为准。

## 旧方案已确认的产品规则

| 事项 | 用户已确认的要求 |
| --- | --- |
| 初始化数量 | 默认生成 10 个收款地址；找零地址按需另行派生，不计入这 10 个 |
| 收款轮换 | 检测到有效的未确认收款即可切换到下一个未使用地址；之后不退回旧地址 |
| 地址池扩展 | 自动补足 10 个未使用收款地址；10 个是初始数量及备用数量，不是永久上限 |
| 恢复边界例外 | 达到可恢复的地址空隙边界时，允许暂停新增地址，备用数量可暂时少于 10 个；已用地址不退回 |
| 钱包范围 | 新建钱包、助记词导入钱包、本地已有助记词钱包 |
| 恢复范围 | 保证 WowPay 当前 `m/44'/236'/0'` 根下地址的恢复，不扩展其他 BSV 钱包路径 |
| 新设备发现策略 | 标准自动发现后提供继续扩大扫描入口，不增加附加备份文件；自动窗口不保证发现任意空隙之后的资金 |
| 旧钱包升级 | 首次使用 BSV 时认证，并完成公开派生能力及地址池升级 |
| 地址展示 | 在 BSV 币种详情的“地址 TAB → 地址记录”查看所有地址 |
| 编码时机 | 先完成并确认设计，再开始编码 |

“所有地址”在本设计中指正式加入本地地址池的收款与找零记录，包括当前、未使用、已使用及余额已花完的地址。未来尚未分配的无限派生空间不枚举；恢复期间临时探测、无历史且未纳入地址池的候选地址不成为展示记录。列表不能只保留有余额的地址，也不能只显示前 10 个。

范围限于 Rust Core、iOS、必要的共享模型、本地化及仓库文档。单地址观察钱包和单私钥钱包保留其原有能力；本次不增加 xpub 观察钱包导入、其他链 HD 管理、其他钱包路径扫描、分叉币拆币或代币支持。不检查、修改、生成、构建或测试 Android；共享接口对该平台的影响另列为未验证。

## 当前代码基础与缺口

| 当前所有者 | 已核对的能力或限制 | 本次调整 |
| --- | --- | --- |
| [bsv_home_derivation 的 BIP32](../core/crates/bsv_home_derivation/src/mnemonic/bip32.rs) | 内部支持指定路径派生；公开助记词接口仍每链一个默认账户 | 增加受限的 BSV 分支与索引派生、账户 xpub 和公开子地址派生 |
| [账户派生](../core/crates/bsv_home_derivation/src/mnemonic/derivation.rs)、[BitcoinChain](../core/crates/primitives/src/chain_bitcoin.rs) | BSV 不在此 `BitcoinChain` 枚举中，未进入 Bitcoin 家族 xpub 生成分支 | 在派生所有者处明确处理 BSV，不为取得 xpub 而假装成 BCH |
| [公钥字段赋值](../core/crates/bsv_home_derivation/src/private_key/import.rs) | BSV 当前 `extended_public_key` 实际是十六进制原始公钥，不是账户 xpub | 新地址池使用有明确类型的 xpub 元数据；不根据字段名称猜测内容，不直接覆盖旧字段语义 |
| [BsvHomeWalletService](../core/bsv_home_core/src/services/wallet/mod.rs)、[BsvHomeKeystore](../core/bsv_home_core/src/keystore/keystore.rs) | 已有创建、导入、重复钱包检测、缺链补齐及秘密存储边界；签名只按链取得默认路径私钥 | 在同一钱包域扩展初始化和幂等升级，签名在 Rust 内按输入派生密钥 |
| [iOS AccountRecord](../ios/Packages/Store/Sources/Models/AccountRecord.swift)、[资产关联](../ios/Packages/Store/Sources/Models/AssetRecord.swift) | 可存不同路径，但资产按链 `hasOne` 关联账户，业务通常取同链第一个账户 | 保留一个稳定 BSV 主账户，地址池单独保存，避免列表重复和关联歧义 |
| [余额服务](../core/bsv_home_core/src/services/balance/mod.rs)、[余额规则](../core/bsv_home_core/src/services/balance/rules.rs) | 同一 AssetId 的多次更新会赋值覆盖；部分请求成功时可能先写成功结果 | BSV 钱包级汇总后只写一份完整余额，不能以成功子集代替钱包总额 |
| [BSV provider](../core/crates/bsv_home_bsv/src/provider/chain_traits.rs)、[RPC 校验](../core/crates/bsv_home_bsv/src/rpc/client.rs) | 单地址 UTXO、余额和预加载，已有前序输出校验、分页、节流和读取重试 | 在现有 provider 上查询地址池，复用原生可花费规则，合并后按 outpoint 去重 |
| [BSV 签名](../core/crates/bsv_home_bsv/src/signer.rs)、[ForkID 构件](../core/crates/bsv_home_bitcoin/src/signer/transaction.rs) | 输入与找零须属于一个 sender，全部输入使用一个私钥 | 支持逐输入拥有者与密钥，确认计划显式绑定找零和费用 |
| [交易服务](../core/bsv_home_core/src/services/transactions/mod.rs)、[BSV mapper](../core/crates/bsv_home_bsv/src/provider/transactions_mapper.rs) | 按一个地址读取和投影历史 | 按钱包地址集合读取，txid 去重后投影一次，处理自转账与找零 |
| [Portal 支付](../core/bsv_home_core/src/services/portal/payment.rs) | 支付余额仍直接查询主地址，恢复仅用单个 sender 投影 | 复用钱包总余额和支出描述，保持认证身份及订单地址稳定 |
| [BSV 地址 TAB](../ios/Features/Assets/Sources/Scenes/AssetScene.swift)、[页面模型](../ios/Features/Assets/Sources/ViewModels/AssetSceneViewModel.swift) | 地址记录当前只有主地址一行，复制也绑定主地址；添加入口尚未开放 | 返回并呈现池内全部记录，每行复制自身地址，当前收款卡与二维码使用当前收款地址 |

技术基础足够开展调整，但“生成 10 个地址”不能单独作为交付。地址初始化、恢复、余额、转账、支付恢复和展示需要形成同一条可验证的链路。

## 地址派生与稳定身份

拟采用当前账户根下的两个非硬化分支，保留所有已有派生约定：

```text
m/44'/236'/0'
  0/i  收款分支，初始化 i = 0 至 9
  1/i  找零分支，需要找零时分配
```

`m/44'/236'/0'/0/0` 继续作为既有 BSV 主账户地址。WalletId、KeystoreId、账户类型、Portal 登录身份及现有账户关联不能因收款轮换改变。地址池是同一个钱包的子地址集合，不是 10 个独立钱包，也不向 `Wallet.accounts` 追加 10 个同链账户。

账户 xpub 从 `m/44'/236'/0'` 生成，用于公开派生两个分支。它只保存在本地公共元数据中，不上传 WhatsOnChain，不写日志，也不在 UI 展示。助记词仍保存在现有加密 Keystore 中；不新增 xprv 或子私钥持久化。[BIP32](https://github.com/bitcoin/bips/blob/master/bip-0032.mediawiki) 说明了公开子项派生能力及扩展公钥的关联隐私边界；分支约定来自 [BIP44](https://github.com/bitcoin/bips/blob/master/bip-0044.mediawiki#change)。

## Core 所有权与存储设计

地址池属于 Core 的钱包域。建议新增钱包域内的地址池模块和可独立注入的窄查询能力，由其拥有派生策略、公开元数据、地址使用状态、恢复进度和找零分配。它服务于余额、历史、收款、确认及资产详情这些实际消费者，不引入面向所有链的通用 HD 框架。依赖不反向指向 `BsvHomeWalletService`，避免 wallet/session/balance 形成循环。

iOS 只实现 GRDB 存储端口、观察数据库变化、认证和渲染。Receive 页面继续调用 `BsvHomeReceiveService`，Assets 页面继续调用 `BsvHomeAssetDetailsService`；这两个现有服务在 Core 内组合地址池能力，页面不新增地址派生服务或直接读种子。

以下是拟新增的契约字段，具体类型及 API 名称在编码第一阶段冻结，不代表当前已有接口：

| 数据 | 最小内容与约束 |
| --- | --- |
| 钱包地址池 | walletId、BSV 主网身份、固定账户根、账户 xpub、元数据版本、初始化状态、当前收款定位、分支分配进度、扫描进度及修订号 |
| 地址记录 | walletId、分支枚举、非硬化索引、真实地址、使用状态及首次有效使用证据；路径由固定根和定位生成 |
| 唯一性 | 同一钱包的分支与索引唯一；地址与定位必须相符，发现重复或矛盾归属时拒绝继续 |
| 找零及输入预留 | 与确认操作关联的找零定位、所选 outpoint、支出状态；须能在重启后恢复占用 |
| 页面结果 | 当前收款地址、按明确顺序排列的地址记录、初始化与同步状态、可重试的错误信息；不返回秘密或 xpub 给页面 |

GRDB schema migration 只建公共元数据表，不解密钱包。新建及导入钱包时，主账户与初始地址池公共元数据应通过钱包域的初始化存储端口在同一数据库事务提交；后续池更新只写实际变化的记录。秘密文件与数据库不能组成同一个事务，因此保留既有创建补偿逻辑，并验证所有中断点：不能删除已有秘密，也不能向 UI 发布“已就绪”的半个钱包。删除钱包时级联删除地址池、扫描游标与预留记录。

## 初始化与旧钱包升级

新建钱包在现有创建与保存链路中生成账户 xpub、地址 0 至 9，并保存一个主账户和一个地址池。找零分支首次没有预分配的展示记录。初始化地址不移动任何资金。

助记词导入先确定与旧实现一致的钱包身份，再建立公开派生能力并开始历史发现。导入重复钱包时仍识别同一个 WalletId，同时补齐缺少的地址池，而不是仅判断“BSV 主账户已存在”就结束。离线或扫描失败时显示恢复未完成，保留进度及旧数据；不能把暂时查询不到的数据当作零余额或完整恢复成功。

已有助记词钱包在首次进入 BSV 详情、收款或发送等真实使用入口时，经现有认证与密码端口升级。多个入口共享同一幂等初始化操作。启动阶段不主动触发额外钱包解密；数据库初始化也不执行迁移认证。

升级前在 Rust 内验证派生地址 0 与原主账户一致，再补齐 xpub、地址池和状态。认证取消、密码读取失败、路径不符或存储失败均不标记升级完成，不覆盖主账户，不清空原余额、历史或 pending。取消后保留原钱包数据，可再次进入重试；新的多地址操作在准备完成之前不可执行。升级失败不能伪装成完整地址池成功。

## 收款轮换与恢复发现

当前收款地址由 Core 从已分配的未使用收款记录中按索引选择。有效的未确认收款或已确认历史足以标记地址已使用；一旦标记，不因余额清零、交易移出 mempool 或重组而自动回退为“未使用”。已有地址继续能够收款，且持续纳入余额和历史查询。

地址使用判定必须检查合法的链上归属历史，不能以“可用原生余额大于零”替代：已花完的地址、被原生金额策略排除的 1 sat 输出、代币相关输出及未成熟 coinbase 也可能证明地址已使用。它们是否可花费仍由原有原生输出规则判断。

标记当前地址已使用后，原子推进到下一个未使用地址，并在恢复边界允许的范围内补足 10 个未使用收款记录。查看地址列表、重复打开收款页、刷新及二维码重绘不消耗地址。历史已使用地址永不从地址池删除。

恢复扫描不能止于初始 10 个。建议外部分支采用连续 20 个无历史地址的 gap limit；依据是 [BIP44 地址发现](https://github.com/bitcoin/bips/blob/master/bip-0044.mediawiki#address-gap-limit)。本次固定账户索引 0，不扫描其他账户根。找零也需要恢复：优先从已发现交易的完整输出和公开派生脚本建立归属，并扫描内部分支；内部分支若使用连续未用窗口，则属于本项目策略，不能宣称是 BIP44 原文要求。

收款分支同样需要分配边界。多个地址仅收到未确认交易后，这些交易可能全部移出 mempool；本地仍标记已用，但种子恢复时它们可能没有任何历史。不能因此无限推进索引。拟以可验证的已确认历史确定安全分配范围：使用 gap 20 时，从最近有已确认历史的索引之后，最多正式分配 20 个后续索引；尚无已确认历史时，上界为索引 19。有效未确认收款可以推进当前地址，但不能单独扩大这个上界。确认历史消失或扫描不完整时暂停扩大范围并重新发现，不能沿用无法核验的边界。

达到边界时按用户确认暂停新增，显示等待确认或恢复的状态，允许备用地址少于 10 个。池内仍有未用地址时可继续使用；没有可安全分配的当前地址时暂不提供新的收款 QR，不回退已用地址。历史得到可验证的确认后重新计算边界并补足备用地址。上述具体边界是拟定技术策略，须在接口阶段证明其恢复性质；本产品自己造成的空隙不能作为“不支持任意空隙”的例外排除。

暂停未来分配不能撤回已对外提供的地址。例如地址 0 曾确认后允许分配至地址 20，之后地址 0 的历史被重组移除，而地址 20 又收到独立付款，单纯从 0 开始遇到 20 个空地址就停止会漏掉地址 20。本地钱包必须持续覆盖已保存的全部分配范围；新设备只有助记词时，没有这个索引证据。用户已选择“标准自动发现加可继续扩大扫描”，不增加索引备份；因此 gap 20 只是自动发现停止规则，不能声称它在任意重组后都保证自动找到所有已分配地址。

继续扫描从已记录的分支进度向更高索引探测，查到有效历史后纳入池并刷新归属、余额和历史。拟在地址 TAB 的地址记录区域提供扫描进度及继续扫描动作，覆盖收款和找零两个分支；单次批量大小与可取消进度在接口阶段冻结。扫描只扩大查询范围，不把每个空候选地址变成正式收款地址，也不绕过安全分配边界。页面说明已扫描范围，不以“自动发现结束”保证不存在更远地址中的资金。

找零的取消、失败和签名预留也不能无限消耗无链上历史的索引。拟采用按操作固定的找零定位、未签名且确认未提交时释放预留，并给尚未形成历史的找零分配设置相同目的的有界窗口；达到窗口先恢复未完成操作，不能继续跳号。每个已完成发送记录都保存其找零定位。内部分支发现与窗口边界在第一阶段用“连续取消后再成功找零”的恢复测试冻结；只有证明从助记词能重建本产品产生的地址后才开放该分支花费。

已知本地地址须全部覆盖，不受 gap limit 截断。扫描失败、429、分页遗漏、重复游标及网络中断都不计作“无历史”；进度可保存，但不标记恢复完成。扫描完成后保存已使用及正式分配的地址，在安全边界内补足 10 个备用收款地址，保留可继续发现的游标。Wallet 创建可离线生成地址，链上余额及已扫描范围的成功状态需要网络验证。

## 多地址余额与交易历史

BSV 查询范围包含主地址、全部已分配收款地址、已分配找零地址及未完成操作引用的地址。现有 `native_unspent` 的金额、脚本、确认、mempool 花费、铭文或共签 token 排除及 coinbase 成熟度规则全部保留。未确认原生金额继续计入 pending，当前政策下不作为可花费输入。

余额以钱包为单位查询和汇总：按 outpoint 去重，分别累计可用、未确认及本地预留金额，向现有 BSV 资产余额记录提交一次更新。任何必要地址查询失败、响应缺失或分页不完整时保留旧的完整快照并返回失败，不写成功子集；其他链已有的独立刷新行为保持。与未完成支出对应的输入不能因为节点暂时仍返回未花费而再次进入可选余额。

历史先收集全部地址的 txid 并去重，再读取交易并以完整钱包归属集合投影一次。收款、外部付款、找零及钱包内转账均不得重复记账；coinbase 或无可识别地址输入的交易只累计归属于本钱包的输出。本地内部转账使用已确认的收款输出金额；只有链上数据、无法识别付款与找零角色时，展示钱包净变化及交易详情，不以全部钱包输出之和猜测转账金额。该显示规则是拟定设计，需随本文一起审阅。

历史刷新失败保留既有记录，不清空本地发送或 pending，也不推进成功游标。链上历史与本地发送记录按同一 txid 合并。同步期间钱包切换、池修订变化和并发刷新必须隔离，过期结果不能写入另一钱包或被误标为完整快照。

## 转账签名与找零完整性

建议在现有交易 metadata 中增加 BSV 专属支出描述，保持其他链的 UTXO 契约与单地址默认行为。实际跨 FFI 的字段通过现有模型映射生成，不能在 Swift 写平行模型或手改生成文件。

每个选中输入须包含 outpoint、金额、经验证的锁定脚本和受限的分支索引定位；找零须包含固定定位、地址及脚本；计划同时绑定钱包身份、收款输出、费用和确认上下文。定位不能接受任意路径字符串，不允许其他 coin type、账户根或硬化子节点。规划器可能重新排序输入，因此密钥按 outpoint 关联，不按查询数组位置关联。

`BsvHomeKeystore` 在认证后于 Rust 内解密一次助记词，校验钱包身份根，再为各输入派生对应私钥并签名。`bsv_home_keystore` 保持秘密存储职责；派生由 `bsv_home_derivation` 拥有，BSV 规则由 `bsv_home_bsv` 拥有，共享 ForkID 构件只扩展当前必要的逐输入密钥能力。BTC/BCH 既有单私钥入口和签名向量继续验证。

多地址不能通过放宽 `sender` 校验完成。签名前必须证明输入脚本与派生公钥相符、输入属于当前钱包、outpoint 不重复、金额合法，以及“输入总额等于收款加找零加费用”。实际交易只允许一个确认的收款输出和至多一个明确的找零输出；其他未知输出拒绝。最大金额、无找零、最小找零及跨分支输入使用同一规划与费用规则。

找零在最终确认计划中固定，预留与操作绑定。签名时不重新分配地址、静默换输入或改变费用；变更须使旧确认失效。无找零的发送不占用一个新的找零地址。公开地址用于显示和 RPC 查询；私钥及种子不跨 UniFFI，也不保存到数据库或日志。

## 支付恢复与兑换兼容

现有普通发送先广播后保存交易，保存错误还会在 `store_pending` 中被 `unwrap_or_default` 吞掉；它与 Portal 的签名后保存完整 checkpoint 是不同恢复能力。该已知窗口见 [首期 BSV 恢复记录](BSV_INTEGRATION.md#113-请求广播与恢复边界) 和 [当前发送 owner](../core/bsv_home_core/src/services/confirm/mod.rs)。可靠的多地址输入占用不能继续依赖这个窗口。

拟让所有 BSV 资金发送入口复用现有确认与 checkpoint 能力，在广播前持久保存支出描述、原始签名字节、txid、输入和找零预留。普通 BSV 发送需要接入该能力；不重构其他链的发送恢复体系。新 checkpoint 带版本，缺版本的旧 Portal 数据按 legacy 单地址协议解码和校验。解码或校验失败须保留记录并报错，不能视为“没有记录”后重新签名。

现有 checkpoint 的固定金额校验明确拒绝 `use_max_amount`，不能直接接到普通 BSV“发送全部”流程。新 BSV 版本需要保存原始 max 意图、确认时已解析的实际收款额及最终支出计划，恢复严格使用被冻结的金额和签名字节，不按实时余额重算。旧 Portal checkpoint 继续使用固定金额约束，不以支持普通 max 发送为由放宽旧协议。

新多地址 checkpoint 必须验证真实输入输出、费用、派生归属及输入签名，不能只相信保存的地址列表。现有 legacy BSV 校验主要检查脚本结构、拥有者、输出及 txid，不声称已经逐输入执行完整 ECDSA 验证；新方案须用独立签名向量补齐相应契约。

广播超时、App 退出或响应不明时保留预留，先查原 txid 与链上输入状态。需要重播时只使用原签名字节和原 txid，不重新创建替代支付。仅在确认未广播取消、获得足以排除原支付继续提交的确定证据，或已确认相应花费后按状态规则处理占用。现有 tracker 会在查询错误超时后给出本地 `Failed`；这个状态、查询 404、超时或单次看到输入未花费，都不能单独作为释放预留的证据。旧普通 pending 若缺少恢复输入所需信息，先查询其原 txid；无法建立安全占用时暂停新的 BSV 花费并显示待恢复状态，不猜测它已消失。

Portal 登录身份继续使用稳定主地址；现有和本阶段新订单的 payout/refund 地址沿用该稳定契约，收款轮换不改订单地址。支付前的 BSV 余额检查改为统一钱包余额，支付签名走同一多地址路径；恢复支付以保存的支出描述及钱包归属校验交易。账户身份、订单 scope、收款金额、费用、收款人、旧 txid 和签名字节必须保持绑定。[相关 Portal 契约](PORTAL_WALLET_AUTH.md)、[兑换契约](PORTAL_SWAP.md)。

## iOS 地址记录与收款页面

在现有 [AssetScene.addressContent](../ios/Features/Assets/Sources/Scenes/AssetScene.swift) 将单条地址记录替换为 Core 返回的地址池列表，不新增独立详情页或改变 TAB 结构。拟显示真实地址、收款或找零类型、当前收款标记及使用状态；稳定主地址可同时属于已使用收款记录，不能把“主地址”当作第三个派生分支。

记录按 Core 的稳定顺序排列，以分支索引作为记录标识，支持加载所有池内记录及逐项复制。地址数增长时使用惰性渲染或读取分页，分页只是展示方式，不限制可访问的记录总数。每行复制必须使用该行地址，不能复用目前绑定主地址的 `copyModel`。继续扫描是已确认的独立动作，显示分支扫描进度、取消及重试状态；手动添加地址未在已确认范围中，原入口维持明确的不可用状态，不能把扫描等同于新增收款地址。查看列表不触发额外分配。

详情页顶部收款卡、概览 QR、独立 Receive 页、复制和分享统一使用当前收款地址。旧二维码生成任务在地址或钱包变化后失效，不能让新文本配旧 QR。主账户字段继续用于身份、发送方及 Portal，不把页面当前收款地址写回 `Account.address`。发送页需保证“钱包总可用余额”与费用预估一致，同时避免把展示 sender 错当全部输入拥有者。

初始化、认证、恢复和同步都有明确加载、取消、失败与重试状态；准备失败不显示空白成功列表或假地址，查询失败保留缓存并标识状态。复用现有列表、QR、LoadingView、卡片及 [UI 组件](UI_COMPONENTS.md) 和 [样式 token](UI_STYLE_TOKENS.md)。新增可见类型与状态文案通过 Fluent 源及 iOS 本地化生成，不硬编码字符串。

## 执行顺序与阶段产物

确认本文后，按以下顺序实施；每阶段通过相应检查再推进，不先完成一个无法花费的展示地址列表。

| 阶段 | 调整所有者与工作 | 产物及进入下一阶段的条件 |
| --- | --- | --- |
| 1 接口与恢复边界 | 冻结钱包域池模型、受限定位、初始化存储事务、扫描与找零空隙边界、BSV 支出 metadata、checkpoint 版本及依赖图 | 新类型均有具体消费者，无服务循环；本地旧数据兼容和种子恢复契约明确 |
| 2 派生与公共存储 | bsv_home_derivation、BsvHomeKeystore、钱包域地址池及 iOS GRDB adapter | 同一助记词稳定生成 10 个收款地址，公开派生与私钥派生一致，主身份不变；绑定生成仅针对 iOS |
| 3 初始化升级与发现 | BsvHomeWalletService、BSV 首次使用认证入口、扫描及状态规则 | 新建、重复导入、旧钱包升级、离线恢复和中断重试正确；内部找零不会形成不可恢复的地址空隙 |
| 4 钱包查询 | BsvHomeBalanceService、BsvHomeTransactionsService、BSV provider 与 mapper | 多地址余额只写一次完整快照，交易去重与方向金额正确；收款后轮换并补足备用地址 |
| 5 花费与恢复 | BSV planner/signer、共享 ForkID 构件、BsvHomeConfirmService、checkpoint、Portal payment | 跨收款和找零地址输入可验证，确认与签名一致，预留和原 txid 恢复贯通全部 BSV 发送入口 |
| 6 iOS 集成 | 现有 AssetDetails/Receive Core API、ServicesFactory、Assets/Transfer/WalletTab 页面、本地化 | 地址 TAB 可看全部记录，逐项复制正确，收款 QR 同步，普通发送与兑换使用钱包总余额 |
| 7 验收与交付 | 两轮清理、Core/iOS 自动检查、模拟器与专用钱包验收、更新接入及 Keystore 文档 | 准确记录通过、失败、跳过和未测试分支；不把确定性签名测试当作主网收发已验收 |

当前工作区有大量既有修改。编码时只调整本方案所有者，保留其他工作，不清空模拟器或原钱包。阶段 1 会再次记录基线，避免把原有问题与本次回归混为一谈。新增接口需同步实际 iOS 工厂、协议与测试 mock，但不执行其他平台工作。

## 验证计划

| 领域 | 必须覆盖的独立契约 |
| --- | --- |
| 派生与身份 | 10 地址唯一且向量一致，地址 0 不变，xpub 与受保护派生一致，分支索引边界及不支持钱包类型 |
| 初始化及迁移 | 新建、单链或多链助记词导入、重复导入、已有 BSV/缺 BSV 钱包、取消认证、密码失败、事务失败、断点恢复、删除级联 |
| 发现及轮换 | 地址 9 以外已有资金、已花完地址、有效未确认收款、重复事件、重组不回退、10 个备用地址、扫描错误不能算空地址、连续未确认收款消失后的边界暂停与种子恢复、已分配高索引前的确认历史被移除后继续扫描发现资金、连续取消找零后种子恢复 |
| 余额及历史 | 收款与找零多地址合计、重复 outpoint、任一地址失败保留旧非零快照、分页和 429、同 txid 多地址出现、内部转账、找零不重复入账 |
| 签名及确认 | 至少两个不同子密钥输入、跨分支输入、排序后密钥仍匹配、错误 locator/公钥/脚本/前序金额、外国钱包输入、未知输出、实际签名验证、费用守恒和最大金额 |
| 恢复及并发 | 普通发送和 Portal 共用输入互斥、保存失败零广播、广播超时、签名后退出、旧 checkpoint、max 金额冻结后恢复、同字节重播、tracker 超时 Failed 不释放输入、未知旧 pending、切换钱包及双重确认 |
| iOS | 初始 10 条、扩展后超过 10 条、历史找零和零余额记录均可查看，逐行复制及 QR 独立解码，继续扫描进度/取消/重试，资产列表与余额不重复，认证和 Portal 身份稳定 |

编码前制定的 Core 检查从 `core/` 执行，最终实际执行命令与结果见末尾：

```bash
just test bsv_home_derivation
just test bsv_home_bsv
just test bsv_home_bitcoin
just test primitives
just test bsv_home_core
cargo clippy --locked -p bsv_home_derivation -p bsv_home_bsv -p bsv_home_bitcoin -p primitives -p bsv_home_core --all-features --all-targets -- -D warnings
just format
```

`just test <crate>` 使用全 features 并跳过 integration_tests。默认忽略的 live tests 和实际跳过数量需在结果中单列；零选中测试不算通过。格式化若触及无关已有文件，只保留本次必要变更，不盲目覆盖工作区。

iOS 接口、模型及可见文案稳定后从仓库根执行所需生成，再进行定向验证：

```bash
just ios generate-models
just ios generate-core
just ios localize
just ios test StoreTests
just ios test BsvHomeCoreServicesTests
just ios test AssetsTests
just ios test TransferTests
just ios test WalletTabTests
just ios test OnboardingTest
just ios build
git diff --check
```

仅在对应模型或本地化输入改变时运行相应生成。若 AppService 的首次使用编排实际调整，再执行 `just ios test AppServiceTests`；不把“首次使用升级”改成 startup 自动升级。Core 与 iOS 生成、库构建相互有依赖，应按顺序执行；并行只用于互不依赖的只读检查。遵循 [质量检查](../skills/quality-checks.md) 与 [iOS 专用命令](../skills/development-commands.md)，不运行涉及其他平台的根生成或构建配方。

模拟器验证需覆盖新建及旧钱包首次升级、列表超过 10 条、每行复制、未确认收款后当前地址与 QR 同步、多地址余额发送及 Portal 恢复。确定性 fixture 覆盖资金与故障场景；真实费用、广播及完整种子恢复另用专用小额 BSV 钱包验收，不使用现有用户资金。若尚未取得该验收条件，明确保留为未完成验收项。

## 已知限制与编码前确认

WhatsOnChain 当前单地址 `/unspent/all` 已在本地实现有界分页及完整性校验，可作为首轮多地址实现的基础，复用共享节流、有限重试和公共节点选择。批量优化不是必需前提。官方 [Address 文档](https://docs.whatsonchain.com/address) 与 [UTXO 文档](https://docs.whatsonchain.com/un-spent-transaction-outputs) 对批量地址数量及返回项设有限制；UTXO 章节的请求方法与示例还存在不一致。未验证完整返回、方法及分页语义前，不引入新的批量 UTXO 路径，不把截断结果作为全钱包数据。

当前没有阻止文档设计继续进行的外部依赖，也不需要为本方案更换数据服务。供应商批量接口问题可用现有逐地址查询避开；较大地址池的刷新耗时和限流表现需实际验证。恢复范围只覆盖本产品固定根；标准自动窗口配合继续扩大扫描支持找回较远地址，不能承诺任意空隙或重组后都在首次自动扫描中发现全部资金，不承诺其他钱包路径的资产恢复。

助记词恢复的保证对象是可验证的链上资金与地址归属。本地“曾收到但后来消失的未确认交易”标签、从未广播的操作和本地查看记录不能仅从种子及当前链上历史完整重建。已有本地钱包保留这些状态；全新设备恢复可能无法还原相同的 UI 使用标签。本文不引入额外的元数据云备份，不能声称种子包含这些信息。

已整体确认的技术方案包括稳定主账户与独立地址池、按上述规则发现与分配找零、普通 BSV 发送接入持久 checkpoint 和预留、Portal 保持稳定订单地址，以及地址记录的展示字段。主网小额验收使用的专用钱包与资金条件在执行该步骤前落实，不凭零余额模拟器推断收发成功。

用户已授权按本文落地。共享接口的其他平台影响保持未验证，不执行 Android 工作。

## 实际接口与实施记录

`bsv_home_derivation` 提供固定账户根的 `bsv_account_xpub`、`bsv_address` 与受保护的 `bsv_private_keys`。定位由 `BsvAddressBranch` 和非硬化 `index` 构成。BSV 账户仍只有一条；旧 `Account.extended_public_key` 的原始公钥语义保留，地址池 xpub 独立保存。受控种子、私钥字节与持有的扩展私钥字段在 Rust 内清理；依赖库内部 Copy 值和密码算法临时副本的完整擦除无法由消费方保证。

Core 钱包域 [BsvHomeBsvAddressPool](../core/bsv_home_core/src/services/wallet/addresses/mod.rs) 暴露 `prepare`、`snapshot`、`sync`、`continue_scan` 及显式取消接口，使用既有 Gateway、钱包 Store、Keystore 与认证端口。初始 10 条收款记录可离线生成。自动发现对收款和找零分别扫描 gap 20；单次最多探测 1000 个候选，未完成状态保存游标并支持继续扫描。继续扫描从保存的两个分支游标分别向后扩展。已知正式记录始终复核，空探测不加入展示列表。恢复未完成时，钱包余额和历史保留旧快照，花费加载要求自动发现完成；扫描期间不扩大备用地址池。

首页余额刷新与后台收款补同步使用地址池内部的展示刷新入口。同一钱包、扫描 generation、收款事件 generation、地址池 revision 与节点范围的在途余额查询共享结果，完成后不缓存；首页另合并并发的地址复核，后台收款仍独立查询历史。返回前再次检查钱包身份、地址池 revision、generation 与当前节点；有效的新收款或 Resync 事件使旧查询范围失效，地址池返回前跟进当前余额，连续失效时结束本轮并返回取消错误。签名、资金预留和指定收款交易的索引核验继续使用各自原有的新鲜读取路径。不同钱包的扫描分别排队，已知地址复核没有状态变化时不重复写入地址池或递增 revision。已知地址复核及整钱包历史使用供应商现有的批量历史接口，每批最多 20 个地址，仍校验响应地址集合、逐地址错误、独立分页与交易归属。补同步期间到达的新 Resync 保留后续补同步，不把它们当作已完成查询吞掉。

当前 UniFFI async Swift 桥本身不把 `Task.cancel()` 传回 Rust。地址扫描使用显式 `scan_generation`、`sync_at_generation` 与 `cancel_scan`；AssetDetails/Receive 端口同步读取 generation，在异步调用中携带它，取消按钱包递增 generation 并中止运行和排队的扫描。过期调用在首次 Rust poll 前被拒绝；RPC 返回、状态提交和完成边界再次检查 generation。中断保留已经成功保存的进度和初始化公开元数据，不把未完成 RPC 当作空地址。Rust 的 `Abortable` 负责释放正在等待 RPC/锁的扫描，UI 同时停止进度读取；后续新扫描使用新 generation 继续。

`BsvHomeBsvAddressStore` 保存带 revision 的公开 JSON 状态并以 CAS 提交，返回未知旧 pending 用于花费保护。iOS [BsvAddressPoolStore](../ios/Packages/Store/Sources/Stores/BsvAddressPoolStore.swift) 使用 GRDB，钱包主记录、一个 BSV 主账户及初始池在同一数据库事务保存，删除级联清理。Keystore 文件保存独立；公共初始化或事务失败只补偿本次新建的秘密，补偿失败会显式返回。地址名称写入失败不删除已经提交的钱包秘密。

`BsvSpendMetadata` 保存逐输入的 outpoint、金额、地址、锁定脚本、定位及可选找零。BSV planner 只冻结最终选中的输入；ForkID 签名按排序后的 outpoint 取得对应密钥。池根校验、脚本与 ECDSA 验证共同约束原始交易。找零窗口耗尽时允许有效的无找零发送，实际需要找零的发送仍暂停。钱包余额分别累计可用、未确认和节点仍返回的预留输出，避免已花费输入与新找零双计；查询期间池 revision 改变会拒绝提交旧结果。

新版 BSV checkpoint 为 version 1，保存原始 max 意图、实际金额、费率、全部支出描述、签名字节及真实 txid；缺版本的旧数据按 version 0 验证。普通 BSV 与 Portal 共用池中的输入及找零预留，签名字节持久保存、待发送记录写入成功后才广播。超时、404、tracker Failed 不释放签名预留；同一未完成意图恢复冻结计划并仅重播原字节。已准备但未签名的操作可在进程重启后继续，同一进程的并发签名由 lease 排斥。加载或保存 checkpoint 时同时验证钱包根、真实签名、哈希和预留内容。

iOS 详情与 Receive 使用既有 Core 服务提供的地址池 API，地址记录展示全部正式收款和找零地址及其状态，每行复制自身地址。当前收款卡、QR、复制和分享来自同一当前地址。继续扫描有分支进度、取消和失败重试。发送入口完成池准备、发现和钱包余额刷新后才估算费用。Portal 登录、payout/refund 及稳定发送方身份保留主地址。

## 本轮验证记录

以下命令在当前未提交工作区执行；Core 命令的工作目录为 `core/`，iOS 命令的工作目录为仓库根。生成产物均由源模型、UniFFI 或 Fluent 生成，没有手工修改生成文件。

| 检查 | 实际结果 |
| --- | --- |
| `just test bsv_home_derivation` | 21 passed，0 failed / ignored / filtered |
| `just test bsv_home_bsv` | 42 passed，0 failed / ignored / filtered |
| `cargo test --locked -p bsv_home_bitcoin --lib --all-features` | 67 passed，0 failed / ignored / filtered；保留并验证既有 BTC/BCH 签名行为 |
| `just test primitives` | 263 passed，0 failed / ignored / filtered |
| `just test bsv_home_core` | 显式取消修改后最终 837 passed，0 failed，2 ignored，0 filtered；包含 19 项地址池规则/集成、3 项 HD Keystore 与 HD checkpoint 测试 |
| `cargo test --locked -p generate --bin generate` | 12 passed，0 failed；iOS TypeShare 失败会显式终止生成 |
| `cargo clippy --locked -p bsv_home_derivation -p bsv_home_bsv -p bsv_home_bitcoin -p primitives -p bsv_home_core -p generate --all-features --all-targets -- -D warnings` | passed |
| `cargo clippy --locked -p bsv_home_core --all-features --all-targets -- -D warnings` | 显式取消修改后定向重跑 passed |
| `cargo test --locked -p bsv_home_core --lib --all-features services::wallet::addresses::tests::test_cancel` | 2 passed，0 failed / ignored，837 filtered；覆盖阻塞 RPC、排队扫描、已提交游标、重试与 first-poll 之前的取消 |
| `just format` | passed；最终格式化后再次执行受影响检查 |
| `just ios generate-models` | passed；包含 Bsv / BsvSpend 与 remote mapper |
| `just ios generate-core` | passed；最终 Swift/header 与 iOS 模拟器、真机 debug 静态库均已生成 |
| `just ios localize` | passed；13 项 BSV 文案覆盖 31 个现有语言 |
| `just ios test StoreTests` | 98 测试、103 参数实例，0 failed / skipped；包含 5 项 BSV 地址池存储测试 |
| `just ios test BsvHomeCoreServicesTests` | 显式取消修改后最终 48 测试、48 参数实例，0 failed / skipped；包含 3 项 native 池初始化/升级与 2 项真实 FFI 取消集成 |
| `just ios test AssetsTests` | 显式取消及生命周期修正后最终 41 测试、53 参数实例，0 failed / skipped；包含全部地址记录、轮换、独立 QR 解码、认证/扫描取消与重试及取消后不立即重启余额扫描 |
| `just ios test TransferTests` | 最终 167 测试、172 参数实例，0 failed / skipped；包含 Receive 首次认证防重入、网络 lookup / 准备 / 页面离开取消与重试、暂停、单地址钱包及 BSV Send 认证失败 gate；QR 等待后的修订号检查另经源码终审，没有独立悬挂 QR 取消 fixture |
| `just ios test WalletTabTests` | 70 测试、76 参数实例，0 failed / skipped |
| `just ios test OnboardingTest` | 17 测试、19 参数实例，0 failed / skipped |
| `just ios build` | 最终生成库与显式取消接口集成后 passed，`BUILD SUCCEEDED` |
| `git diff --check` | passed |

最终六个 iOS unit 目标合计 441 个测试、471 个参数实例通过，0 failed / skipped，包含原生取消严格计时版本。新建/导入原生池保持 10 个地址、一个 BSV 主账户和重复导入稳定；旧钱包首次显式 prepare 仅读一次密码，重开已有池不重读；观察钱包不读取密码、不建立 HD 池。AppService/startup 编排未修改，因此没有额外运行 `AppServiceTests`。SwiftFormat 定向检查通过；`swiftlint` 在当前环境不可用（命令退出 127），未宣称其检查通过。

两项忽略测试为 `services::portal::live_tests::test_live_portal_pairs_quotes_and_orders_without_creating_or_paying_orders` 与 `test_live_portal_wallet_authentication_and_cached_session`，需要批准的 Portal 测试部署；此次没有运行这两项，不计为 live 通过。

收款不回退规则另做一次实际反向验证：暂时反转 `PoolState.observe` 的有历史判定后，`test_unconfirmed_receive_usage_rotates_and_remains_used_after_history_rollback` 按预期失败；恢复源码后完整 BsvHomeCore 套件通过。反向验证的修改没有进入生成库或最终工作区。

实施与测试前后已分别清理和检查所有本次所有者；独立终审覆盖恢复边界、CAS/修订号、余额和历史的完整快照、Keystore 密钥归属、普通/Portal checkpoint 与重播。地址池与 HD 发送路径的存储、RPC、认证和验证错误均向调用者返回，不以空数据、部分成功或重建秘密掩盖失败。既有非 HD 普通发送错误策略属于原路径，本轮未扩展修改。

另一中间 Assets 运行因参数 fixture 在后台 refresh 尚未进入前取消、与预期不同而失败（exit 65）。fixture 改为等待本轮 refresh 实际进入 loading 后再取消，保留后续新调用可正常重试的生产契约；完整最终 Assets 套件通过。

一次中间 Assets 运行因系统 `UIPasteboard` 读取在 `libRPAC` 等待而中止（exit 75），不计为通过。新 unit fixture 已移除真实系统剪贴板读写及 setter 副作用，保留各行复制目标、当前地址、QR 独立解码与轮换后的对应关系，之后完整 Assets 套件通过。模拟器实际逐行复制显示了与所选地址一致的反馈；未读回系统剪贴板的值，实际剪贴板内容保持未验收。

真实 FFI 取消 fixture 的首次运行因 history 空数组违反完整响应契约、等待时间未覆盖 400ms 节流而失败（exit 65）；该轮不计通过。随后 fixture 按请求地址返回完整 confirmed/unconfirmed 历史结构，等待第 12 RPC 的上限改为 30 秒，取消后的悬挂 HTTP `stopLoading` 必须在 2 秒内出现（短于原 5 秒请求超时），避免将自然超时当作取消。终审将 2 秒 `stopLoading` 计时置于取消后的第一步，先检查 HTTP 中止再等待 scan.value；该严格计时版本所在完整原生套件最终 48 项通过，包含 Core typed Cancelled、已提交 receiveScanned=1/changeScanned=0 保留及重试完成双分支 20/20。取消先发生、首次 Rust poll 后发生的旧 generation 调用也被拒绝且没有读取密码或写池。

### 模拟器实际验收

使用保留原数据的 iPhone 17 / iOS 26.4 模拟器，未清空、退出或删除原钱包。安装最终构建后启动，既有其他币种资产仍显示。执行命令如下，均成功；安装完成后再次终止并启动 App，确保运行最终安装产物：

```sh
xcrun simctl boot 012AAE51-0C8D-4BEE-A704-CD9CF3C65402
xcrun simctl bootstatus 012AAE51-0C8D-4BEE-A704-CD9CF3C65402 -b
xcrun simctl install 012AAE51-0C8D-4BEE-A704-CD9CF3C65402 ios/build/DerivedData/Build/Products/Debug-iphonesimulator/Gem.app
xcrun simctl terminate 012AAE51-0C8D-4BEE-A704-CD9CF3C65402 com.gemwallet.ios
xcrun simctl launch 012AAE51-0C8D-4BEE-A704-CD9CF3C65402 com.gemwallet.ios
```

通过 CUA 实际操作确认：

- 进入既有助记词钱包的 BSV 详情，地址页有 10 条收款记录，索引 0 至 9；当前地址同时标注钱包身份，各行有独立复制入口。复制索引 9 的反馈与该行地址一致。
- 标准发现进行到收款 20 / 找零 4 时取消，显示已取消并保留进度及地址列表；重试继续完成 20 / 20。
- 点击继续扫描后进度越过原窗口；在 31 / 20 取消，重试完成 40 / 40。页面明确说明结果仅覆盖已扫描范围，探测的空地址没有变成正式展示记录。
- 详情页与收款页均正常显示 QR；收款页地址与地址 TAB 的当前收款记录一致，复制反馈也一致。二维码内容另由自动测试独立解码验证，模拟器截图未另作解码。
- 关闭收款页后返回地址 TAB，当前地址、正式记录及 40 / 40 完成状态保留。模拟器最终停留在此页，便于检查。

模拟器为零 BSV 钱包，本轮没有发送交易或使用原有其他币种资金。真实未确认收款触发轮换、地址记录超过 10 条及找零展示、多地址有余额发送、Portal checkpoint 恢复和远端种子资金恢复由确定性测试覆盖，未在该零余额模拟器完成资金场景验收。新建/导入与旧钱包单次认证升级的计数、事务和密码读取由原生集成测试验证；本次手工验收只使用既有钱包，没有新增钱包或验证实体设备认证弹窗。Receive 等待 QR 期间退出的独立悬挂 fixture、系统剪贴板内容读回、较大地址池真实性能仍未验证。

用户明确暂不提供专用测试钱包，因此主网小额收发、实际广播费用与到账、真机认证保留为后续验收项。两个 Portal live 测试保持 ignored；本轮未执行 Android 工作，共享变化对 Android 的影响未验证。代码与文档留在当前未提交工作区，没有提交、推送或发布。
