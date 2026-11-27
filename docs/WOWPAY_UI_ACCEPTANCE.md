# WowPay UI Acceptance Audit

Current prototype: [`prototype/index.html`](</Users/gavin/Downloads/prototype/index.html>), with the complete package indexed in [Current Prototype](WOWPAY_UI_REFACTOR.md#current-prototype--2026-11-15). The 2026-11-15 replacement updates prototype references and documentation only. Its top wallet selector, collective-wallet pages and creation/import identity proposals have no new native implementation or acceptance result from this replacement. The earlier user-approved BSV multisig implementation used WowPay 5 and its integration contract; that execution record and its remaining boundaries are in [BSV Multisig](BSV_MULTISIG_PLAN.md#本轮实施记录). Route, modal and action inventories below retain their original prototype versions and verification scope; they are not an inventory or acceptance audit of the current package.

Recorded audit date: 2026-09-18; import-route update: 2026-10-01. Audit source: the earlier user-supplied WowPay prototype and subsequent user-provided import screenshots. Scope: iOS light appearance, Simplified Chinese, and the accepted native-wallet boundaries in [the migration contract](WOWPAY_UI_REFACTOR.md). Source coverage, compilation, runtime checks and final visual approval are different milestones. This file does not certify full acceptance merely because a route has an implementation.

## Page Inventory

The earlier HTML used for this audit registers 18 routes. Paths below are relative to the repository root. Earlier runtime evidence is recorded in the migration contract; this recorded audit adds the controls and checks described below.

| HTML route | Native owner | Coverage and acceptance boundary |
|---|---|---|
| `home` | `ios/Features/WalletTab/Sources/Scenes/WowPayHomeScene.swift` | Four quick actions, balance/privacy, promotion, services and news present. Actual portfolio replaces demo balances; unavailable content uses feedback. Four-tab visual polish remains deferred. |
| `assets` | `ios/Features/WalletTab/Sources/Scenes/WalletScene.swift` | Real asset rows, search/manage/add, privacy, existing wallet selector and capability-gated operations retained. BSV/MNEE are separate unavailable entries. Native scanner occupies the header action; notifications remain in profile. |
| `bsv` | `ios/Features/WalletTab/Sources/Scenes/WowPayUnavailableScene.swift` | Overview/address/activity/about tabs. This audit replaces the address-tab jump-only presentation with the same address form used by profile, adds the about card, recent-activity action and more menu. Values remain unavailable. |
| `send` | `ios/Features/Transfer/Sources/Scenes/RecipientScene.swift`, `AmountScene.swift`, `ConfirmTransferScene.swift`; deferred `.send` | Real supported send keeps recipient → amount → confirmation. Deferred BSV includes recipient, scan, contacts, amount, all/percentage actions, note and fee surface. All unsupported actions give feedback; no signing or invented fee. |
| `swap` | `ios/Features/WalletTab/Sources/Scenes/WowPaySwapLandingScene.swift`; `ios/Features/Swap/Sources/Scenes/SwapScene.swift` | Two amount/asset cards, direction switch, reference/fee placeholders, quick pairs and supported-swap entry. This audit restores all and notification actions. Real quotes/confirmation require supported assets. |
| `receive` | `ios/Features/Transfer/Sources/Scenes/ReceiveScene.swift`; deferred `.receive` | 2026-09-22 supported-page alignment uses centered title/subtitle, standalone bordered QR with Logo, separate real address, network guidance, inline actions and Paymail card. The user-approved prototype email is display-only; Paymail remains unavailable. Supported Solana/BTC QR payloads, clipboard, share sheet and native return were verified. BSV retains its unavailable page; no HTML demo QR/address is used. |
| `profile` | `ios/Features/Settings/Sources/Settings/Scenes/SettingsScene.swift` | Wallet summary, create/import/Paymail/address/activity entries plus existing settings. Real wallet count replaces prototype account claims. |
| `welcome` | `ios/Features/Onboarding/Sources/Scenes/OnboardingScene.swift`, `ImportWalletWelcomeScene.swift` | Initial onboarding remains separate. 我的 → 导入钱包 opens a pushable welcome page with back, notifications, and create/import actions. Its import action pushes the new method page; create retains the existing secure flow. |
| `import-method` | `ios/Features/Onboarding/Sources/Scenes/ImportWalletMethodScene.swift`; original `ImportWalletTypeScene.swift` retained | The profile-entry page renders four prototype-style rows. Phrase, private-key and watch-only rows push a type-specific input page; Keystore gives unavailable feedback. The original onboarding selector is unchanged. |
| `import-seed` | `ios/Features/Onboarding/Sources/Scenes/ImportWalletInputScene.swift`; original `ImportWalletScene.swift` retained | Shared type-specific input page, real import model and Core validation. Phrase has 12/15/18/21/24 selectable counts; private key and watch address select a supported network first. Demo filler is replaced with secure paste and real safety text. |
| `import-success` | Existing import completion → selected wallet | No unconditional standalone demo-success page. Visual success/return after an actual recovery remains untested. |
| `create` | `ios/Features/Onboarding/Sources/Scenes/SecurityReminderScene.swift`, `ShowSecretDataScene.swift`, `VerifyPhraseWalletScene.swift`, `SetupWalletScene.swift` | Existing secure generation/backup route, WowPay presentation. Reminder reviewed previously; secret-bearing screens were not captured. |
| `create-success` | Existing create completion → selected wallet | Success requires actual creation. Live creation/completion remains untested. |
| `paymail` | Deferred `.paymail` | Heading, description, name field, save and unavailable notice. Input is transient; no registration or fake persistence. |
| `address` | Deferred `.address` | BSV-address card, unavailable address surface, copy, history heading, add and empty state. Reused directly by BSV address tab. Real contacts remain a separate feature. |
| `transactions` | `ios/Features/Transactions/Sources/Scenes/TransactionsScene.swift`, `TransactionsFilterScene.swift`, `TransactionScene.swift`; deferred `.transactions` | Real list/filter/details preserve data and status semantics. Empty/network/type/clear states are audited separately; nonempty real detail remains untested. Deferred BSV list contains no fake records. |
| `scan` | `ios/Features/QRScanner/Sources/QRScannerScene.swift` | Existing camera/gallery/permission/cancel behavior retained; no demo-recipient button. Simulator cannot establish physical-camera scanning correctness. |
| `asset-info` | `ios/Features/Assets/Sources/Scenes/AssetScene.swift`; deferred `.mnee` | Native supported asset details retain existing capabilities and real values. MNEE is an unavailable identity/balance surface; BSV-specific address tabs are no longer shown for MNEE. |

## Modal and Inline State Inventory

| Prototype state | Native equivalent / accepted behavior |
|---|---|
| `menu` | BSV/send native more menu with receive and transaction-history destinations; standard back navigation retained. |
| `choose-from`, `choose-to` | Landing native asset menus; supported swap keeps its real selectors. No simulated quote is generated from landing selection. |
| `contacts` | Supported recipient scene keeps existing contact selection. Deferred BSV contacts action says unavailable; prototype demo contact is not injected. |
| `send-confirm` | Existing native confirmation/auth/signing for supported assets. Deferred BSV confirmation gives unavailable feedback. |
| `swap-confirm` | Existing native supported swap quote/confirmation. Unsupported pairs give unavailable feedback. |
| `success` | Only the real completed operation may report success. No prototype-only success modal. |
| BSV `overview/address/transactions/about` | Four native segments; shared address content, unavailable activity and asset identity/about card. |
| Balance visible/hidden | Existing persisted privacy preference and shared balance renderer. |
| Import seed count/method | The profile import page offers all five Core-supported mnemonic lengths and uses the existing import model; private key and watch address select a supported network before input. No second parser or fake completion. |
| Empty/loading/error/filtered activity | Existing feature models and empty components; WowPay only changes presentation. Nonempty status rendering has earlier fixture/test evidence, not a live-wallet acceptance claim. |

## Action Coverage

The audited source declares 28 literal `data-action` names, plus dynamic asset-choice actions, `data-go`, `data-tab`, `data-preset`, `data-pair`, `data-method`, `data-seed-count` and `data-choose` handlers.

| HTML actions | Native handling |
|---|---|
| `back`, `dismiss-modal` | Native navigation back, sheet close/cancel and menu dismissal. |
| `toggle-balance` | Existing privacy binding; real balances only. |
| `notify` | Existing profile notifications; unavailable feedback on prototype-only header actions. |
| `menu` | Restored BSV/send receive/history menu. |
| `max-send`, `max-swap`, percentage presets | Real flow retains its computed maximum. Deferred actions retain buttons and report unavailable. |
| `switch-assets`, dynamic `choose-*`, `data-choose` | Landing display selections and direction switch; real supported selection remains owned by Swap. |
| `confirm-send`, `confirm-swap` | Existing real flows or unavailable feedback, depending on capability. |
| `send-success`, `swap-success`, `close-success` | Existing actual operation completion and return. Prototype success shortcuts intentionally excluded. |
| `contacts`, `contact-fill` | Real contact selection or unavailable feedback. No demo address inserted. |
| `scan-fill` | Real scanner input in supported flow; unavailable feedback in BSV. No synthetic result. |
| `copy-address`, `share` | Real receive payload in supported flow; unavailable feedback in BSV. |
| `save-paymail`, `new-address` | Visible action with unavailable feedback and matching existing TODO register. |
| `demo-seed`, `import-complete`, `create-complete` | Existing validated import/create only; demo filler and unconditional completion excluded. |
| `asset-options`, `add-asset` | Existing native asset management. |
| `news`, `more-services`, `service` | Unavailable feedback with existing deferred register. |
| `data-go`, `data-tab` | Native navigation and four BSV segments. |
| `data-pair` | Quick-pair button with unavailable feedback. |
| `data-method`, `data-seed-count` | The new profile-entry method and word-count selection use real input validation; the original onboarding network-first route remains available. |

## Remaining Acceptance Work

- User review of the four-tab/secondary-form polish and screenshot comparison below. The recorded insufficient-balance Markdown delimiters and English invalid-phrase error have been fixed; verification boundaries are recorded separately below.
- Real populated transaction detail, pending animation and all specialized transaction layouts on a device. Earlier static fixture rendering and model tests are not equivalent to runtime coverage.
- Real import/create/backup/success paths with a dedicated test-wallet procedure; secret-bearing screens must not be captured in audit artifacts.
- Real camera permission and QR scanning on a physical device; supported transaction/swap completion requires a separate functional-validation scope.
- Small-screen and large-text coverage for all secondary states; earlier SE welcome/terms evidence does not cover the entire app.
- Supported native multistep flows, platform sheets and retained legacy entries follow the approved adaptation boundary. This audit does not certify pixel-identical HTML reproduction.

## Current Batch Evidence

Simulator: iPhone 17, iOS 26.4, Simplified Chinese. The existing wallet was retained. No wallet was imported/created/deleted, no secret was revealed, and no transaction was signed or submitted.

| Check | Result / limit |
|---|---|
| Profile → activity → type filter → transfer → confirm | Passed: returned to filtered-empty state with active filter indicator. This extends the preceding batch's network-filter evidence. |
| Filtered-empty → clear | Passed: original empty state and buy/receive actions restored; filter indicator returned to inactive. |
| Activity → back | Returned to profile; subsequent native sheet open/close checks also passed. |
| Paymail input/save/close | Passed with non-secret synthetic text `ui-review@example.invalid`; software keyboard appeared, Return dismissed it, save showed “暂未开放”, and close returned to profile. This is not a claim that every keyboard/scroll layout passed; small-screen coverage remains open. |
| Address copy/add/close | Passed: both actions show unavailable feedback; no address record or success is fabricated; close returns to profile. |
| BSV address/about/activity segments | Passed: address card/actions render directly in the tab, about contains asset identity, activity is empty/unavailable. Overview “查看更多” changes to the activity segment. |
| BSV/send more menus | Both entries visible. BSV menu → receive and send menu → history were opened, then returned through the existing stack. |
| Deferred send contacts/all | Passed: visible, clickable, unavailable feedback; amount remains unpopulated. |
| MNEE detail | Passed: unavailable identity/balance, without BSV address tabs; back returns to assets. |
| Swap all/notification | Passed: visible actions show unavailable feedback. No quote or balance was synthesized. |
| Home scanner open/close | Passed for simulator unsupported-camera fallback with photo-selection entry; close returns to home. Camera permission, gallery decoding and physical scanning remain untested. |

Retained screenshots are ignored local artifacts under `ios/build/wowpay-full-audit/`: `address.png`, `bsv-address-tab.png`, `swap-controls.png`. Toast and navigation checks were also inspected through simulator screenshots/accessibility during this run; the retained screenshots are not a complete recording of every interaction.

Compilation: `cd ios && just build-package WalletTab` passed during iteration; `cd ios && just build` passed after the final Swift edits. Logs: `/tmp/wowpay-audit-package.log` and `/tmp/wowpay-audit-app.log`. The targeted regression result is recorded in the migration verification record.

Cleanup reviewed both changed scene owners, their existing consumers, documentation links and inventory. The batch reuses existing style/localization tokens and the address content owner; no new style system, component API, localization key, Core change or Android UI change was introduced. Scoped SwiftFormat lint passed; final whitespace/link checks are recorded in the migration record.

## Visual Polish Evidence — 2026-09-18

This supersedes the preceding batch's pending typography/error items. Four-tab and Paymail reference/runtime screenshots are available in the local [comparison report](../ios/build/wowpay-visual-polish/review.html). Component and style inventories are synchronized.

| Check | Result / limit |
|---|---|
| Four main tabs | Reviewed against HTML: supplied bell assets, shared balance shadow, quick-pair padding and asset-heading placement. Actual balances/native entries remain; first-screen screenshots are not full scroll-state coverage. |
| Empty-name wallet selector | Final installed app displays “钱包” with a 44 pt minimum target; no wallet rename or persistence change. |
| Long name / full balance / long button | Static shared-component samples inspected at 320, 375 and 402 pt. Asset text stacks/wraps and buttons expand. These are synthetic fixtures, not all pages running on three devices. |
| Swap decimal keyboard | Software keyboard displays a working “完成” action; no real quote submitted. |
| Paymail form and keyboard | 12 pt description reviewed; input accepts non-secret `ui-review@example.invalid`, Done closes software keyboard, save shows “暂未开放”, close returns to profile. No Paymail record created. |
| Invalid mnemonic localization | Real empty submission displays “验证错误: 助记词无效”. Typed service/mapping regressions pass; no wallet imported. |
| Insufficient balance formatting | Actual simplified/traditional Chinese source and generated templates parse with asset emphasis and no literal `**`. Not claimed as a new live transaction check. |
| Final iOS checks | Full app build passed; 201 targeted tests passed, zero failures/skips. Core: 665 passed, zero failures/ignored. Exact commands/result paths are in the migration record. |
| Android shared error integration | Kotlin consumer/bindings compile passed. Full native build/generation blocked by missing Android toolchain (`cargo ndk` unavailable); APK/runtime remains unverified. |

No secret-bearing screenshot, wallet reset or actual transaction was used for this audit. The open acceptance items above remain: full secondary-page small-screen/large-text coverage, populated/specialized transaction states, physical scanning and protected recovery/create/backup/completion paths. The screenshot comparison supports user review rather than certifying pixel-identical reproduction or complete functional readiness.

## Dual-Currency Card Follow-Up — 2026-09-18

Home and Assets now show fixed USD total with CNY equivalent, independently of global pricing settings. Configuration, extension instructions and `WOWPAY-TOTAL-ASSET-SCOPE` product TODO are recorded in [the migration record](WOWPAY_UI_REFACTOR.md#dual-currency-total-assets--2026-09-18). The footer uses a currency label rather than a BSV identity.

Full iOS build passed; 29 targeted iOS tests and 666 Core tests passed with no failures/skips. Both cards and shared privacy/accessibility masking were exercised on the existing zero-balance simulator wallet; original privacy preference was restored. Safe local screenshots: [Home](../ios/build/wowpay-dual-currency/home.png), [Assets](../ios/build/wowpay-dual-currency/assets.png). Nonzero conversion, rate changes/failures and wallet isolation have deterministic test coverage, not live funded-wallet evidence. Android Kotlin assets consumer compiled; full native build remains blocked by missing cargo-ndk. Exact commands are in the migration record.

## Balance Symbol, Stable Privacy Height and Prototype Icons

Both root balance cards now opt into `$` for USD while retaining two-decimal/localized number formatting. Supplied eye/eye-off assets replace SF Symbols. The permanent render regression first reproduced the original 147 → 149 pt height change, then passed after preserving layout across real/masked text. It checks 320/375/402 pt widths, short/long primary values and short/wrapped secondary values.

The final full app build and 30 tests (34 parameterized runs) passed with zero failures/skips. Simulator checks confirm repeated toggles, root-tab synchronization, prototype icons, `$0.00` and hidden accessibility text with no real amounts. The original visible preference was restored. Local evidence: [Home visible](../ios/build/wowpay-balance-fixes/home-visible.png), [Home hidden](../ios/build/wowpay-balance-fixes/home-hidden.png), [Assets visible](../ios/build/wowpay-balance-fixes/assets-visible.png), [Assets hidden](../ios/build/wowpay-balance-fixes/assets-hidden.png). Commands/results and configuration are in the migration record. Other untested device/functional scopes above remain unchanged.

## BSV Multisig Navigation Contract — 2026-10-14

All non-tab, non-modal multisig pages use the system navigation bar, title and back behavior. WowPay 5 defines content and interactions; it does not authorize custom navigation-bar styling. Current Portal collaboration, dedicated funding/recovery, protected storage, Dynamic Type and test status are maintained in [the current multisig contract](BSV_MULTISIG_PLAN.md), with older offline-package results retained only as history.
