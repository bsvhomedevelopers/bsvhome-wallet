// Copyright (c). Gem Wallet. All rights reserved.

import Components
import Foundation
import Localization
import Primitives
import Style
import SwiftUI

struct SecurityReminderScene: View {
    @State private var model: SecurityReminderViewModel

    init(model: SecurityReminderViewModel) {
        self.model = model
    }

    var body: some View {
        ScrollView {
            VStack(spacing: Spacing.wowPay.section) {
                VStack(spacing: Spacing.wowPay.card) {
                    Image("wowpay-shield-check")
                        .resizable()
                        .scaledToFit()
                        .frame(width: Sizing.wowPay.brandMarkHeight, height: Sizing.wowPay.brandMarkHeight)
                        .foregroundStyle(Colors.WowPay.brand)
                        .accessibilityHidden(true)
                    Text(Localized.wowpaySecurityTitle)
                        .font(.wowPay.title)
                        .foregroundStyle(Colors.WowPay.heading)
                    Text(model.message)
                        .font(.wowPay.subtitle)
                        .foregroundStyle(Colors.WowPay.secondaryText)
                }
                .multilineTextAlignment(.center)
                .frame(maxWidth: .infinity)
                .padding(.vertical, Spacing.wowPay.page)

                VStack(spacing: Spacing.space12) {
                    ForEach(model.items) { item in
                        ListItemView(
                            title: TextValue(text: item.title, style: .wowPay.rowTitle),
                            titleExtra: TextValue(text: item.subtitle, style: .wowPay.value),
                            imageStyle: item.image,
                        )
                        .wowPayCard(cornerRadius: Sizing.wowPay.controlCorner)
                    }
                }
            }
            .padding(Spacing.wowPay.page)
        }
        .wowPayBackground()
        .safeAreaButton {
            StateButton(
                text: Localized.Common.continue,
                type: .wowPayPrimary(),
                action: model.onNext,
            )
            .accessibilityIdentifier("continueWalletSecurityReminder")
        }
        .navigationTitle(model.title)
        .toolbarTitleDisplayMode(.inline)
        .toolbarInfoButton(url: model.docsUrl)
    }
}

#Preview {
    SecurityReminderScene(
        model: SecurityReminderViewModel(
            title: Localized.Wallet.New.title,
            onNext: {},
        ),
    )
}
