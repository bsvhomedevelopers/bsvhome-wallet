import Style
import SwiftUI

public struct WowPayBottomPanel<Content: View>: View {
    @Environment(\.dismiss) private var dismiss
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @State private var isVisible = false
    @State private var isClosing = false
    @State private var headerHeight: CGFloat = .zero
    @State private var contentHeight: CGFloat = .zero

    private let title: String
    private let closeTitle: String
    private let closeImage: Image?
    private let isDismissDisabled: Bool
    private let content: (@escaping () -> Void) -> Content

    public init(
        title: String,
        closeTitle: String,
        closeImage: Image? = nil,
        isDismissDisabled: Bool = false,
        @ViewBuilder content: @escaping (@escaping () -> Void) -> Content,
    ) {
        self.title = title
        self.closeTitle = closeTitle
        self.closeImage = closeImage
        self.isDismissDisabled = isDismissDisabled
        self.content = content
    }

    public var body: some View {
        GeometryReader { geometry in
            ZStack(alignment: .bottom) {
                Colors.WowPay.modalBackdrop
                    .opacity(isVisible ? 1 : 0)
                    .ignoresSafeArea()
                    .onTapGesture(perform: close)
                    .accessibilityHidden(true)
                    .zIndex(0)
                if isVisible {
                    VStack(alignment: .leading, spacing: Spacing.wowPay.card) {
                        HStack(alignment: .top) {
                            Text(title)
                                .font(.wowPay.sectionTitle)
                                .foregroundStyle(Colors.WowPay.heading)
                                .accessibilityAddTraits(.isHeader)
                            Spacer(minLength: Spacing.small)
                            Button(action: close) {
                                Group {
                                    if let closeImage {
                                        closeImage.resizable().scaledToFit()
                                            .frame(width: Sizing.wowPay.balanceVisibilityIcon, height: Sizing.wowPay.balanceVisibilityIcon)
                                    } else {
                                        Text(closeTitle).font(.wowPay.label)
                                    }
                                }
                                .frame(minWidth: Sizing.wowPay.minimumTapTarget, minHeight: Sizing.wowPay.minimumTapTarget, alignment: .topTrailing)
                                .contentShape(Rectangle())
                            }
                            .foregroundStyle(Colors.WowPay.brand)
                            .accessibilityLabel(closeTitle)
                            .disabled(isDismissDisabled)
                        }
                        .onGeometryChange(for: CGFloat.self, of: { $0.size.height }) { headerHeight = $0 }
                        ScrollView {
                            content(close)
                                .frame(maxWidth: .infinity, alignment: .leading)
                                .onGeometryChange(for: CGFloat.self, of: { $0.size.height }) { contentHeight = $0 }
                        }
                        .scrollBounceBehavior(.basedOnSize)
                        .frame(height: min(contentHeight, max(0, geometry.size.height - headerHeight - Spacing.wowPay.card - Spacing.wowPay.modalTop - Spacing.wowPay.modalBottom)))
                    }
                    .padding(.horizontal, Spacing.wowPay.modalHorizontal)
                    .padding(.top, Spacing.wowPay.modalTop)
                    .padding(.bottom, Spacing.wowPay.modalBottom + geometry.safeAreaInsets.bottom)
                    .background(Colors.WowPay.surface, in: UnevenRoundedRectangle(topLeadingRadius: Sizing.wowPay.modalCorner, topTrailingRadius: Sizing.wowPay.modalCorner))
                    .transition(reduceMotion ? .opacity : .move(edge: .bottom))
                    .zIndex(1)
                }
            }
            .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .bottom)
            .ignoresSafeArea(edges: .bottom)
            .buttonStyle(.plain)
            .allowsHitTesting(!isClosing)
            .accessibilityAction(.escape, close)
            .onAppear {
                withAnimation(reduceMotion ? nil : .easeOut(duration: WowPayMotion.modalDuration)) {
                    isVisible = true
                }
            }
        }
        .interactiveDismissDisabled(isDismissDisabled)
    }

    private func close() {
        guard !isClosing, !isDismissDisabled else { return }
        isClosing = true
        withAnimation(reduceMotion ? nil : .easeInOut(duration: WowPayMotion.modalDuration), completionCriteria: .removed) {
            isVisible = false
        } completion: {
            var transaction = Transaction()
            transaction.disablesAnimations = true
            withTransaction(transaction) { dismiss() }
        }
    }
}
