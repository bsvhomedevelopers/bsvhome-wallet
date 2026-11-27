// Copyright (c). Gem Wallet. All rights reserved.

import Components
import Foundation
import Localization
import Primitives
import Style
import SwiftUI

struct AcceptTermsScene: View {
    @State private var model: AcceptTermsViewModel

    init(model: AcceptTermsViewModel) {
        self.model = model
    }

    var body: some View {
        ScrollView {
            VStack(spacing: Spacing.wowPay.section) {
                Text(model.message)
                    .font(.wowPay.subtitle)
                    .foregroundStyle(Colors.WowPay.secondaryText)
                    .multilineTextAlignment(.center)
                    .frame(maxWidth: .infinity)
                    .padding(.vertical, Spacing.wowPay.page)

                VStack(spacing: Spacing.space12) {
                    ForEach($model.items) { $item in
                        Toggle(isOn: $item.isConfirmed) {
                            Text(item.message)
                                .font(.wowPay.body)
                                .foregroundStyle(Colors.WowPay.heading)
                        }
                        .accessibilityIdentifier(item.id)
                        .toggleStyle(CheckboxStyle(position: .left))
                        .tint(Colors.WowPay.brand)
                        .frame(maxWidth: .infinity, alignment: .leading)
                        .wowPayCard(cornerRadius: Sizing.wowPay.controlCorner)
                    }
                }
            }
            .padding(Spacing.wowPay.page)
        }
        .wowPayBackground(.solid)
        .safeAreaButton {
            StateButton(
                text: Localized.Onboarding.AcceptTerms.continue,
                type: .wowPayPrimary(model.isConfirmed ? .normal : .disabled),
                action: { model.onNext?() },
            )
            .accessibilityIdentifier("acceptWalletTerms")
        }
        .navigationTitle(model.title)
        .toolbarTitleDisplayMode(.inline)
        .toolbarInfoButton(url: model.termsAndServicesURL)
    }
}
