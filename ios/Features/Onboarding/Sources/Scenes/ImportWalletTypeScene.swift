// Copyright (c). Gem Wallet. All rights reserved.

import Components
import BsvHomeCorePrimitives
import Localization
import Primitives
import PrimitivesComponents
import Style
import SwiftUI

struct ImportWalletTypeScene: View {
    let model: ImportWalletTypeViewModel
    let onColdWallet: (() -> Void)?
    @State private var searchQuery = ""
    @State private var toastMessage: ToastMessage?

    init(
        model: ImportWalletTypeViewModel,
        onColdWallet: (() -> Void)? = nil,
    ) {
        self.model = model
        self.onColdWallet = onColdWallet
    }

    var body: some View {
        List {
            Section {
                VStack(alignment: .leading, spacing: .small) {
                    Text(Localized.wowpayImportMethods)
                        .font(.wowPay.onboardingTitle)
                    Text(Localized.wowpayImportSubtitle)
                        .font(.wowPay.subtitle)
                        .foregroundStyle(Colors.WowPay.secondaryText)
                }
                .cleanListRow()
            }

            Section {
                NavigationLink(value: ImportWalletType.multicoin) {
                    ListItemView(
                        title: Localized.Wallet.multicoin,
                        titleStyle: .wowPay.rowTitle,
                        imageStyle: .asset(assetImage: AssetImage.image(Images.Logo.logo)),
                    )
                }
                .wowPayListCard()
            }

            if let onColdWallet {
                Section {
                    Button(action: onColdWallet) {
                        ListItemView(
                            title: TextValue(text: Localized.coldWalletTitle, style: .wowPay.rowTitle),
                            titleExtra: TextValue(text: Localized.coldWalletOnlineHint, style: .wowPay.caption),
                            imageStyle: .asset(assetImage: AssetImage.image(Image("wowpay-watch-wallet"))),
                        )
                        .fixedSize(horizontal: false, vertical: true)
                        .frame(maxWidth: .infinity, alignment: .leading)
                        .contentShape(Rectangle())
                    }
                    .buttonStyle(.plain)
                    .wowPayListCard()
                    .accessibilityIdentifier("importMethodColdWallet")
                }
            }

            Section {
                Button(action: onSelectKeystore) {
                    ListItemView(
                        title: Localized.wowpayKeystore,
                        titleStyle: .wowPay.rowTitle,
                        subtitle: Localized.wowpayUnavailable,
                        subtitleStyle: .wowPay.caption,
                        imageStyle: .asset(assetImage: AssetImage.image(Image("wowpay-keystore"))),
                    )
                }
                .buttonStyle(.plain)
                .wowPayListCard()
            }

            if model.items(for: searchQuery).isEmpty {
                StateEmptyView(title: Localized.Common.noResultsFound)
            } else {
                Section {
                    ForEach(model.items(for: searchQuery)) { chain in
                        NavigationLink(value: ImportWalletType.chain(chain)) {
                            ListItemView(
                                title: chain.networkName,
                                titleStyle: .wowPay.rowTitle,
                                imageStyle: .asset(assetImage: AssetImage.image(ChainImage(chain: chain).image)),
                            )
                        }
                        .wowPayListCard()
                        .accessibilityIdentifier("importWalletChain-\(chain.rawValue)")
                    }
                }
            }
        }
        .listStyle(.plain)
        .listSectionSpacing(.custom(Spacing.wowPay.grid))
        .contentMargins(.horizontal, Spacing.wowPay.page, for: .scrollContent)
        .scrollContentBackground(.hidden)
        .wowPayBackground()
        .contentMargins(.top, Spacing.wowPay.page, for: .scrollContent)
        .navigationBarTitle(model.title)
        .navigationBarTitleDisplayMode(.inline)
        .toolbarInfoButton(url: AppUrl.docs(.migrateWallet))
        .searchable(
            text: $searchQuery,
            placement: .navigationBarDrawer(displayMode: .always),
        )
        .autocorrectionDisabled(true)
        .scrollDismissesKeyboard(.interactively)
        .toast(message: $toastMessage)
    }

    private func onSelectKeystore() {
        // TODO(WOWPAY-KEYSTORE): Connect encrypted Keystore import through the existing wallet service.
        toastMessage = ToastMessage(title: Localized.wowpayUnavailable, image: "info.circle")
    }
}
