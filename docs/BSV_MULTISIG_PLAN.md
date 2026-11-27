# BSV 多签钱包接入文档

## 当前契约与阶段5执行授权 — 2026-11-15

iOS 产品概念与显示文案统一为“多签钱包”，繁体中文为“多簽錢包”，英文为“Multisig wallets”。内部 `Collective` 类型与既有协议字段保留原名。

用户已明确批准 Android 对齐方案的阶段0–8，并在阶段4 LOCAL gate 通过后授权开始阶段5：多签钱包创建、加入、恢复、配置核对和专用签名身份。事实基线为本机 Android 仓库 `/Users/gavin/workspace/wowpay-android` 的干净 `bsvhome` 分支、固定提交 `cc895b5a72084eae1b028578d5ce1331d71c2588`；已提供 APK 作为运行界面参考，[当前原型](WOWPAY_UI_REFACTOR.md#current-prototype--2026-11-15)仅补充视觉和交互意图。旧文档中“原型提案尚未形成实施授权”的说明不能覆盖这次明确批准；同时，原型演示不证明能力已经实现。本节是当前合同与链路索引，不是阶段5实现或验收通过记录；阶段状态与逐阶段本地验收门禁见[Android 对齐计划](WOWPAY_ANDROID_ALIGNMENT.md#phase-sequence-and-acceptance-gates)。

编码范围仍限 Core 与 iOS；用户已单独授权只读参照上述外部 Android 仓库，不修改、构建或生成 Android，不操作含密码或秘密的现有运行钱包，不部署服务端。下面保留的旧合同及实施记录属于历史证据，其中旧 Origin、当前普通钱包身份、原型未授权和不检查 Android 等当时范围均不替代本节。

### 环境、接口与实测边界

普通 bsvhome 使用 V1 Origin `https://paymail.bsvhome.com`、Portal 前缀 `/api/portal`，多签前缀为 `/api/portal/auth/multisig`；沿用钱包签名登录、原始 `Authorization` token、iOS `deviceType=4` 及 `portal_bsvhome` 受保护会话命名空间。保留的 V2/platform/MPC 环境及其数据不迁移、不删除，也不与当前会话或配置信任混用。认证、固定钱包作用域和取消约束以[Portal 钱包认证](PORTAL_WALLET_AUTH.md#ownership-and-scope)为准；禁止重定向、缓存和业务写请求自动重放。

已从固定 Android 的创建/成员/配置调用链确认的接口为：`GET wallets`，`POST wallets`，`POST wallets/{id}/invites`，`POST invites/preview`，`POST join`，`POST wallets/{id}/confirm`。固定 Android 的配置确认从钱包行重算本地策略后直接 POST confirm，不调用 `GET wallets/{id}/config`；阶段5 Core 已将确认前的独立读取改为钱包 detail GET，继续核对身份、版本和本地脚本/hash；非空 detail DTO 已在本轮受限生产验收中解码通过，仍不据此臆造其他端点的 schema。恢复复用身份导入与钱包列表读取，没有新的 restore、公开配置 import 或 backup 写接口。端点与字段以实际源码的已知契约为界，非空钱包、邀请、成员及配置 DTO 已在新授权的生产生命周期内通过；该结果不扩展到支付/入金等未验端点，空数据、Mock 或历史 V2 记录不能替代本次实测。

用户已单独授权并完成一次阶段5生产验收：2个全新临时未资助身份、1个2/2多签钱包、创建/邀请/加入/双方确认，实际恰好16次HTTP、5次有效业务写入，全部HTTP 200。双方本地配置Confirmed、canTransact成立且公开配置备份一致，无资金、提案、Paymail、在线密码或广播请求；本轮无需修改生产源码。该一次性预算已消耗，临时加密目录随测试释放（best effort）；服务端身份/钱包/成员/审计记录可保留，不宣称删除。阶段1/4既有授权及本次授权均不扩展到阶段6支付操作，阶段5LOCAL门禁通过及命令证据见[执行记录](WOWPAY_ANDROID_ALIGNMENT.md)。

### 专用身份、恢复与备份语义

Android 的专用身份已经存在，并非仅有原型提案。创建复用普通 12 词生成、完整词序验证与 PHRASE 导入，结果为 `WalletType.Multicoin`，对外 account 为 BSV；默认名称沿用普通钱包递增命名。BSV 成员使用根路径 `m/44'/236'/0'/0/0` 的 secp256k1 公钥及对应 P2PKH 地址；专用身份不是新增私钥格式或身份协议。`MultisigIdentityStore` 仅按本地 wallet ID 保存 dedicated 用途元数据，不能把该元数据当作密钥或成员关系。

新建专用身份时，已有当前本地钱包则保留当前选择；没有当前钱包才设为新身份。导入/恢复复用导入流程，会选择导入的钱包为当前钱包；专用恢复仅对 `New` 结果写 dedicated 标记，`Existing` 不改原分类。标记身份从普通钱包选择列表隐藏，若成为当前钱包，首页/资产页走多签钱包入口；这是展示与用途限制，底层仍是持钥钱包，不宣称其在密码学上不能个人支出。iOS 应按 Core 业务所有权实现用途及展示隔离，不由 Swift 重定义成员身份或签名规则。

多签钱包按 Origin 与远端 wallet ID 记忆选中的本地 signer；成员候选须匹配地址和公钥，不随个人展示钱包切换。策略信任与成员备注按 Origin、本地 signer 和远端钱包隔离；身份选择、创建/加入核对与异步返回须绑定相同作用域。Android 的多签钱包本体只有 VM 内存缓存，不是持久化公开观察钱包；删除唯一身份后不保证保留该多签钱包。不能据原型新增持久化无身份钱包或新的服务端恢复契约。无可用本地身份时不得确认配置、签名或冒充已恢复。

“恢复已有钱包身份”和“恢复专用签名身份”均走 phrase 导入及钱包列表重拉。普通恢复继续原 WalletReady/Setup 流程；专用恢复完成后进入多签钱包列表并选中导入的 signer。已有导入页支持的派生路径、口令和 BSV HD 恢复能力继续由原导入所有者处理，不把它们改成多签公开配置 JSON 导入。恢复后即便服务器成员已经 confirmed，本机没有可信 policy 仍须进入 `NeedsLocalReview`，显式重算配置、核对并确认后才保存本机信任。

专用创建沿用真实词序验证；选择已有 signer 的创建/加入链路没有额外的“备份完成”强制门控，不把原型的备份演示扩成新产品规则。公开配置备份只在本机 policy 一致且钱包 ACTIVE 后生成，包含 `network=BSV_MAINNET`、`type=P2MS`、policy、lockingScript、scriptHash、configurationHash；它不含秘密，不能恢复私钥、服务端成员关系或未决入金。原型中已实现的身份选择不能继续称为未授权提案，尚未实现的额外备份门控也不能凭演示宣称存在。

保留 iOS 已有安全提醒、助记词备份/验证、原生认证、受保护存储和 Core 取消/身份代次检查；既有备份不迁移或重写。Android 定时刷新读取 keystore 密码及页面返回未直接取消业务 Job 的行为不作为安全要求复制；退出、后台和身份变化不能让迟到结果继续授权或污染新作用域。策略保存错误必须显式可见，不照抄身份偏好或备注 `apply()` 的无落盘确认语义。

### 阶段5当前源码接口与所有权

[身份展示上下文](../ios/Features/BsvMultisig/Sources/Types/BsvMultisigIdentityContext.swift)是不可变的 `BsvMultisigIdentityContext(options:selectedWalletId:isSelecting:onSelect:onCreateDedicated:)`，其中 `isSelecting` 默认 `false`、专用创建回调默认 `nil`；[IdentityOption](../ios/Features/BsvMultisig/Sources/Types/BsvMultisigIdentityOption.swift)只提供 `walletId/name/address/isDedicated`，`id` 等于 wallet ID，[IdentityEntry](../ios/Features/BsvMultisig/Sources/Types/BsvMultisigIdentityEntry.swift)为 `.create/.join`。Feature 身份卡只展示候选和调用回调，不持有 Onboarding 或钱包服务，不在 Swift 重定义成员资格。App 的 [CollectiveWalletDirectory](../ios/BsvHome/ViewModels/CollectiveWalletDirectory.swift)提供经 Core 投影的候选与持久化选择：创建/加入可选本机合格身份，已有多签钱包仅可选该钱包的成员身份。

[导航 modifier](../ios/Features/BsvMultisig/Sources/ViewModifiers/BsvMultisigNavigationModifier.swift)保留 `bsvMultisigNavigation(model:navigationPath:)` 兼容调用，并追加默认 `nil` 的 `identityContext`、`onScanInvitation` 和 `identityContextForRoute`。路由 provider 类型为 `@MainActor (BsvMultisigRoute) -> BsvMultisigIdentityContext?`；存在 provider 时严格使用该路由的结果，包括 `nil`，不能回退到全部身份。公开的 [BsvMultisigDestination(model:route:)](../ios/Features/BsvMultisig/Sources/Scenes/BsvMultisigDestination.swift)与 typed `.list/.create/.join` 复用原场景，不额外套导航栈。App 可通过公开的 `EnvironmentValues.bsvMultisigIdentityContext` 和 `.bsvMultisigInvitationScanner` 为直接渲染的根目的地注入上下文。

[CollectiveOnboardingModel](../ios/BsvHome/ViewModels/CollectiveOnboardingModel.swift)与[场景接线](../ios/BsvHome/Scenes/CollectiveOnboardingScene.swift)负责身份切换、扫码呈现及流程生命周期。切换 signer 同步取消旧 fixed-wallet Feature/Core 模型后重建，不把远端多签 wallet ID 或另一个 signer 写成普通钱包 current；创建/恢复自身改变 current 的条件仍遵循上一节。退出、后台和旧模态返回由 App 作用域、代次及请求 ID 使旧任务失效。Feature 页面取消自己的展示任务，不能替代 App 对 Core 模型的取消；取消已发出的服务端写请求不代表服务端记录已撤销。

[创建表单](../ios/Features/BsvMultisig/Sources/Scenes/BsvMultisigCreateScene.swift)按完整选中身份展示快照 `walletId/name/address/isDedicated` 变化重置名称为空、门槛为 2、成员备注为默认 3 项；同身份重绘、同身份新 VM 和原生预览返回保留字段。不能用模型对象地址变化代替身份变化来清空表单。[加入表单](../ios/Features/BsvMultisig/Sources/Scenes/BsvMultisigJoinScene.swift)的邀请码不随身份切换重置；旧预览/授权仍须作废。创建核对不新增确认 checkbox，加入及配置核对保留原有显式确认要求。

[创建 Core 所有者](../core/bsv_home_core/src/services/bsv_multisig/creation.rs)提供 `beginCreation(name:threshold:memberNames:signerWalletId:) -> BsvHomeBsvMultisigCreationDraft`、`submitCreation(draftId:) -> BsvHomeBsvMultisigWallet`、`reconcileCreation(draftId:) -> [BsvHomeBsvMultisigWallet]`、`acceptCreation(draftId:walletId:) -> BsvHomeBsvMultisigWallet`。Draft 包含 `id/name/threshold/memberNames/signingIdentity/status`，状态为 `Ready/Submitted/Resolved(walletId)`；[Feature review](../ios/Features/BsvMultisig/Sources/Types/BsvMultisigCreationReview.swift)保留原 draft，提交不重新推断身份或生成新 draft。Ready 的首次提交先可靠保存 Submitted，再至多发出一次创建 POST。`CreationUncertain(draftId)` 或恢复的 Submitted 只提供 GET 查询候选；即使只有一个候选，也须用户核对后显式 accept，并 fresh GET 验证选中钱包，不自动认领、重新 POST 或自动保存 policy 信任。已 Resolved 的返回沿用原 wallet ID；同名称的另一次明确新创建不等于重试旧请求。

[邀请解析及编码](../core/bsv_home_core/src/services/bsv_multisig/rules.rs)只由 Core 的 `parseBsvMultisigInvitation(value:)` 与 `bsvMultisigInvitationQrPayload(token:)` 决定：接受原始 43 位 URL-safe token 或严格的 `bsvhome://multisig/join?token=<token>`。邀请 QR、复制和分享统一用 canonical URI；Native 不再维护第二套解析器。[扫码回调](../ios/Features/BsvMultisig/Sources/Types/BsvMultisigInvitationScanner.swift)为 `@MainActor (@escaping @MainActor (String) -> Void) -> Void`，由 App 复用现有扫描器、相机权限与呈现 ID；BsvMultisig Feature 不依赖 QRScanner Feature。[公钥/地址/hash 值行](../ios/Features/BsvMultisig/Sources/Views/BsvMultisigValueRow.swift)及[成员行](../ios/Features/BsvMultisig/Sources/Views/BsvMultisigMemberRow.swift)接受显式复制回调，保留完整值、文本选择及辅助功能布局；组件不取得认证或密钥。

创建与导入入口由 Feature 内部 [WalletOnboardingNavigation](../ios/Features/Onboarding/Sources/Navigation/WalletOnboardingNavigation.swift)共同管理 typed multisig entry 和后续同一个 NavigationPath；内部推页/返回不撤销作用域，pop 越过入口深度才同步取消。真实 UI 已发现并修复 Boolean 入口被 typed push 替换时误取消的缺陷；App 在选择器关闭、选择未变且当前根页面可见时显式重启已取消的 overview，不复用失效 fixed Core service。正向窗口测试、受影响 Native 检查、完整 App 构建及实际创建核对/返回/原 Assets 恢复 UI 均已通过；独立模拟器上的公共观察钱包全链路回归及收款 QR 解码也通过。命令、失败及修复证据见[阶段5执行记录](WOWPAY_ANDROID_ALIGNMENT.md)。

用户允许的本地真实 QA 身份资源工具仅为 [BsvHomeTests 默认关闭的 harness](../ios/BsvHomeTests/AlignmentRegressionWalletTests.swift)：显式 opt-in、指定 base simulator、真实 App host/container、完整钱包库检查及单个专用身份标记均须匹配。该工具已通过一次，创建了一个使用既有加密 keystore/Keychain 的未资助身份并保留供后续回归；未增加生产启动种子入口、接受条款或确认备份，不构成真实创建/备份 UI 验收。非空生产DTO及多成员创建/邀请/加入/配置确认已在另一项明确授权下实测通过，阶段5LOCAL验收通过；阶段2人工备份、实体设备/资金及阶段8全量验收仍按原门禁待补。本地创建授权本身不扩展到线上写入；本次另批预算也不复用到后续阶段。

### 阶段5代码链路索引

| 链路 | 固定 Android 所有者与关键行 | iOS 对齐所有者 |
| --- | --- | --- |
| 创建/导入/恢复入口 | [Onboarding 路由](</Users/gavin/workspace/wowpay-android/android/app/src/main/kotlin/com/gemwallet/android/features/onboarding/wowpay/WowPayOnboardingNavigation.kt>) 55–85；[WalletNavGraph](</Users/gavin/workspace/wowpay-android/android/app/src/main/kotlin/com/gemwallet/android/ui/navigation/WalletNavGraph.kt>) 289–313 | [Onboarding Feature](../ios/Features/Onboarding/Sources)、App 目的地接线；复用既有创建/导入安全链 |
| 专用身份创建及标记 | [CreateWalletViewModel](</Users/gavin/workspace/wowpay-android/android/features/create_wallet/viewmodels/src/main/kotlin/com/gemwallet/android/features/create_wallet/viewmodels/CreateWalletViewModel.kt>) 60–88；[IdentityStore](</Users/gavin/workspace/wowpay-android/android/data/services/gemstone/src/main/kotlin/com/gemwallet/android/data/services/gemstone/multisig/MultisigIdentityStore.kt>) 11–23；[恢复标记](</Users/gavin/workspace/wowpay-android/android/app/src/main/kotlin/com/gemwallet/android/features/onboarding/wowpay/DedicatedIdentityImportViewModel.kt>) 17–23 | [Wallet Core](../core/bsv_home_core/src/services/wallet)、既有 keystore/钱包存储端口；用途元数据不改变密钥协议 |
| 身份选择及创建/加入/配置 | [MultisigViewModel](</Users/gavin/workspace/wowpay-android/android/features/asset/viewmodels/src/main/kotlin/com/gemwallet/android/features/asset/viewmodels/multisig/MultisigViewModel.kt>) 167–204、274–365、477–574；[配置六态](</Users/gavin/workspace/wowpay-android/android/features/asset/viewmodels/src/main/kotlin/com/gemwallet/android/features/asset/viewmodels/multisig/MultisigConfiguration.kt>) | [Multisig Core](../core/bsv_home_core/src/services/bsv_multisig)、[BsvMultisig Feature](../ios/Features/BsvMultisig/Sources)，保留固定 signer 认证作用域 |
| API 与错误/取消边界 | [MultisigClient](</Users/gavin/workspace/wowpay-android/android/features/asset/viewmodels/src/main/kotlin/com/gemwallet/android/features/asset/viewmodels/multisig/MultisigClient.kt>) 20–60 | [Portal Core](../core/bsv_home_core/src/services/portal) 与受限 Native transport；不新增未经证实的端点/schema |
| 公开配置备份 | 上述 MultisigViewModel 635–645；[公开配置与导出界面](</Users/gavin/workspace/wowpay-android/android/features/asset/presents/src/main/kotlin/com/gemwallet/android/features/asset/presents/multisig/MultisigPages.kt>) 284–341 | [配置备份场景](../ios/Features/BsvMultisig/Sources/Scenes/BsvMultisigBackupScene.swift)与 Core 导出；不混同身份恢复 |

## 2026-10-14 合同与实施记录（历史）

以下为当时的产品、接口及实施记录；其成功结果只覆盖所记录的版本、环境和用例，不能作为阶段5或当前生产部署通过的证据。

历史接口参考：[接入文档](</Users/gavin/Downloads/IOS_BSV_MULTISIG_INTEGRATION.md>)。配套压缩包 [iOS-BSV-multisig-integration.zip](</Users/gavin/Downloads/iOS-BSV-multisig-integration.zip>)包含同版文档、`BSV_MULTISIG_FUNDING.md` 和 `fixtures/bsv-multisig-interop.json`。

## 产品与协议

- 仅 BSV 主网原生币，成员2–5人，门槛2至N，默认2/3；全员加入及各自确认公开配置后启用。名称trim后1–30字符，成员备注每项1–20字符、数量与成员数一致，只存本机。
- 当前 iOS 与 Android `master` 的开发环境均使用 Origin `https://api-dev.hj0252.com`、API 前缀 `/v2/api/portal/auth/multisig`；复用钱包签名登录、原始 Authorization token 和 iOS deviceType `4`。附件中不带 `/v2` 的路径为历史 V1 契约。禁止跨域重定向、缓存和写请求自动重放。
- 成员身份为当前普通钱包默认 BSV 公钥及P2PKH地址。列表、信任及异步结果按Origin、本地钱包和服务端钱包隔离；付款钱包选择不改变多签会话身份。
- 配置由Core独立计算：规范排序公钥的P2MS脚本、脚本正序SHA256 configurationHash、反序SHA256 scriptHash；服务器confirmed不能代替本机核对，配置变化阻断操作。
- 多签支出只接受主网P2PKH地址和固定金额。API金额为整数聪；提案与钱包版本独立。Prepare保存提案并使用持久幂等键，客户端仍须重建及核验交易，创建后不自动签名。
- 每名成员必须签全部输入；仅COLLECTING且本人未签时可签，只有发起人在COLLECTING可撤回，READY任一成员可核对并广播。撤回或到期不撤销已泄露的链上授权。
- 待办为本人未确认的DRAFT钱包、本人未签的COLLECTING提案及READY提案；不直接采用summary.pendingCount。余额由Portal UTXO受检查求和，失败保留陈旧标志。历史为完整分页提案记录，普通入金不产生提案。
- 公开配置二维码和备份采用相同P2MS JSON，仅记录收款策略，不能恢复成员关系、私钥或未决入金。移除旧包导入/分享、替代提案、全部余额、Paymail及多签目标支出。
- 非Tab、非弹窗页面直接使用系统原生导航栏、标题及返回行为；原型只约束页面内容，不自定义顶部导航样式。首页、资产和多签详情共用一个按当前钱包隔离的业务模型，活跃页面每30秒刷新、返回前台立即刷新，隐藏页面停止调度。

## 所有者与实施顺序

现有 `bsv_home_bsv::multisig` 拥有脚本、来源验证、Review、签名和确定性组装；`BsvHomeBsvMultisigService` 拥有Portal协作、权限、信任、版本、幂等及入金恢复。Portal auth独占token与身份登录。iOS只提供现有认证、受保护存储、导航和渲染端口，每个ViewModel最多持有一个业务Core service。

1. 补齐2–5人Policy/Descriptor、文档TransactionProposal和SignatureSet，并通过附件version2双输入向量；不得把旧version1/feeRate模型直接当作新协议。
2. 实现Keystore和Gateway文档多签/入金导出，复用默认身份、HD扫描和密钥派生；密码及私钥不离开既有安全层。
3. 接入Portal受限请求、错误包装/裸错误、个人待办、完整分页、六态配置信任、权限、版本冲突和操作意图恢复。
4. 生成iOS绑定后接入独立创建/加入/配置/签名/广播核对及普通入金页面，保留系统导航与表单输入；统一Style/Components与本地化。
5. 两轮清理、Core与iOS检查、模拟器验证，同步组件/样式/功能状态及本轮执行结果。

## 接口边界与部署状态

钱包与提案使用独立DTO/版本；Unix时间按秒，API金额与费用为整数聪。业务写入使用文档接口，prepare保存提案、proposal-page读取完整历史；客户端不使用旧proposals有限列表、旧包交换或替代提案。

Create没有服务端幂等键，结果不明先读取钱包列表核查，不能自动重放。Prepare先保存支付意图UUID；明确的首次参数/余额/版本拒绝可释放该意图，任何此前结果不明的操作仍保留原key，禁止改收款方或金额另建提案。成员备注保存失败后的创建恢复补存本机备注，不再POST创建。

Portal没有预约余额、入金提案或旧协作记录迁移接口。本期不补造这些能力。UTXO快照失败显式标记陈旧；提案广播状态核验签名门槛、确定性raw与txid的关联，但服务端确认数并非独立主网验收。429及已知业务限流启动60秒单调时钟退避，写请求不自动登录重放；查询、提案和入金均保持冻结身份。

客户端接口、真实Keystore/Gateway导出和Swift绑定已实现。服务端部署、真实多设备协作、实体设备认证及主网收发需要单独记录实际验收结果，不能用Mock通过代替。

## 普通入金与恢复

入口为接收→从普通钱包转入→付款钱包和金额→五分钟核对→真实认证→结果。支持HD和导入私钥钱包，排除观察钱包。Core取得Normal费率及至多1000个UTXO，排除一聪代币输出，独立核验付款方、目标配置、金额、费用、合计及找零。

签名后必须先可靠保存身份、payerId、完整Funding/Review、raw、txid和状态，再广播。iOS新存储使用Application Support独立命名空间、原子写入、完整文件保护并排除备份，不读取旧GRDB多签数据。保存失败不广播；同目标有未决记录时不新建付款。

重启后重新核验原记录并只恢复同raw/txid。付款钱包缺失、配置改变或数据不一致时阻断。Gateway只提交原raw，异常时查询原txid确认是否已接受，不重选输入或重新签名；广播成功但余额同步失败保留已广播状态，仅重试同步。认证签名期间发生配置变化时，提交前重新读取配置并阻断广播。找零回付款钱包，多签资金不计入成员个人余额。

## 验收

- 附件向量：amount42000、fee200、change57800、双输入、Review/签名/完整raw/txid逐字节匹配；来源、金额、费用、配置和签名篡改拒绝。
- Portal Mock：200业务失败、裸错误、401/forceLogout/LOGIN_REQUIRED、null与整数边界、版本冲突、会话切换、幂等恢复、超过200条分页、个人待办、撤回及广播权限。
- 普通入金：HD/私钥、观察钱包排除、核对过期、保存失败零广播、响应丢失与重启同交易恢复、同步失败不重复付款。
- iOS：注册并实际执行BsvMultisigTests及相关回归，模拟器覆盖完整导航、返回保留输入、长公钥/地址、徽章及状态操作。
- 收尾命令包括受影响Core测试、clippy、格式检查、`just ios generate-core`、`just ios localize`、`just ios build` 和相关iOS测试。服务端部署、Android互操作、实体认证及主网收发分别记录未验证范围，真实付款另需明确交易授权。

## 本轮实施记录

Core和iOS实现已落地；下列结果仅覆盖实际执行的本地回归与模拟器检查。附件version 2双输入向量已逐项核对Review、逐输入签名、完整raw和txid，测试夹具SHA256为`74eba24368b59e88c1bc645421bf81d81f96c285c655084d570f13b7a232ccfa`。真实Keystore/Gateway端口已接入，Swift不派生密钥或拼交易。

Core执行结果：

- `cargo test --manifest-path core/Cargo.toml --locked -p bsv_home_bsv -p bsv_home_bitcoin -p signer -p bsv_home_core --all-features --lib -- --skip integration_tests`：bsv_home_bsv 69项、bsv_home_bitcoin 58项、signer 21项通过；bsv_home_bitcoin 9项链上集成测试按条件过滤。该轮Gemstone 948项通过、3项真实Portal/Paymail测试忽略。
- 随后在`core/`执行`cargo test --locked -p bsv_home_core --lib --all-features`：957项通过、0失败、3项真实服务测试忽略。最后补充三个恢复/配置竞态用例后，`cargo test --locked -p bsv_home_core --lib --all-features services::bsv_multisig::tests`：最终31项通过、0失败。覆盖创建成功但备注保存失败只补存、广播接受但状态保存失败只恢复原raw、认证期间门槛或公钥变化零广播。
- `cargo clippy --locked -p bsv_home_core --all-features --all-targets -- -D warnings`与`cargo clippy --manifest-path core/Cargo.toml --locked -p bsv_home_bsv -p bsv_home_bitcoin -p signer --all-features --all-targets -- -D warnings`通过。`core/`中的`just format`及`cargo fmt --all -- --check`通过。

iOS执行结果：

- `just ios generate-core`、`just ios localize`和`SIMULATOR_NAME='WowPay Multisig QA' just ios build`通过；最终构建包含入金结果保持与导航路径一次性更新修复。
- `SIMULATOR_NAME='WowPay Multisig QA' just ios test 'BsvMultisigTests -only-testing BsvHomeCoreServicesTests -only-testing TransferTests -only-testing AssetsTests -only-testing WalletTabTests -only-testing ManageWalletsTests -only-testing ComponentsTests -only-testing FormattersTests -only-testing SwapTests'`：614项通过、0失败、1项实体设备文件保护测试明确跳过。随后入金结果生命周期修复，在`ios/`执行`SIMULATOR_NAME='WowPay Multisig QA' just test BsvMultisigTests`：专项回归19项（22次参数化执行）通过。没有把修复前的完整结果计作修复后的全套重复执行。
- 最后一轮将资产多签Tab徽章统一复用`WowPayCountBadge`，尺寸随字号缩放；`SIMULATOR_NAME='WowPay Multisig QA' just ios test 'BsvMultisigTests -only-testing ComponentsTests -only-testing AssetsTests'`：91项（108次参数化执行）通过、0失败、0跳过。
- 31个语言资源的192个多签键、重复键和占位符核对通过。Swift格式/解析检查与`git diff --check`通过。完成验证前、验证后的两轮清理，移除旧包操作入口及重复页面标题，保留原历史记录与个人钱包/密钥。

模拟器检查使用iPhone 17 / iOS 26.4.1。隔离的Mock宿主直接加载真实Feature页面，检查创建至5人、移除正在编辑的成员、创建/加入核对默认禁用提交、原生返回保留输入、成员完整公钥、配置与备份、提案/签名/广播权限状态，以及普通入金→核对→模拟认证→结果。入金结果显示原金额与txid，切页不清除结果；成功后只有进入钱包入口，余额同步失败仅提供重试同步。26个路由/状态、深色外观和辅助功能大字号均有截图核对。Mock不构造真实签名或网络Gateway，公开收款JSON协议正确性另由Core向量验证，不以Mock二维码代替协议验收。

另在真实App构建中使用仅含公开地址的观察钱包，实际验证首页、“我的”和普通BSV资产页的多签列表入口、独立创建/加入页面、系统导航栏及返回资产根页。观察钱包显示不能签名的明确错误，没有建立真实Portal成员会话或发起支付；持钥认证、协作与付款状态由隔离Mock和Core回归覆盖。

未验证范围分别保留：服务端部署及真实多设备Portal协作、实体设备认证/锁屏文件保护、Android互操作、真实主网收发。模拟器实际读取文件保护属性为nil；生产存储仍使用原子完整文件保护写入及目录/文件显式保护设置，不能据此宣称实体保护已验收。三个真实Portal/Paymail测试未启用；没有执行真实付款。Core坏记录、未知状态和余额获取失败均显式报错或标记陈旧，不静默跳过记录；未运行分支由上述忽略、过滤和实体设备限制界定。

## 2026-11-06 加密 Keystore 全链路回归

新增 `services::bsv_multisig::tests::keystore::test_independent_keystores_create_join_confirm_sign_all_inputs_and_broadcast`，使用三个相互独立的临时加密 Keystore、钱包、会话和本机信任存储，共享测试 Portal 状态。测试从空多签列表开始，实际调用创建、邀请、加入、三方配置确认和双输入提案；第二、第三成员分别通过真实 `BsvHomeKeystore::bsv_multisig_sign` 签全部输入，第一成员重新加载提案后组装并提交原始交易。测试没有预先注入 READY 或成员签名。逐成员签名集、最终 raw 和 txid 与独立 SDK 夹具逐字节一致。另验证不足门槛不能广播、已签成员不能重签、重建服务保留信任与提案、成员无法从自己的 Keystore 读取另一成员密钥，以及每个 V2 请求携带其自身隔离的 Authorization 会话。

本地组合测试使用公开合成测试密钥、注入的已登录测试会话及测试 Portal/链响应，不代表真实服务端协作或主网接收成功。系统认证端口和本机偏好存储也仍为测试实现。新建真实身份的 V2 实调另行执行：`POST /v2/api/portal/wallet-auth/challenge` 返回 HTTP 200、`OK`，但 `data` 为空，缺少挑战字段；Core 明确返回 `InvalidResponse`，没有签名、登录或发出受保护的多签/Paymail 请求。未带 token 的探测返回 401 属于预期结果，不作为此失败的原因。

本轮执行结果：

- `cd core && cargo test --offline --locked -p bsv_home_core --lib --all-features services::bsv_multisig::tests::keystore::test_independent_keystores_create_join_confirm_sign_all_inputs_and_broadcast -- --exact --nocapture`：新增组合测试通过；清理两处无效 clone 后重复该测试仍通过。
- `cd core && cargo test --offline --locked -p bsv_home_core --lib --all-features services::bsv_multisig::tests`：32 项通过，零失败、零忽略。随后 Core 全量为 BsvHomeCore 1,021 项加 bsv_home_bsv 81 项通过，零失败、五项受控实网测试默认忽略。
- `cd core && cargo test --locked -p bsv_home_core --lib --all-features services::portal::live_tests::test_live_v2_wallet_authentication_multisig_and_paymail_reads -- --ignored --exact --nocapture`：明确执行一项，零通过、一项失败，停在空挑战响应；其余用例按精确过滤未执行。未创建真实多签记录、提案或支付。
- 两项 iOS 正向桥测试实际调用加密 Keystore 签名并匹配独立 SDK 交易字节；完整 App 构建及日常端创建/观察钱包两项原生 UI 测试通过。全量和设备检查见 [统一包回归记录](BSV_COLD_WALLET.md#unified-app-regression--2026-11-06)。

当前验收结论为本地多成员签名和测试节点广播闭环通过，真实 Portal 协作、实体设备认证/扫码、专用测试资金的主网广播及独立链上确认尚未通过。新增测试没有改动生产鉴权、交易复核或签名门槛，也没有静默跳过坏记录或吞掉认证失败。

最终完整 iOS 回归为 1,310 项通过、零失败、一项实体文件保护测试跳过；日常创建/观察钱包原生 UI 回归两项通过。另在最终同一构建的日常模拟器内切换至新生成的持钥钱包，实际进入首页多签入口，显示本地化的“钱包身份验证失败，请重试后继续。”及重试入口，未进入签名或广播。初次加载失败时页面还显示空列表提示“还没有多签钱包”，该提示不能作为服务端已返回空列表的证据；应与鉴权错误一起解读，后续界面回归需覆盖这一状态。

## 历史实施记录

以下保留2026-10-13旧手工协作协议的结果，不适用于当前Portal契约。

## 11. 实施记录（2026-10-13）

实际所有者为 [bsv_home_bsv::multisig](../core/crates/bsv_home_bsv/src/multisig/mod.rs)、[脚本级链服务](../core/crates/bsv_home_bsv/src/rpc/multisig.rs)、[BsvHomeBsvMultisigService](../core/bsv_home_core/src/services/bsv_multisig/service.rs)、[受限 Keystore 签名](../core/bsv_home_core/src/keystore/multisig.rs) 和 [iOS BsvMultisig](../ios/Features/BsvMultisig/Package.swift)。Core 管理配置、审批、冻结提案、验签、占用及链上观察；iOS 的独立全局快照 Store 提供原子 CAS，UI 仅消费展示模型和明确动作。

实际接口包括创建/邀请、包预览与导入、配置确认/导出、收款请求、真实费用提案/替代提案准备、审核明细、签名与签名包导出、分享、停止协作、广播和刷新。`current_signing_identity` 在创建/加入前取得已验证的本机签名钱包ID与公钥供用户审核；`create_wallet` / `join_invitation` 必须携带这份审核身份的 `signer_wallet_id`，当前钱包不匹配时拒绝操作。`get_local_signers` / `select_signer` 显式管理同脚本关联的本地持钥钱包；`related_multisig_count` 用于个人钱包删除警告。创建与加入复用当前受控 BSV 钱包，后续签名复用明确选择的成员钱包。`get_history` 返回校验归属的脚本交易及真实确认数。`balance_satoshis` 为脚本总余额，`available_satoshis` 只包含已确认、成熟且未占用的已验证 UTXO；未取得快照时不冒称为零。钱包页“待处理交易”只统计该钱包未结束提案，列表通用待处理和首页徽章还包括未完成成员确认。

冻结的资源契约：

- 解码后 JSON 最多 4 MiB；传入文本最多 8 MiB；最多32层，协议数字只接受不超过 `2^53-1` 的非负整数。金额、前序金额及费用使用规范十进制聪字符串。文件在读取前检查长度，并于读取后再次检查。
- 每个冻结计划最多100个输入，每份前序或最终交易最多2,000,000字节；金额不超过 `2,100,000,000,000,000` 聪。超限或分页/提供方部分错误均报错，保留已有快照，不将失败当作零余额。
- 协作文件导出为 `.txt`，内容为完整 UTF-8 前缀文本，与复制包使用同一结构；原始 JSON 和前缀文本都可导入。
- 收款二维码文本最多2000字节；无Logo的多签请求使用M纠错并按屏幕像素密度生成，其他既有收款保留H纠错。超限或生成失败使用真实文本复制/文件分享；不截断或生成另一个支付目标。
- `activation.approvals` 保存全部成员的精简 `publicKey` / `signature` 证明；验证时重建同一完整配置的成员确认封套，逐个验证原领域和包摘要。它与单独传递的完整 `configuration-approval` 包语义一致。
- `configurationId` 绑定公开资金配置（包括名称及成员标签，但排除草案ID）；资金记录按主网规范脚本去重。外部同脚本不同配置拒绝覆盖，重复新建复用已有资金记录。成员确认的封套证明仍绑定其完整配置及草案ID，不能将另一个草案的确认合并。
- 普通发送只对经过完整启用证明验证的 `receive-request` 生成内部 `bsv-script:<hex>` 目标；该目标只允许规范的2/2、2/3或3/3脚本，原 P2PKH 地址校验保持原规则。

已执行低层验证：`cargo test --locked -p bsv_home_bsv -p bsv_home_bitcoin --all-features --lib`：58 + 67 测试通过，无忽略；对应两crate的 `cargo clippy --locked ... --all-features --all-targets -- -D warnings` 通过。空脚本历史收尾修复后，`bsv_home_bsv` 全量62项通过；等价 lint 清理后脚本RPC11项再次通过，最终全目标 clippy 通过。独立 BSV SDK 验证全部支持的成员子集和双输入，并拒绝错误 dummy、签名顺序、版本及 sighash；字节向量及复现脚本保存在 `bsv_home_bsv/testdata/multisig_sdk_vectors.*`。隔离临时源码副本放宽四成员限制时，契约回归测试实际失败，正式源码保持约定。

公开链服务预检使用公共链上样例，确认逆序 SHA256 脚本索引、UTXO、confirmed/unconfirmed history、分页 token、raw transaction 和 unspent outpoint 的404语义。现场未使用的公共多签脚本confirmed首页返回HTTP404且正文精确为`Not Found`，同一脚本unconfirmed/UTXO合法返回空；另一个已有确认记录的公共脚本同端点返回200。RPC仅将该confirmed首页精确响应视为空，并继续严格校验unconfirmed；后续页404、其他404正文、500、无效JSON/脚本归属错误均失败。这是现场观察的服务语义，官方文档没有明确空历史404保证。完整多页遍历由 Mock 回归验证，公开主网历史未翻完所有页；没有提交任何交易或读取真实钱包秘密。

Core服务完整回归（收尾补充前）：`cargo test --manifest-path core/Cargo.toml --locked -p bsv_home_core --lib --all-features` 为935通过、0失败、3个既有 Portal/Paymail 实网测试依门禁忽略；新增多签服务21项均通过。`cargo clippy --locked -p bsv_home_core --all-features --all-targets -- -D warnings` 通过。全员确认规则在临时弱化至门槛M后，回归实际失败，已恢复原规则。最终整仓 `cargo fmt --all -- --check` 通过。普通付款收尾新增固定金额请求约束：请求指定金额时必须精确到聪匹配，禁止Max；未指定金额保留原Max行为。补充回归覆盖精确匹配、正负1聪篡改和Max，`services::send` 最终8项通过。

服务回归覆盖三种门槛及双输入真实 Keystore 签名、公开配置恢复、认证取消、provider失败与缓存、签名子集的实际消费者、共享提案占用、伪替代关系拒绝、旧协作通知/迟到广播响应不得倒退链状态、提交前CAS失败、网络接受后结果CAS失败的原txid恢复且不重发。配置确认还绑定草案 transcript；同配置ID但不同草案不能污染已有审批，同ID外部提案导入必须保持共享风险。持久包查找的7处吞掉body解析错误已移除，审批、部分签名和提案缓存均严格检查kind和body；即使坏记录位于已匹配记录之后也报Storage错误并保持原快照。三个新增损坏回归与原21项服务回归合计24项全部通过，最终Gemstone全目标clippy通过。持久Store返回IO错误、两个独立新提案同时争抢真实GRDB revision未单独模拟；已执行Core CAS拒旧revision及iOS Store事务回归。

### 11.1 iOS 执行结果

从仓库根目录执行 `just ios generate-core`、`just ios localize` 和 `just ios build`，均通过；最新静态库生成包含收尾的金额、缓存和空历史修复。UniFFI契约已生成，未改TypeShare模型，因此没有运行不适用的`generate-models`。本地化31个locale各162个多签键，严格生成及缺失/占位符检查通过。

主回归测试命令为：

```bash
just ios test 'StoreTests -only-testing BsvHomeCoreServicesTests -only-testing TransferTests -only-testing AssetsTests -only-testing WalletTabTests -only-testing ManageWalletsTests -only-testing ComponentsTests -only-testing FormattersTests'
```

结果为630项测试通过、700次参数化执行、0失败、0跳过，覆盖Store CAS、现有密钥恢复/受限签名桥、普通发送多签收款请求、取消/过期请求，以及组件和金额格式回归。二维码测试真实生成并解码完整短请求和约1.5KiB长请求，验证超量返回失败而不截断。首轮Vision解码在模拟器报`Could not create inference context`，改用Core Image解码后全部通过；该失败属于测试解码环境，不作为资金功能通过证据。

导航收尾将多签目的地注册移到App主栈的稳定层，资产Tab内容和目的地共用同一个配置模型及对应的NavigationPath。之后再次执行 `just ios build` 和 `just ios test 'AssetsTests -only-testing WalletTabTests'`，均通过；后者为163项测试、193次参数化执行、0失败、0跳过。

模拟器使用独立iPhone 17/iOS26.4环境、公开观察账户和经过Core验证的公开2/2配置。已复核首页及“我的”入口、BSV五Tab/默认多签选择、零徽章隐藏、配置预览/幂等导入、钱包8位BSV数量、创建表单的2/2与2/3、3/3交互、成员规则、备份、真实收款二维码及空历史。已实际完成“钱包→成员→备份→返回成员→返回钱包→接收/历史”的连续导航；返回列表后再次打开钱包和成员页，系统导航栏及返回功能保持正常。设置的偏好子页面也已复核原生标题、返回按钮和返回Tab根页面。最终SwiftUI日志没有再出现缺失多签目的地注册的导航错误。未接收或发送资产，未创建真实密钥、更改用户偏好或接受条款。

真实多设备认证及专用主网小额广播仍需独立验收；观察账户不覆盖持钥设备的创建/加入/认证签名及广播界面全部分支。12路由代码及原型样式映射已落地，未将部分公开数据页面的截图核对宣称为全部路由视觉验收；实体设备、小屏、大字号、RTL和深色外观没有执行。本轮Android未检查和验证，共享Core对其他消费者的影响未经验证。
