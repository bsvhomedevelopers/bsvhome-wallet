import Components
import Localization
import Style
import SwiftUI

struct PhaseOnePage<Content: View>: View {
    let title: String
    let isBusy: Bool
    let error: String?
    @ViewBuilder let content: () -> Content

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: Spacing.wowPay.card) {
                if isBusy {
                    ProgressView().frame(maxWidth: .infinity)
                }
                if let error {
                    Label(error, systemImage: "exclamationmark.triangle")
                        .font(.body.weight(.medium))
                        .foregroundStyle(Colors.WowPay.heading)
                        .wowPayCard()
                        .accessibilityIdentifier("phaseOneError")
                }
                content()
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            .padding(Spacing.wowPay.page)
        }
        .wowPayBackground()
        .navigationTitle(title)
        .navigationBarTitleDisplayMode(.inline)
        .toolbar(.visible, for: .navigationBar)
        .scrollDismissesKeyboard(.interactively)
    }
}
