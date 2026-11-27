import Style
import SwiftUI

public struct WowPayReceiveQRCodeCard: View {
    private let state: StateViewType<UIImage>
    private let title: String
    private let infoTitle: String
    private let receiveTitle: String
    private let retryTitle: String
    private let onHelp: () -> Void
    private let onReceive: () -> Void
    private let onRetry: () -> Void

    public init(state: StateViewType<UIImage>, title: String, infoTitle: String, receiveTitle: String, retryTitle: String, onHelp: @escaping () -> Void, onReceive: @escaping () -> Void, onRetry: @escaping () -> Void) {
        self.state = state
        self.title = title
        self.infoTitle = infoTitle
        self.receiveTitle = receiveTitle
        self.retryTitle = retryTitle
        self.onHelp = onHelp
        self.onReceive = onReceive
        self.onRetry = onRetry
    }

    public var body: some View {
        VStack(alignment: .leading, spacing: Spacing.space16) {
            HStack(spacing: .tiny) {
                Text(title)
                    .font(.wowPay.selectionTitle)
                    .foregroundStyle(Colors.WowPay.heading)
                    .accessibilityAddTraits(.isHeader)
                Button { onHelp() } label: {
                    Image("wowpay-help").renderingMode(.template).resizable().scaledToFit()
                        .frame(width: Sizing.wowPay.paymailHelpIcon, height: Sizing.wowPay.paymailHelpIcon)
                        .foregroundStyle(Colors.WowPay.brand)
                        .contentShape(Rectangle().inset(by: -Spacing.space16))
                }
                .buttonStyle(.plain)
                .accessibilityLabel(infoTitle)
            }
            Group {
                switch state {
                case let .data(image):
                    Button { onReceive() } label: {
                        Image(uiImage: image).resizable().interpolation(.none).scaledToFit()
                            .padding(Spacing.tiny)
                            .background(Colors.whiteSolid)
                    }
                    .buttonStyle(.plain)
                    .accessibilityLabel(receiveTitle)
                    .accessibilityIdentifier("wowPayAssetReceiveQR")
                case .loading:
                    LoadingView()
                case .noData:
                    Text("—").foregroundStyle(Colors.WowPay.secondaryText)
                case .error:
                    Button(retryTitle) { onRetry() }
                        .font(.wowPay.caption)
                }
            }
            .frame(width: Sizing.wowPay.detailQR, height: Sizing.wowPay.detailQR)
            .frame(maxWidth: .infinity)
            Spacer(minLength: .zero)
        }
        .padding(Spacing.wowPay.card)
        .frame(maxWidth: .infinity, minHeight: Sizing.wowPay.paymailCardHeight, alignment: .topLeading)
        .wowPayCard(padding: .zero, cornerRadius: Sizing.wowPay.rowCorner)
    }
}
