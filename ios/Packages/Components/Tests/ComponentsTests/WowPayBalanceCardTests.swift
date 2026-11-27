#if canImport(UIKit)
    import Components
    import SwiftUI
    import Testing

    @MainActor
    struct WowPayBalanceCardTests {
        @Test(arguments: [320.0, 375.0, 402.0])
        func privacyKeepsCardHeight(width: Double) throws {
            for coinSelectable in [false, true] {
                for amount in ["$0.0000", "$1,234.5678", "$123,456,789,012.3456"] {
                    for secondaryAmount in ["≈ $0.0000", "≈ $123,456,789,012,345.6789"] {
                        let visible = try size(amount: amount, secondaryAmount: secondaryAmount, hidden: false, width: width, coinSelectable: coinSelectable)
                        let hidden = try size(amount: amount, secondaryAmount: secondaryAmount, hidden: true, width: width, coinSelectable: coinSelectable)
                        #expect(visible == hidden)
                    }
                }
            }
        }

        private func size(amount: String, secondaryAmount: String, hidden: Bool, width: Double, coinSelectable: Bool) throws -> CGSize {
            let onSelectCoin: (() -> Void)? = coinSelectable ? {} : nil
            let card = WowPayBalanceCard(
                title: "总资产 (USD)",
                value: amount,
                subtitle: "BSV",
                secondaryAmount: secondaryAmount,
                isHidden: .constant(hidden),
                coinImage: coinSelectable ? Image(systemName: "bitcoinsign.circle.fill") : nil,
                onSelectCoin: onSelectCoin,
            )
            .frame(width: width)
            let renderer = ImageRenderer(content: card)
            return try #require(renderer.uiImage).size
        }
    }
#endif
