import Components
import Formatters
import Localization
import Style
import SwiftUI

public struct WowPayUnavailableScene: View {
    private enum Field: Hashable {
        case recipient
        case amount
        case note
    }

    @FocusState private var focusedField: Field?
    private let page: WowPayUnavailablePage
    @State private var toast: ToastMessage?
    @State private var selectedTab = 0
    @State private var recipient = ""
    @State private var amount = ""
    @State private var note = ""
    @State private var isHidden = false

    public init(page: WowPayUnavailablePage) {
        self.page = page
    }

    public var body: some View {
        if page == .mnee {
            WowPayPlaceholderAssetScene()
        } else {
            unavailablePage
        }
    }

    private var unavailablePage: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: Spacing.wowPay.section) {
                if page != .paymail {
                    Text(Localized.wowpayUnavailableDescription)
                        .font(.wowPay.caption).foregroundStyle(Colors.WowPay.secondaryText)
                        .frame(maxWidth: .infinity, alignment: .leading)
                        .wowPayCard()
                }
                content
            }
            .padding(Spacing.wowPay.page)
        }
        .wowPayBackground()
        .scrollDismissesKeyboard(.interactively)
        .navigationTitle(page.title)
        .navigationBarTitleDisplayMode(.inline)
        .toolbar(.visible, for: .navigationBar)
        .toolbar(page == .paymail ? .hidden : .automatic, for: .tabBar)
        .toolbar {
            if page != .paymail {
                ToolbarItemGroup(placement: .keyboard) {
                    Spacer()
                    Button(Localized.Common.done) { focusedField = nil }
                }
            }
            if page == .bsv || page == .send {
                ToolbarItem(placement: .primaryAction) {
                    Menu {
                        NavigationLink(Localized.wowpayReceiveBsv) { WowPayUnavailableScene(page: .receive) }
                        NavigationLink(Localized.Activity.title) { WowPayUnavailableScene(page: .transactions) }
                    } label: {
                        Image(systemName: "ellipsis")
                    }
                    .accessibilityLabel(Localized.wowpayMore)
                }
            } else if page != .paymail {
                ToolbarItem(placement: .topBarTrailing) {
                    Button(action: unavailable) {
                        Image("wowpay-bell")
                            .resizable()
                            .scaledToFit()
                            .frame(width: Sizing.wowPay.icon, height: Sizing.wowPay.icon)
                    }
                    .accessibilityLabel(Localized.Settings.Notifications.title)
                }
            }
        }
        .onSubmit { focusedField = nil }
        .toast(message: $toast)
    }

    @ViewBuilder
    private var content: some View {
        switch page {
        case .bsv:
            assetContent
        case .mnee:
            WowPayPlaceholderAssetScene()
        case .send:
            sendContent
        case .receive:
            VStack(spacing: Spacing.medium) {
                Image(systemName: "qrcode")
                    .font(.wowPay.balance)
                    .foregroundStyle(Colors.WowPay.secondaryText)
                    .frame(width: Sizing.wowPay.receiveQR, height: Sizing.wowPay.receiveQR)
                    .background(Colors.WowPay.addressSurface, in: RoundedRectangle(cornerRadius: Sizing.wowPay.rowCorner))
                    .accessibilityHidden(true)
                Text(Localized.wowpayUnavailable)
                HStack {
                    Button(Localized.Common.copy, action: unavailable).buttonStyle(.wowPaySecondary())
                    Button(Localized.Common.share, action: unavailable).buttonStyle(.wowPay())
                }
            }.wowPayCard()
            destination(.paymail, image: "paymail")
        case .paymail:
            paymailContent
        case .address:
            addressContent
        case .transactions:
            transactionsContent
        }
    }

    private var paymailContent: some View {
        VStack(alignment: .leading, spacing: Spacing.wowPay.section) {
            WowPayPaymailForm(
                title: Localized.wowpayPaymailSetup,
                description: Localized.wowpayPaymailDescription,
                fieldTitle: Localized.Wallet.name,
                placeholder: "yourname@wowpay.com",
                saveTitle: Localized.wowpayPaymailSave,
                doneTitle: Localized.Common.done,
                name: $recipient,
                onSave: unavailable,
            )
            Text(Localized.wowpayUnavailable)
                .font(.wowPay.caption)
                .foregroundStyle(Colors.WowPay.secondaryText)
        }
    }

    private var addressContent: some View {
        VStack(alignment: .leading, spacing: Spacing.wowPay.section) {
            VStack(alignment: .leading, spacing: Spacing.wowPay.card) {
                VStack(alignment: .leading, spacing: Spacing.small) {
                    Text(Localized.wowpayBsvAddress)
                        .font(.wowPay.cardTitle)
                        .foregroundStyle(Colors.WowPay.heading)
                    Text(Localized.wowpayAddressDescription)
                        .font(.wowPay.caption)
                        .foregroundStyle(Colors.WowPay.secondaryText)
                }
                Text(Localized.wowpayUnavailable)
                    .font(.wowPay.caption)
                    .foregroundStyle(Colors.WowPay.secondaryText)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .wowPayCard(padding: Spacing.space12, cornerRadius: Sizing.wowPay.inputCorner, surface: Colors.WowPay.addressSurface)
                Button(action: unavailable) {
                    Label(Localized.Wallet.copyAddress, systemImage: "doc.on.doc")
                }
                .buttonStyle(.wowPaySecondary())
            }
            .wowPayCard()

            VStack(alignment: .leading, spacing: Spacing.wowPay.grid) {
                ViewThatFits(in: .horizontal) {
                    HStack {
                        addressRecordsTitle
                        Spacer()
                        addAddressButton
                    }
                    VStack(alignment: .leading, spacing: Spacing.wowPay.grid) {
                        addressRecordsTitle
                        addAddressButton
                    }
                }
                HStack(spacing: Spacing.wowPay.grid) {
                    Image("wowpay-address-book").resizable().scaledToFit()
                        .frame(width: Sizing.wowPay.icon, height: Sizing.wowPay.icon)
                        .accessibilityHidden(true)
                    Text(Localized.wowpayUnavailable)
                        .font(.wowPay.subtitle)
                        .foregroundStyle(Colors.WowPay.secondaryText)
                }
                .frame(maxWidth: .infinity, alignment: .leading)
                .wowPayCard(padding: Spacing.wowPay.rowPadding, cornerRadius: Sizing.wowPay.controlCorner)
            }
        }
    }

    private var addressRecordsTitle: some View {
        Text(Localized.wowpayAddressRecords)
            .font(.wowPay.sectionTitle)
            .foregroundStyle(Colors.WowPay.heading)
    }

    private var addAddressButton: some View {
        Button(Localized.wowpayAddressAdd, action: unavailable)
            .font(.wowPay.sectionAction)
            .foregroundStyle(Colors.WowPay.brand)
            .frame(minHeight: Sizing.wowPay.minimumTapTarget)
    }

    private var assetContent: some View {
        VStack(spacing: Spacing.wowPay.section) {
            HStack {
                Image(page == .bsv ? "wowpay-bsv" : "wowpay-mnee").resizable().scaledToFit()
                    .frame(width: Sizing.wowPay.detailAssetIcon, height: Sizing.wowPay.detailAssetIcon)
                Text(page.title).font(.wowPay.assetHeading)
                Spacer()
                Text("—").font(.wowPay.title)
            }
            if page == .bsv {
                Picker("", selection: $selectedTab) {
                    Text(Localized.wowpayOverview).tag(0)
                    Text(Localized.Common.address).tag(1)
                    Text(Localized.Activity.title).tag(2)
                    Text(Localized.wowpayAbout).tag(3)
                }.pickerStyle(.segmented)
            }
            if selectedTab == 0 {
                WowPayBalanceCard(title: Localized.wowpayTotalAssets, value: "—", subtitle: Localized.wowpayUnavailable, isHidden: $isHidden)
                if page == .bsv {
                    HStack(alignment: .top, spacing: Spacing.wowPay.grid) {
                        destination(.paymail, image: "paymail")
                        destination(.receive, image: "qr-code")
                    }
                    VStack(spacing: Spacing.wowPay.grid) {
                        WowPaySectionHeader(title: Localized.Activity.title, actionTitle: Localized.wowpayMore) {
                            selectedTab = 2
                        }
                        transactionsContent
                    }
                    HStack {
                        NavigationLink(Localized.Wallet.send) { WowPayUnavailableScene(page: .send) }.buttonStyle(.wowPay())
                        NavigationLink(Localized.Wallet.receive) { WowPayUnavailableScene(page: .receive) }.buttonStyle(.wowPaySecondary())
                    }
                }
            } else if selectedTab == 1 {
                addressContent
            } else if selectedTab == 2 {
                Text(Localized.Activity.title).font(.wowPay.sectionTitle)
                    .frame(maxWidth: .infinity, alignment: .leading)
                transactionsContent
            } else {
                VStack(alignment: .leading, spacing: Spacing.wowPay.grid) {
                    Text(Localized.wowpayAbout).font(.wowPay.cardTitle)
                    Text(page == .bsv ? "Bitcoin SV" : "MNEE").font(.wowPay.subtitle)
                    Text(Localized.wowpayUnavailableDescription)
                        .font(.wowPay.caption)
                        .foregroundStyle(Colors.WowPay.secondaryText)
                }
                .frame(maxWidth: .infinity, alignment: .leading)
                .wowPayCard()
            }
        }
    }

    private var transactionsContent: some View {
        StateEmptyView(title: Localized.wowpayUnavailable)
            .wowPayCard()
    }

    private var sendContent: some View {
        VStack(alignment: .leading, spacing: Spacing.wowPay.section) {
            VStack(alignment: .leading, spacing: Spacing.medium) {
                Text(Localized.wowpayRecipient).font(.wowPay.label)
                TextField(Localized.wowpayRecipient, text: $recipient, prompt: Text(Localized.wowpayRecipient).foregroundStyle(Colors.WowPay.inputPlaceholder))
                    .textInputAutocapitalization(.never).autocorrectionDisabled()
                    .focused($focusedField, equals: .recipient)
                    .submitLabel(.done)
                    .wowPayInput()
                HStack(spacing: Spacing.wowPay.grid) {
                    Button(Localized.Wallet.scanQrCode, action: unavailable)
                    Spacer()
                    Button(Localized.Contacts.title, action: unavailable)
                }
                .font(.wowPay.label)
                .foregroundStyle(Colors.WowPay.brand)
                .frame(minHeight: Sizing.wowPay.minimumTapTarget)
                HStack {
                    Text(Localized.wowpayAmount).font(.wowPay.label)
                    Spacer()
                    Button(Localized.Common.all, action: unavailable)
                        .font(.wowPay.label)
                        .foregroundStyle(Colors.WowPay.brand)
                        .frame(minHeight: Sizing.wowPay.minimumTapTarget)
                }
                CurrencyInputView(text: $amount, currencySymbol: "BSV", currencyPosition: .trailing, secondaryText: Localized.wowpayUnavailable, keyboardType: .decimalPad, style: .wowPayTransfer)
                    .focused($focusedField, equals: .amount)
                    .wowPayInput()
                HStack {
                    ForEach([25, 50, 75, 100], id: \.self) { percent in
                        Button(PercentFormatter.unsigned.string(Double(percent)), action: unavailable).buttonStyle(.wowPaySecondary())
                    }
                }
                TextField(Localized.wowpayNote, text: $note, prompt: Text(Localized.wowpayNote).foregroundStyle(Colors.WowPay.inputPlaceholder))
                    .focused($focusedField, equals: .note)
                    .submitLabel(.done)
                    .wowPayInput()
            }.wowPayCard()
            HStack {
                Text(Localized.wowpayNetworkFee).font(.wowPay.label)
                Spacer()
                Text("—").font(.wowPay.rate)
            }.wowPayCard(surface: Colors.WowPay.pale)
            Button(Localized.Transfer.Send.title, action: unavailable).buttonStyle(.wowPay())
        }
    }

    private func destination(_ page: WowPayUnavailablePage, image: String) -> some View {
        NavigationLink {
            WowPayUnavailableScene(page: page)
        } label: {
            VStack(alignment: .leading, spacing: Spacing.small) {
                Image("wowpay-\(image)").resizable().scaledToFit()
                    .frame(width: Sizing.wowPay.icon, height: Sizing.wowPay.icon)
                Text(page.title).font(.wowPay.label)
                Text(Localized.wowpayUnavailable).font(.wowPay.caption)
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            .wowPayCard()
        }
        .buttonStyle(.plain)
        .foregroundStyle(Colors.WowPay.brand)
    }

    private func unavailable() {
        focusedField = nil
        // TODO(WOWPAY-BSV): Wire real chain support before enabling transactions, fees or receive payloads.
        // TODO(WOWPAY-MNEE): Connect the token through the shared asset service.
        // TODO(WOWPAY-ADDRESS): Connect real BSV address generation and records before enabling copy or add actions.
        toast = ToastMessage(title: Localized.wowpayUnavailable, image: "info.circle")
    }
}
