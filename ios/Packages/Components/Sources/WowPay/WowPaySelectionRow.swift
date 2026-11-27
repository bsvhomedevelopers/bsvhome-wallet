import Style
import SwiftUI

public struct WowPaySelectionRow: View {
    public enum Style {
        case standard
        case stacked
    }

    @ScaledMetric(relativeTo: .caption2) private var supportingLineHeight = Sizing.wowPay.walletPickerSupportingLineHeight

    private let title: String
    private let subtitle: String
    private let image: Image
    private let titleTag: String?
    private let isSelected: Bool
    private let style: Style
    private let detail: String?
    private let trailingImage: Image?

    public init(title: String, subtitle: String, image: Image, titleTag: String? = nil, isSelected: Bool = false, style: Style = .standard, detail: String? = nil, trailingImage: Image? = nil) {
        self.title = title
        self.subtitle = subtitle
        self.image = image
        self.titleTag = titleTag
        self.isSelected = isSelected
        self.style = style
        self.detail = detail
        self.trailingImage = trailingImage
    }

    public var body: some View {
        Group {
            switch style {
            case .standard: standardRow
            case .stacked: stackedRow
            }
        }
        .padding(Spacing.wowPay.servicePadding)
        .background(Colors.WowPay.surface, in: RoundedRectangle(cornerRadius: Sizing.wowPay.selectionRowCorner))
        .overlay {
            RoundedRectangle(cornerRadius: Sizing.wowPay.selectionRowCorner)
                .strokeBorder(Colors.WowPay.selectionBorder, lineWidth: .space1)
        }
        .accessibilityElement(children: .combine)
        .accessibilityAddTraits(isSelected ? .isSelected : [])
    }

    private var standardRow: some View {
        HStack(spacing: Spacing.space12) {
            image.resizable().scaledToFit()
                .frame(width: Sizing.wowPay.assetIcon, height: Sizing.wowPay.assetIcon)
                .accessibilityHidden(true)
            ViewThatFits(in: .horizontal) {
                HStack(spacing: Spacing.space12) {
                    titleContent.fixedSize(horizontal: true, vertical: true)
                    Spacer(minLength: Spacing.small)
                    subtitleText.fixedSize(horizontal: true, vertical: true)
                }
                VStack(alignment: .leading, spacing: Spacing.tiny) {
                    titleContent
                    subtitleText
                }
                .frame(maxWidth: .infinity, alignment: .leading)
            }
            if isSelected {
                Image(systemName: "checkmark.circle.fill")
                    .foregroundStyle(Colors.WowPay.heading)
                    .accessibilityHidden(true)
            }
        }
    }

    private var stackedRow: some View {
        HStack(spacing: Spacing.space12) {
            image.resizable().scaledToFit()
                .frame(width: Sizing.wowPay.icon, height: Sizing.wowPay.icon)
                .foregroundStyle(Colors.WowPay.icon)
                .accessibilityHidden(true)
            VStack(alignment: .leading, spacing: Spacing.tiny) {
                HStack(alignment: .firstTextBaseline, spacing: Spacing.tiny) {
                    titleContent
                    if isSelected {
                        Text("✓")
                            .font(.wowPay.selectionTitle)
                            .foregroundStyle(Colors.WowPay.heading)
                            .accessibilityHidden(true)
                    }
                }
                stackedSupportingText(subtitle)
                if let detail {
                    stackedSupportingText(detail)
                }
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            if let trailingImage {
                trailingImage.resizable().scaledToFit()
                    .frame(width: Sizing.wowPay.chevron, height: Sizing.wowPay.chevron)
                    .foregroundStyle(Colors.WowPay.icon)
                    .opacity(WowPayOpacity.selectionSubtitle)
                    .accessibilityHidden(true)
            }
        }
    }

    private func stackedSupportingText(_ text: String) -> some View {
        Text(text)
            .font(.wowPay.serviceCaption)
            .foregroundStyle(Colors.WowPay.secondaryText)
            .fixedSize(horizontal: false, vertical: true)
            .frame(minHeight: supportingLineHeight, alignment: .leading)
    }

    private var titleText: some View {
        Text(title).font(.wowPay.selectionTitle).foregroundStyle(Colors.WowPay.heading)
    }

    private var titleContent: some View {
        HStack(spacing: Spacing.tiny) {
            titleText
            if let titleTag {
                ListItemView.TitleTagView(titleTag: TextValue(text: titleTag, style: .wowPay.caption, lineLimit: 1), titleTagType: .none)
            }
        }
    }

    private var subtitleText: some View {
        Text(subtitle).font(.wowPay.body).foregroundStyle(Colors.WowPay.secondaryText.opacity(WowPayOpacity.selectionSubtitle))
    }
}
