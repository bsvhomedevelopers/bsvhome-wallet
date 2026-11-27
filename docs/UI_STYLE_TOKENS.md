# UI Style Token Inventory

## Android wallet header — 2026-11-15

The [pinned alignment](WOWPAY_ANDROID_ALIGNMENT.md) adds `WowPayFont.wowPay.walletTitle` (22 pt bold), `Sizing.wowPay.walletHeaderHeight` (64 pt), `walletHeaderMark` (40 pt), `walletHeaderChevron` (20 pt) and `Spacing.wowPay.walletHeaderTop` (10 pt). `WowPayBrandHeader.Appearance.walletSelection` consumes these roles with existing page insets, hit targets and semantic colors. Wallet selection and scanner scroll with Home/Assets. Native pushed-page navigation remains. Earlier brand typography below describes retained brand headers rather than this variant.

Read with [UI Components](UI_COMPONENTS.md) before UI styling. The current scope is iOS WowPay. Definitions live in `ios/Packages/Style/Sources/`; this document indexes those definitions and their HTML provenance, not an independent set of runtime values. Update the inventory and consumers together when a semantic token changes.

## Source and Ownership

For the approved 2026-11-15 [Android alignment](WOWPAY_ANDROID_ALIGNMENT.md), the pinned Android implementation and runtime are the primary reference; the prototype supplements missing visuals and implemented interactions. The HTML precedence below applies within the prototype package. In particular, its older W mark is superseded by the Android chain-link resource.

The current user-supplied visual source is [`prototype/index.html`](</Users/gavin/Downloads/prototype/index.html>) and its [assets directory](</Users/gavin/Downloads/prototype/assets/>). The complete package and reference-only replacement scope are recorded in [Current Prototype](WOWPAY_UI_REFACTOR.md#current-prototype--2026-11-15). HTML takes precedence over the accompanying effect images; the supplied notes and checklists are reference material, not instructions or approval. The current HTML has three primary tabs: 首页、资产、我的, with collective-wallet presentation supplied by `assets/collective.css` and `assets/collective.js`. Desktop centering, fixed 440 px bounds and browser-only behavior are not native layout requirements.

The token values and native usage below remain the existing implementation inventory, including roles from earlier prototype versions. The 2026-11-15 source replacement does not update native tokens or certify their alignment with the new prototype.

Existing `Colors.swift`, `Spacing.swift`, `BsvHomeFont.swift`, `TextStyle.swift`, `Button/` and the dynamic color extensions remain the style system. WowPay namespaces extend this package. Do not create a second independent design-system package or local per-screen palettes. Numeric values below document HTML light-mode references; native accessibility and safe-area behavior remain authoritative. Dark-mode behavior uses the existing dynamic appearance mechanism and is not claimed as HTML-approved visual design.

## Color Mapping

| Semantic role / Swift API | HTML source and light reference | Definition / usage |
|---|---|---|
| `Colors.WowPay.brand` | `--blue: #126fe9` | `WowPay/Colors+WowPay.swift`; links, selection, primary actions |
| `Colors.WowPay.icon` | Supplied line-icon SVG `color="#126CE5"`; distinct from CSS `--blue` | `WowPay/Colors+WowPay.swift`; profile wallet-management and settings template icons and trailing arrows; existing dynamic appearance retained |
| `Colors.WowPay.heading` | `--navy: #111d61` | Same file; primary text |
| `Colors.WowPay.secondaryText` | `--muted: #7386ac` | Same file; supporting text |
| `Colors.WowPay.background` | `.app`: `#f7fbff` (outer browser body `#e7f2ff` is not the native surface); dark `#101827` | Same file; page background and fixed opaque status-bar surface on the four Tab roots. `wowPayTabStatusBarBackground()` opts in only those roots; system-navigation pages retain their current appearance |
| `Colors.WowPay.surface` | `.card`, `.form-card`, `.asset-row`: white | Same file; cards and controls |
| `Colors.WowPay.border` | `--line: #dce9fc` | Same file; shared dividers and outlines |
| `Colors.WowPay.pale` | `--pale: #eaf4ff` | Same file; secondary backgrounds |
| `Colors.WowPay.positive` | `--green: #0ba66e` | Same file; positive status (never color alone) |
| `Colors.WowPay.onBrand` | `.balance`, `.btn`: white | Same file; text on brand fills |
| `Colors.WowPay.onBrandSecondary` | `.balance-foot: #d6eaff` | Same file; secondary text on balance card |

The two-amount USD balance footer reuses `WowPayFont.wowPay.balanceFootnote` (14 pt), `Colors.WowPay.onBrandSecondary` and `Spacing.small`. Its left currency label and right approximate amount share a baseline; long secondary amounts may wrap. Currency configuration is behavior, not a style token: see `WalletBalanceConfiguration` in [the migration record](WOWPAY_UI_REFACTOR.md#dual-currency-total-assets--2026-09-18).

These APIs are implemented in the referenced source files; build/runtime results remain separate. Do not consolidate semantically distinct roles solely because they currently share a hex value.

## Typography and Geometry

WowPay typography is owned by [WowPayFont](../ios/Packages/Style/Sources/WowPay/WowPayFont.swift) and the existing role files. `WowPayFont` retains native fonts unchanged or resolves a role's base size and weight with `ScaledMetric(relativeTo:)` in the current SwiftUI environment on iOS 17 and later. The point sizes in this inventory are the default Large text-size values. The six existing semantic multisig fonts retain their original native scaling curves; the other 45 roles use their specified semantic scaling style without changing the default size or weight. Callers retain `.font(.wowPay.role)` and weight chaining. `TextStyle` stores the same native/scaled variant and retains its existing native-Font initializer. Default-size and accessibility layout acceptance remain separate runtime checks.

| API / role | HTML reference | Definition and usage |
|---|---|---|
| `WowPayFont.wowPay.brand` | `.brand strong`: 30 px, weight 800 | `WowPay/Font+WowPay.swift`; brand wordmark |
| `WowPayFont.wowPay.balance` | `.balance-number`: 39 px, weight 700 | Same file; portfolio headline, respecting privacy |
| `WowPayFont.wowPay.title` | `.welcome h1` / swap heading: 30 px | Same file; page title |
| `WowPayFont.wowPay.walletPickerTitle` | Current prototype `assets/multisig.css` `.ms-title h1`: 26 px bold | Same file; wallet-picker page title, scaling relative to native title |
| `WowPayFont.wowPay.walletPickerSection` | Current prototype `assets/collective.js` wallet-picker bare `h2`: browser default 24 px bold | Same file; personal, cold and collective wallet group headings, scaling relative to native title2 |
| `WowPayFont.wowPay.sectionTitle` | `.section h2`: 21 px, weight 800 | Same file; section heading |
| `WowPayFont.wowPay.body` | Matches the existing `Font.app.body` default (16 pt medium); HTML body varies by role | Same file; readable body text |
| `WowPayFont.wowPay.label` | Quick action labels: 14 px, weight 700 | Same file; actions and compact row labels |
| `WowPayFont.wowPay.caption` | `.tiny` and shared captions: 12 px; brand subtitle has its own token | Same file; secondary labels |
| `Spacing.wowPay.page` | `.content` horizontal inset: 20 px | `WowPay/Spacing+WowPay.swift`; page inset |
| `Spacing.wowPay.section` | `.section` top margin: 28 px | Same file; section separation |
| `Spacing.wowPay.homeOperations` | Pinned Android `HomeScreen.kt:110–113`: 20 dp only when operations exist after the ordinary quick-action grid | `WowPay/Spacing+WowPay.swift`; `WowPayHomeContent` uses this conditional gap before its optional operations slot. No gap is emitted when the slot is nil; ordinary Home remains unchanged. The [cold Dashboard](../ios/BsvHome/Views/ColdWalletDashboard.swift) consumes the slot on Home; ordinary roots remain unchanged. The [phase 7 local record](../ios/build/phase7/local-gate-acceptance.json) and the phase 8 rendered matrix are separate evidence. |
| `Spacing.wowPay.card` | `.form-card` padding: 18 px | Same file; default card inset |
| `Spacing.wowPay.grid` | `.quick-grid`, `.service-grid`, `.duo`: 10 px | Same file; grid gap |
| `Sizing.wowPay.cardCorner` | `.card`, `.balance`: 22 px | Same file; prominent cards |
| `Sizing.wowPay.rowCorner` | `.asset-row`, `.tx-list`: 18 px | Same file; row cards |
| `Sizing.wowPay.controlCorner` | `.btn`, `.quick-box`: 16 px | Same file; controls |
| `Sizing.wowPay.icon` | Shared 24 px icon role | Same file; standard icon |
| `Sizing.wowPay.actionIcon` | `.quick-box .icon`: 31 px | Same file; quick action icon |
| `Sizing.wowPay.brandMarkWidth/Height` | `.top .brand img.icon`: 82 × 62 px | Same file; header mark |

Reuse existing `Spacing.space*`, `Sizing.image`, `TextStyle` and `Font.app` for matching roles. HTML has closely related 13/14/15 px paddings and 17/18 px corners; additions should be role-based and justified by a current component. Do not generate a global constant for every incidental inline value. Avoid fixed page heights or fixed text containers that clip large text and translations.

## Gradients, Surfaces and States

`Sizing.wowPay.coldWalletQR` in `WowPayColdWallet.swift` owns the 260-point bitmap-generation size. `BsvColdWalletQRPlayer` displays that bitmap as a square at the available card width, matching pinned Android `ColdWalletViews.kt:53–73`; it has no additional 260-point display cap or 12-point outer image border. Pairing/Request/Signed retain their actual payload, error correction and 700 ms playback. Single frames show their count as well. QR encoding remains with the existing generator; optical acceptance is separate. The Cold pages explicitly use the existing solid background. Runtime acceptance is tracked separately in [BSV cold wallet](BSV_COLD_WALLET.md).

The same Cold Style owner defines page item gap 14 / bottom inset 28, service tile vertical padding 14, service icon 22 / avatar 42, value font 15, service subtitle font 10, unsupported heading 24 and description 16. These roles come from the fixed Android Cold page, tile and help owners. Existing page horizontal20, card padding18/corner22, service-avatar corner14, generic 4/8/10/12/16 spacing, label14, caption12 and primary-button48/corner16 are reused. The service subtitle uses native line spacing2 (approximately Android line-height14 at the default font size); translated/Dynamic Type fit and exact optical comparison remain runtime checks, not source-proven pixel acceptance.

| API / role | HTML source | Owner |
|---|---|---|
| `WowPayGradients.balance` | `.balance`: 124° from `#104995` via `#1459bd` to `#087cf1` | `WowPay/WowPayGradients.swift` |
| `WowPayGradients.action` | `.quick-box`: 145° from `#0a75f3` to `#1768de` | Same file; shared action treatment |
| `WowPayGradients.primaryButton` | `.btn`: 120° from `#1680ee` to `#0b68e6`; kept separate from quick-action gradient | `WowPay/WowPayGradients.swift` |
| `WowPayGradients.page` | `.app` radial background: `#c7e2ff` to `#f7fbff` | Same file; page surface |
| Card shadow | `--shadow`: `0 10px 30px rgba(36,107,204,.08)` | `Colors.WowPay.shadow`, `Sizing.wowPay.shadowRadius = 15`, `shadowOffset = 10`; consumed by `WowPayCardModifier`, native blur mapping requires visual review |
| Primary balance shadow | `.balance`: `0 13px 20px rgba(16,99,205,.17)` | `Colors.WowPay.balanceShadow`, `Sizing.wowPay.balanceShadowRadius = 10`, `balanceShadowOffset = 13`; shared by `WowPayBalanceCard` and `ValueHeaderView.wowPay` |
| Button states | `.btn:disabled` opacity .5; active primary/secondary/outline fills | `WowPay/ColorButtonStyle+WowPay.swift`: `.wowPay()`, `.wowPaySecondary()`, `ButtonStylePalette.wowPayPrimary/wowPaySecondary`; `StateButton` with `.wowPayPrimary(state)` retains loading/disabled behavior and uses `WowPayGradients.primaryButton` and `Sizing.wowPay.buttonHeight` (48 pt); both WowPay color-button appearances read enabled state and apply `WowPayOpacity.disabled` |

## Extended HTML Roles

These APIs are read from the current Style sources. The migration checklist records completed typography/layout builds and screenshot checks separately from untested visual states.

| Color API (`Colors.WowPay`) | Light value / HTML role |
|---|---|
| `inputBorder`, `inputPlaceholder` | `#D8E6FA`, `#A4B1CC`, `.input-wrap` border and empty input placeholder |
| `brandSubtitle` | `#697AA2`, `.brand small` |
| `promoTitle` | `#111D61`, `.promo` title and chevron; fixed color in light and dark modes because `wowpay-stablecoin-promo` keeps its light image background |
| `promoSubtitle` | `#6C82A9`, `.promo small`; fixed color in both modes on the same light image background |
| `navigationInactive` | `#7D90B2`, `.bottom-nav button` |
| `backgroundAccent` | `#C7E2FF`, radial background origin |
| `serviceSurface`, `serviceBorder` | `#E8F3FF`, `#E1EAFA`, service icon background and card border |
| `selectionSurface`, `pairBorder` | `#F0F6FF`, `#E1EBF9`, asset picker and quick-pair border |
| `secondaryButton` | `#E6F2FF`, secondary button fill |
| `balanceStart`, `balanceMiddle`, `balanceEnd` | `#104995`, `#1459BD`, `#087CF1`, balance gradient stops |
| `actionStart`, `actionEnd` | `#0A75F3`, `#1768DE`, quick-action gradient |
| `primaryButtonStart`, `primaryButtonEnd` | `#1680EE`, `#0B68E6`, primary button gradient |
| `shadow` | `#246BCC` at .08 opacity (dark base black), shared card shadow |
| `balanceShadow` | `#1063CD` at .17 opacity (dark base black), gradient balance card shadow |

All color APIs above live in `WowPay/Colors+WowPay.swift`; dynamic dark variants remain in the same owner.

| Font API (`WowPayFont.wowPay`) | Native value / consumer |
|---|---|
| `brandSubtitle` | 12.5 pt semibold; brand supporting line |
| `compactBrand`, `compactBrandSubtitle` | 26 pt heavy / 10 pt semibold; compact header |
| `balanceLabel`, `balanceFootnote` | 15 pt regular / 14 pt regular; balance card |
| `sectionAction` | 16 pt regular; section action label |
| `cardTitle`, `promoTitle` | 20 pt bold / 19 pt bold; profile wallet card / promotion |
| `serviceCaption` | 11 pt regular; service and quick-pair captions |
| `amount` | 25 pt bold via `WowPayTypography.amountSize`; compact swap input |
| `sendRecipientInput`, `sendAmountInput`, `sendReceiptAmount` | 15 pt regular recipient / 18 pt semibold amount input / 28 pt bold submitted amount; pinned Android `WowPaySendScreen` and `BsvSendSuccessScene`, consumed by Transfer's send form and receipt |
| `assetHeading`, `assetBalance` | 26 pt bold / 33 pt bold; `.asset-head strong` and `.bsv-balance .balance-number`; identity headers and asset balances, with 33 pt also reused by native transaction confirmation |
| `assetTitle`, `assetValue` | 19 pt bold / 17 pt bold; existing asset row WowPay variant |
| `rowTitle` | 16 pt bold; profile rows and buttons |
| `navigation` | 12 pt bold; bottom tabs |
| `rate`, `subtitle` | 13 pt regular / 14 pt regular; exchange details and supporting copy |

Font role APIs live in `WowPay/Font+WowPay.swift`; `WowPay/WowPayFont.swift` owns their environment-aware renderer. `WowPay/WowPayTypography.swift` additionally owns `brandTracking = -1.1`, `brandSubtitleTracking = 0.3`, `compactBrandSubtitleTracking = 0`, `balanceTracking = -1`, and `amountSize = 25`. SwiftUI and UIKit amount-input measurements resolve the same amount-size token and scaling style; the existing editor height follows that resolved size.

| Geometry API (`Spacing.wowPay` / `Sizing.wowPay`) | Values / role |
|---|---|
| `balancePadding`, `balanceHorizontalPadding`, `balanceTitleGap`, `balanceMinimumHeight` | 22 / 23 / 13 / 146 pt; balance card |
| `compactBrandMarkWidth`, `compactBrandMarkHeight`, `regularBrandMinimumWidth` | 62 / 48 / 341 pt; fitting brand header |
| `brandHeaderHeight`, `compactBrandHeaderHeight` | 74 / 66 pt minimum height; regular/compact brand header |
| `welcomeIllustration`, `welcomeVertical`, `secretInputHeight` | 150 / 42 / 150 pt; welcome geometry and HTML secret-editor minimum height |
| `welcomeBrandMark`, `welcomeButtonMaxWidth` | 72 / 380 pt; Android-aligned welcome mark and maximum action width; the central mark reuses `Sizing.image.extraLarge = 120` |
| `minimumTapTarget`, `buttonHeight`, `actionAspectRatio` | 44 pt hit area / 48 pt WowPay button / 1.14 quick-action tile ratio |
| `walletPickerSupportingLineHeight` | 16.5 pt minimum supporting-text line box for stacked `WowPaySelectionRow`, matching current prototype `.method small` 11 px type at `line-height: 1.5`; scales relative to caption2 and allows wrapped text to grow |
| `swapAmountMaxWidth` | 125 pt; compact amount entry |
| `shadowRadius`, `shadowOffset` | 15 / 10 pt; native shared card shadow |
| `balanceVisibilityIcon` | 18 pt in both states, HTML `.icon.sm`; `wowpay-eye` / `wowpay-eye-off` template resources tinted with `onBrand` |
| `balanceShadowRadius`, `balanceShadowOffset` | 10 / 13 pt; native gradient balance shadow |
| `chevron`, `rowPadding` | 18 / 15 pt; profile card arrow and inset |
| `detailAssetIcon`, `inputCorner`, `receiveQR`, `receiveQRImage` | 58 / 13 / 165 / 105 pt; asset-detail identity icon, `.input-wrap` radius, shared placeholder QR frame and image |
| `sendSuccessIcon` | 60 pt; pinned Android submitted-payment check-circle in `Transfer/BsvSendSuccessScene`; independent from asset artwork sizing |
| `receivePageQR`, `receivePageCompactQR` | 300 / 220 pt; standalone receive QR in regular / compact vertical layout; `ReceiveViewModel.qrSize` supplies the same size to generation and rendering |
| `assetIcon`, `assetRowVertical`, `assetRowHorizontal` | 48 / 15 / 14 pt; asset rows |
| `assetRowGap`, `assetTextGap`, `assetChevron` | 13 / 3 / 17 pt; HTML `.asset-row` icon/text gap, supporting-text margin and trailing arrow |
| `compactCardCorner` | 20 pt; promotion/swap surfaces |
| `serviceCorner`, `servicePadding`, `serviceIcon`, `serviceIconCorner`, `serviceGap` | 17 / 14 / 45 / 14 / 11 pt; service cards |
| `swapHeroWidth`, `swapHeroHeight`, `swapCardPadding` | 112 / 96 / 17 pt; swap landing |
| `selectionCoin`, `pairCoin`, `pairCorner`, `pairArrow`, `pairIconGap` | 33 / 26 / 14 / 15 / 3 pt; asset selector and quick pairs |
| `pairHorizontalPadding`, `pairVerticalPadding` | 7 / 12 pt; HTML `.pair` padding, allowing three quick-pair cards at 320 pt width |
| `navigationIcon` | 25 pt; bottom tab icon |

Geometry lives in `WowPay/Spacing+WowPay.swift`. `Spacing` and `Sizing` remain existing aliases of `CGFloat`, so the same namespace is accessible through either alias without a duplicate declaration. `WowPay/WowPayOpacity.swift` owns `navigationIcon = 0.55` and `disabled = 0.5`, plus `assetChevron = 0.6` for the HTML asset-row arrow. The tap target is native interaction geometry rather than an HTML visual measurement.

The shared `.wowPayCard(padding:cornerRadius:surface:border:)` modifier consumes these tokens and permits a surface override plus optional border; border width reuses `Spacing.space1`. Do not restate the complete card treatment at every call site. Primary color buttons use `ColorButtonStyle.Appearance.wowPayPrimary`, secondary buttons use `.wowPaySecondary`, and unchanged consumers default to `.standard`.

WowPay stateful buttons use 48 pt as a minimum height and grow for multiline labels, with existing 12 pt vertical / 16 pt horizontal padding. Paymail and address form descriptions use `WowPayFont.wowPay.caption` (12 pt), matching `.form-card p`; their input prompts use `Colors.WowPay.inputPlaceholder`. Notification headers reuse the supplied `wowpay-bell` asset at the shared 24 pt icon size. These changes add no parallel font or component family.

## Asset and Transfer Presentation Extensions

`WowPay/TextStyle+WowPay.swift` adds `TextStyle.wowPay.rowTitle` (16 pt bold / heading), `.value` (14 pt / secondary text) and `.caption` (12 pt / secondary text). The instance method `TextStyle.wowPay(font:color:)` replaces the font and maps only existing neutral `Colors.black`, `.gray`, `.secondaryText`, SwiftUI `.primary` and `.secondary` to the supplied WowPay color; other colors, explicit font weight and background are preserved. This keeps transaction warnings, status colors and price changes semantic when `ListItemView`, `ListItemImageView` and `PriceListItemView` opt into WowPay.

`CurrencyInputStyle.wowPayTransfer` reuses the 25 pt bold amount token and navy heading color with leading alignment and available width. `.wowPaySwap` retains trailing alignment and its 125 pt cap; `.standard` retains its existing 44 pt input. The same enum drives SwiftUI and native line-height measurement. `CurrencyInputValidationView` forwards the style without changing validation or currency actions. `FloatFieldStyle.wowPay` reuses the existing floating-label scale and reads `inputPlaceholder` / `secondaryText`; recipient composition applies the `inputBorder` and `inputCorner` tokens.

`ValueHeaderView.wowPay` uses existing balance padding, gradient, radius and shadow with the asset-specific 33 pt amount; `HeaderButtonsView.wowPay` reuses the 10 pt grid gap and 48 pt primary/secondary buttons. `ValueHeaderView.wowPayConfirmation` reuses the white card surface, 58 pt asset image, 33 pt amount and 14 pt supporting text. Native confirmation preserves the full amount by allowing wrapping. `AssetBalanceView.wowPay` reuses 48 pt row image, 19 pt asset title and 14 pt subtitle. No new balances, prices or transaction states are introduced by these style roles.

These asset/transfer additions are implemented and compile in the final app. Targeted tests and selected simulator checks passed; confirmation and other unexercised visual states are tracked in [the migration checklist](WOWPAY_UI_REFACTOR.md).

## Consumer Rules

1. Read this mapping, inspect the actual token definition, and use the existing semantic role.
2. Extract missing repeated styles into `Style` before wiring pages. Add only the roles the implemented page/preview needs.
3. Compose SwiftUI views with shared `.wowPayCard(...)` and `.wowPayBackground()`; do not restate their padding, corner, fill and shadow in every page.
4. Localized strings remain in the existing localization pipeline, not style files. Asset artwork remains in resource catalogs, not encoded as colors or layout constants.
5. Appearance and language entries follow the prototype where present; otherwise retain existing entries and behavior. Light/simplified-Chinese is the current acceptance focus, not permission to remove other locales or force-hide existing preferences.

## Wallet and Onboarding Batch

`WowPayFont.wowPay.onboardingTitle` is 25 pt bold, matching the HTML `.center-screen h1` role; it is used by the welcome import heading and import form heading. The import-method heading uses `WowPayFont.wowPay.title`. Method cards override the existing 16 pt `rowTitle` to semibold, matching Android's `titleMedium` weight. Welcome reuses 16 pt between mark and brand, 8 pt between brand and its regular-weight tagline, and a 72 pt mark with a 380 pt maximum action width. Root presentation uses `Spacing.wowPay.welcomeHeaderTop = 48` below the native safe area and a plain skip link; pushed presentation retains the native bar. The original Android chain-link PNG replaces the older prototype W in `wowpay-mark`; the filled wallet path from Android `AppIcons.Wallet → ic_wallet.xml` replaces the older outline in the existing template PDF asset.

`wowPayBackground(_ style: WowPayBackgroundStyle = .gradient)` retains the existing gradient for default callers. Only welcome, terms and create/import method pages select `.solid`, which uses the existing `Colors.WowPay.background` and preserves foreground/tint behavior. This does not change input, secret, verification or other page backgrounds. The secret editor uses the shared 150 pt minimum height without imposing a fixed page height. Wallet names reuse `wowPayInput()` with 14 pt input type, existing floating-label behavior, 12/10 pt horizontal/vertical padding and 13 pt outline corner.

`wowPayListCard(isSelected:)` reuses 18 pt row corners, existing row insets, 1 pt border, white surface and selected brand/pale colors. Secret grid cells reuse 14 pt type (the existing explicit semibold word/index weight), pale surface, 13 pt corners and brand current-word outline. Verification choices retain 44 pt minimum tap height and disabled opacity. All values come from existing Style tokens; page compositions do not define another palette.

## Transaction History Batch

HTML `.tx strong`, `.tx small` and `.tx-value` map to existing `WowPayFont.wowPay.label` (14 pt bold), `serviceCaption` (11 pt), and new `transactionValue` (13 pt bold). `Sizing.wowPay.transactionIcon = 38` and `Spacing.wowPay.transactionRowPadding = 13` match `.tx-badge` and `.tx` padding. The existing transaction model still supplies participant labels, amount signs, fiat equivalents, status badges and status colors; neutral colors map to WowPay heading/secondary text. Badge backgrounds and explicit semantic weights/colors remain intact. Date sections retain real grouping, and list separators use the shared border color.

Filter/detail/Paymail/address pages reuse existing page, card, field and button tokens. No extra feature palette or independent date, amount, selection or status renderer is introduced.

The 2026-11-17 user request removes the Home News module. Its dedicated `newsImageWidth`, `newsImageHeight`, `newsVertical`, `newsSubtitleGap` and `newsCorner` tokens are removed with their only consumer; shared caption, color and spacing tokens remain in use elsewhere. The earlier editorial/date layout is historical and is no longer rendered.

## Bottom Selection Sheet

The shared selection sheet maps `.modal-layer`, `.modal` and `.method` to existing `Style` namespaces. `Colors.WowPay.modalBackdrop` uses HTML `#0A1C4C` at 0.5 opacity (black dark variant); `selectionBorder` uses `#DFEAFA`. `Sizing.wowPay.modalCorner = 25`, `Spacing.wowPay.modalHorizontal = 22`, `modalTop = 24`, `modalBottom = 26`, and `selectionRowCorner = 15` come from the HTML panel/row geometry. The panel adds native bottom safe-area space; measured header/row heights bound scrolling rather than fixing a phone-specific panel height.

Rows reuse the 48 pt asset icon, 14 pt service padding and 12 pt gap. `WowPayFont.wowPay.selectionTitle` is 15 pt bold; descriptions use the existing body font and secondary text with `WowPayOpacity.selectionSubtitle = 0.5`. The header reuses section-title type. Close artwork reuses the prototype `wowpay-more` resource at 18 pt with a 44 pt minimum tap area. `WowPayMotion.modalDuration = 0.25` is the native panel transition duration (the HTML has no specified transition); opening uses ease-out and closing uses ease-in-out. Reduce Motion avoids sliding. Existing background, surface and brand tokens supply all remaining colors. No page-local palette is introduced.

## Receive Layout Alignment — 2026-09-22

`ReceiveScene` follows the pinned Android standalone receive composition: asset artwork/name, a 300 pt QR or 220 pt in compact vertical layout, address and network/Memo warning, native toolbar Share and a bottom Copy Address action. Generation and display both use `ReceiveViewModel.qrSize`; the existing Logo, actual selected address, solid white background and nearest-neighbor display remain. The earlier prototype Paymail card and supporting copy are omitted. Existing `receiveQR` / `receiveQRImage` tokens remain available to the shared placeholder QR card; the personal BSV asset overview does not generate or display a QR. Native safe areas, scrolling, return navigation, network selection and disabled/error/retry states remain.

## Asset Detail Prototype Alignment — 2026-09-29

The BSV/MNEE detail architecture follows the approved BSV screenshot and local HTML `.bsv`, `.tabs`, `.duo`, `.paymail-card`, `.qr-card` and `.tx` styles. USDT uses the token variant below; actual network/account/contract metadata overrides the HTML's BSV-token demonstration values.

`Spacing.wowPay` includes `assetOverviewBalanceHeight = 140`, `detailTabGap = 18`, `detailTabUnderline = 3`, `paymailCardHeight = 170`, `paymailAvatar = 34`, `paymailIcon = 22`, `paymailHelpIcon = 15`, `paymailActionCorner = 11`, `paymailActionHeight = 36`, and `detailQR = 80` pt. Scrollable asset tabs use the existing 14 pt bold label and 10 pt bottom inset; equal-width collective tabs retain their own font. The BSV root omits the prototype's extra asset/market header, placing the wallet header directly above the tabs. The retained Paymail action-button variant uses the old 170 pt card; the Android-aligned whole-card variant wraps its content, uses a 42 pt avatar / 26 pt icon, existing 14 pt service-icon corners, 16 pt insets, 12 pt gaps and action corners, 11 pt action vertical inset, and the existing 22 pt card corner. Its semantic fonts are `paymailOverviewTitle` (16 pt bold), `paymailOverviewAddress` (14 pt regular), existing 12 pt caption and 14 pt bold label. No separate help control or empty handle/status row is added in the whole-card variant. The overview/form components otherwise reuse existing page, card, grid, shadow, input, 44 pt tap target and action tokens. `WowPayFont.wowPay` retains `assetBalanceSymbol` (19 pt bold), `assetPrice` (22 pt bold), `paymailAction` (12 pt heavy), and `moreDots` (23 pt). `WowPayTypography.moreDotsTracking = 2` matches the HTML dot-menu spacing. `Colors.WowPay.receiveBadge` maps `.tx-badge.green` to `#E4FAF2` with existing dynamic appearance support.

Balances use full native quantity precision with at least four fractional digits rather than prototype sample precision, except USDT balances, which show exactly four fractional digits by truncating extra digits. The Home/Assets total-assets card also truncates both currency amounts to exactly four fractional digits. The balance footer stays present with real fiat value or `—`. Market movement displays the change from the previous price implied by the existing 24-hour percentage; it is not copied from demo values. `CurrencyFormatter.signedNumber` uses localized formatting with the same four-digit minimum and retains additional precision so small stablecoin movements remain visible. Paymail and help artwork use the brand blue template tint. The Paymail action has an expanded 44 pt hit region around its 36 pt visible background. The standalone Receive page generates its QR from the actual selected address and displays it on a solid white square with nearest-neighbor interpolation; the personal BSV asset overview has no QR. Asset detail hides the native title toolbar in favor of the branded in-content header, and bottom send/receive actions scroll with the content as in the HTML.

## USDT Token Detail Variant — 2026-09-29

The USDT layout follows the local HTML `tokenDetail`, `.token-balance`, `.token-actions`, `.market-grid`, `.token-info` and `.token-note` styles. `Sizing.wowPay` adds `tokenBalanceHeight = 138`, `tokenActionIcon = 28`, `tokenMetricTitleGap = 7`, `tokenNetworkGap = 5`, and `tokenIdentifierWidth = 190`. `WowPayFont.wowPay.tokenBalance` is 34 pt bold and `tokenBalanceSymbol` is 18 pt bold. `Colors.WowPay.tokenNoteSurface` uses `#FFF8E9` and `tokenNoteText` uses `#8A6724`, with dynamic dark variants. Existing page20, section28, grid10, action padding14/8, metric padding15/corner17, info corner18/padding4/16, and network-pill serviceSurface `#E8F3FF` are reused. Body safe areas and scrolling remain native; long real network/amount text can wrap or scale. The explicit token styles leave BSV/MNEE geometry unchanged.

The two metric cards stretch to equal height, including when their quantity footnote wraps. Their footnotes reuse the 13 pt rate font in bold, matching the HTML's default small-text scale. The token introduction card fills the content width. USDT tab changes reset the scroll to the top as the prototype's `render()` does.

## BSV Multisig

`WowPay/WowPayMultisig.swift` adds the roles shared across the multisig routes in the current prototype’s `assets/multisig.css`. The Feature owns page composition, while all reused visual values remain in Style. Native subpages retain their system navigation bar.

| API | Prototype reference / consumers |
| --- | --- |
| `Colors.WowPay.multisigPositiveSurface/PositiveText` | `.ms-chip.good`: `#E2F8EF` / `#07895A`; confirmed members and enabled wallets |
| `multisigWarningSurface/WarningText` | `.ms-chip.wait`: `#FFF3DB` / `#A67417`; pending member and proposal status |
| `multisigNoticeSurface/NoticeText` | `.ms-notice`: `#EAF4FF` / `#5C7094`; collaboration explanations |
| `multisigRiskSurface/RiskText` | `.ms-notice.warning`: `#FFF5E6` / `#936718`; signing, funding recovery and configuration constraints |
| `multisigBadge` | `.ms-badge`: `#EC3947`; current-identity personal pending counts, hidden at zero and capped at 99+ |
| `multisigInputSurface` | `.ms-input`: `#F9FCFF`; existing `wowPayInput(surface:)` variant |
| `multisigErrorSurface/ErrorText` | `.ms-error`: `#FFF1EF` / `#B74235`; field, invitation and transaction validation errors |
| `multisigCoin` | `.ms-symbol.bsv`: `#F0B51A`; BSV multisig asset identity |
| `WowPayFont.wowPay.multisigLabel`, `multisigValue`, `multisigTab`, `multisigChip`, `multisigBannerTitle`, `multisigTotal` | Semantic SwiftUI footnote bold / footnote semibold / footnote bold / caption2 bold / title2 bold / largeTitle heavy; scales with Dynamic Type. Review text and buttons use semantic roles, and member identity rows stack at accessibility sizes. |
| `WowPayFont.wowPay.multisigTransactionAmount` | 30 pt bold; the pinned Android proposal, independent review, funding and activity-detail amount role. The retained balance role is unchanged. |
| `Sizing.wowPay.multisigPaymentQR` | 260 pt; the collective amount payment-code QR. Generation and rendering share this value; the older asset `receiveQR` and personal 300/220 pt receive-page roles retain their separate consumers. |
| `Spacing.wowPay.multisigMember/Field` | 14 / 15 pt; member rows and creation/transfer fields |
| `Sizing.wowPay.multisigTabControlHeight/Avatar/Person/Progress/Badge/Coin` | 40 / 36 / 31 / 6 / 20 / 39 pt; wallet/pending selectors, member identity, wallet member chips, signature progress, badges and asset icon. Text/buttons use minimum heights so larger fonts can expand. |

The shared `WowPayBottomPanel` retains the existing selection-sheet motion, backdrop and measured scrolling. Runtime visual comparison and transaction verification are separate from these source mappings.

## Portal Multisig Review and Funding — 2026-10-14

WowPay 5 creation and transaction review pages use the existing native navigation, card, input and button roles rather than the former multisig confirmation panels. Funding review and results reuse the shared amount, label, notice and value typography. Creation/join actions use equal button typography and minimum height. Single-digit pending badges are square circles with centered text; multi-digit counts use a minimum-width capsule. Existing light/dark tokens and accessibility sizing remain authoritative.

Native system navigation is required for non-tab, non-modal pages, including all multisig detail and review routes. Use the system title and back behavior; prototype headers do not authorize a custom navigation bar, duplicate page title, or custom back control.
