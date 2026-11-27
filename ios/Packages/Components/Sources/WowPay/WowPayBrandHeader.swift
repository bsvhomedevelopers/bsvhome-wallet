import Style
import SwiftUI

public struct WowPayBrandHeader<Trailing: View>: View {
    public enum Appearance {
        case brand
        case walletSelection(onSelect: () -> Void)
    }

    private let title: String
    private let subtitle: String
    private let mark: Image
    private let trailing: Trailing
    private let appearance: Appearance

    public init(title: String, subtitle: String, mark: Image, appearance: Appearance = .brand, @ViewBuilder trailing: () -> Trailing) {
        self.title = title
        self.subtitle = subtitle
        self.mark = mark
        self.trailing = trailing()
        self.appearance = appearance
    }

    public var body: some View {
        switch appearance {
        case .brand:
            brandHeader
        case let .walletSelection(onSelect):
            HStack(spacing: Spacing.small) {
                Button(action: onSelect) {
                    HStack(spacing: Spacing.small) {
                        mark.resizable().scaledToFit()
                            .frame(width: Sizing.wowPay.walletHeaderMark, height: Sizing.wowPay.walletHeaderMark)
                            .accessibilityHidden(true)
                        Text(title)
                            .font(.wowPay.walletTitle)
                            .lineLimit(1)
                            .truncationMode(.tail)
                        Image(systemName: "chevron.down")
                            .font(.body.weight(.semibold))
                            .frame(width: Sizing.wowPay.walletHeaderChevron)
                    }
                    .padding(.vertical, Spacing.space12)
                    .padding(.horizontal, Spacing.space4)
                    .contentShape(RoundedRectangle(cornerRadius: Sizing.wowPay.controlCorner))
                }
                .buttonStyle(.plain)
                .accessibilityIdentifier("walletBar")
                Spacer(minLength: .zero)
                trailing
                    .frame(minWidth: Sizing.wowPay.minimumTapTarget, minHeight: Sizing.wowPay.minimumTapTarget)
            }
            .foregroundStyle(Colors.WowPay.heading)
            .frame(minHeight: Sizing.wowPay.walletHeaderHeight)
            .padding(.top, Spacing.wowPay.walletHeaderTop)
        }
    }

    private var brandHeader: some View {
        ViewThatFits(in: .horizontal) {
            header(
                titleFont: .wowPay.brand,
                subtitleFont: .wowPay.brandSubtitle,
                markWidth: Sizing.wowPay.brandMarkWidth,
                markHeight: Sizing.wowPay.brandMarkHeight,
                subtitleTracking: WowPayTypography.brandSubtitleTracking,
            )
            .frame(minWidth: Sizing.wowPay.regularBrandMinimumWidth, minHeight: Sizing.wowPay.brandHeaderHeight)
            header(
                titleFont: .wowPay.compactBrand,
                subtitleFont: .wowPay.compactBrandSubtitle,
                markWidth: Sizing.wowPay.compactBrandMarkWidth,
                markHeight: Sizing.wowPay.compactBrandMarkHeight,
                subtitleTracking: WowPayTypography.compactBrandSubtitleTracking,
            )
            .frame(minHeight: Sizing.wowPay.compactBrandHeaderHeight)
        }
    }

    private func header(titleFont: WowPayFont, subtitleFont: WowPayFont, markWidth: CGFloat, markHeight: CGFloat, subtitleTracking: CGFloat) -> some View {
        HStack(spacing: .small) {
            mark
                .resizable()
                .scaledToFit()
                .frame(width: markWidth, height: markHeight)
                .accessibilityHidden(true)
            VStack(alignment: .leading, spacing: .space6) {
                Text(title)
                    .font(titleFont)
                    .tracking(WowPayTypography.brandTracking)
                    .foregroundStyle(Colors.WowPay.heading)
                Text(subtitle)
                    .font(subtitleFont)
                    .tracking(subtitleTracking)
                    .foregroundStyle(Colors.WowPay.brandSubtitle)
            }
            .fixedSize(horizontal: true, vertical: false)
            Spacer(minLength: .zero)
            if Trailing.self != EmptyView.self {
                trailing
                    .frame(minWidth: Sizing.wowPay.minimumTapTarget, minHeight: Sizing.wowPay.minimumTapTarget)
                    .foregroundStyle(Colors.WowPay.heading)
            }
        }
    }
}

#Preview {
    WowPayBrandHeader(title: "WowPay", subtitle: "Freedom to pay", mark: Image(systemName: "w.circle.fill")) {
        Button("Notifications", systemImage: "bell", action: {})
            .labelStyle(.iconOnly)
    }
    .padding(Spacing.wowPay.page)
    .wowPayBackground()
}
