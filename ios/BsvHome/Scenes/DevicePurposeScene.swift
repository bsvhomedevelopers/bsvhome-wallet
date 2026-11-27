import Components
import Localization
import Primitives
import Style
import SwiftUI

struct DevicePurposeScene: View {
    let confirm: (DevicePurpose) throws -> Void

    @State private var isPreparingColdWallet = false
    @State private var showsStorageError = false

    var body: some View {
        NavigationStack {
            ScrollView {
                VStack(alignment: .leading, spacing: Spacing.wowPay.section) {
                    Text(Localized.devicePurposeTitle)
                        .font(.title.bold())
                        .foregroundStyle(Colors.WowPay.heading)
                        .accessibilityAddTraits(.isHeader)

                    VStack(spacing: Spacing.wowPay.card) {
                        Button(Localized.devicePurposeDaily) { select(.daily) }
                            .buttonStyle(.wowPay())
                            .accessibilityIdentifier("selectDailyDevicePurpose")

                        Button(Localized.devicePurposeCold) { isPreparingColdWallet = true }
                            .buttonStyle(.wowPaySecondary())
                            .accessibilityIdentifier("selectColdDevicePurpose")
                    }
                }
                .frame(maxWidth: .infinity, alignment: .leading)
                .padding(Spacing.wowPay.page)
            }
            .wowPayBackground()
            .navigationTitle(Localized.wowpayBrand)
            .navigationBarTitleDisplayMode(.inline)
            .navigationDestination(isPresented: $isPreparingColdWallet) {
                ScrollView {
                    VStack(alignment: .leading, spacing: Spacing.wowPay.section) {
                        Text(Localized.devicePurposeColdPreparation)
                            .font(.body)
                            .foregroundStyle(Colors.WowPay.heading)

                        Button(Localized.devicePurposeConfirmCold) { select(.coldWallet) }
                            .buttonStyle(.wowPay())
                            .accessibilityIdentifier("confirmColdDevicePurpose")
                    }
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .padding(Spacing.wowPay.page)
                }
                .wowPayBackground()
                .navigationTitle(Localized.devicePurposeCold)
            }
        }
        .alert(Localized.devicePurposeStorageError, isPresented: $showsStorageError) {
            Button(Localized.Common.done, role: .cancel) {}
        }
        .tint(Colors.WowPay.brand)
    }

    private func select(_ purpose: DevicePurpose) {
        do {
            try confirm(purpose)
        } catch {
            showsStorageError = true
        }
    }
}
