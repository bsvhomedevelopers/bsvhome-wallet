import Style
import SwiftUI
import Testing
import UIKit

@MainActor
@Suite(.serialized)
struct WowPayTypographyDynamicTypeTests {
    @Test
    func semanticSystemFontScalesInTheHostedMeasurement() {
        let normal = measure(Text(Self.sample).font(.body), at: .large)
        let enlarged = measure(Text(Self.sample).font(.body), at: .accessibility3)
        #expect(enlarged.width > normal.width)
        #expect(enlarged.height > normal.height)
    }

    @Test(arguments: Role.allCases)
    func wowPayFontsKeepTheirExistingDefaultMetrics(_ role: Role) {
        let actual = measure(sample(role), at: .large)
        let expected = measure(Text(Self.sample).font(role.existingDefault), at: .large)
        #expect(abs(actual.width - expected.width) < 0.5)
        #expect(abs(actual.height - expected.height) < 0.5)
    }

    @Test(arguments: Role.allCases)
    func wowPayFontsScaleWithTheNativeTextSize(_ role: Role) {
        let normal = measure(sample(role), at: .large)
        let enlarged = measure(sample(role), at: .accessibility3)
        #expect(enlarged.width > normal.width)
        #expect(enlarged.height > normal.height)
    }

    @Test
    func wowPayTextStyleKeepsItsDefaultAndScalesWithTheNativeTextSize() {
        let normal = measure(Text(Self.sample).textStyle(.wowPay.rowTitle), at: .large)
        let expected = measure(Text(Self.sample).font(.system(size: 16, weight: .bold)), at: .large)
        #expect(abs(normal.width - expected.width) < 0.5)
        #expect(abs(normal.height - expected.height) < 0.5)
        let enlarged = measure(Text(Self.sample).textStyle(.wowPay.rowTitle), at: .accessibility3)
        #expect(enlarged.width > normal.width)
        #expect(enlarged.height > normal.height)
    }

    @Test(arguments: [DynamicTypeSize.large, .accessibility3])
    func nativeTokensPreserveTheExistingSemanticFontMetrics(size: DynamicTypeSize) {
        let actual = measure(Text(Self.sample).font(.wowPay.multisigValue), at: size)
        let expected = measure(Text(Self.sample).font(.footnote.weight(.semibold)), at: size)
        #expect(abs(actual.width - expected.width) < 0.5)
        #expect(abs(actual.height - expected.height) < 0.5)
    }

    @Test(arguments: [DynamicTypeSize.large, .accessibility3])
    func anUnsetTokenPreservesTheNativeInheritedFont(size: DynamicTypeSize) {
        let actual = measure(Text(Self.sample).font(nil as WowPayFont?).font(.title2), at: size)
        let expected = measure(Text(Self.sample).font(nil as Font?).font(.title2), at: size)
        #expect(abs(actual.width - expected.width) < 0.5)
        #expect(abs(actual.height - expected.height) < 0.5)
    }

    enum Role: CaseIterable, Sendable {
        case rowTitle
        case body
        case balance

        var existingDefault: Font {
            switch self {
            case .rowTitle: .system(size: 16, weight: .bold)
            case .body: .system(size: 16, weight: .medium)
            case .balance: .system(size: 39, weight: .bold)
            }
        }
    }

    private static let sample = "Typography 012345"

    @ViewBuilder
    private func sample(_ role: Role) -> some View {
        switch role {
        case .rowTitle: Text(Self.sample).font(.wowPay.rowTitle)
        case .body: Text(Self.sample).font(.wowPay.body)
        case .balance: Text(Self.sample).font(.wowPay.balance)
        }
    }

    private func measure(_ content: some View, at size: DynamicTypeSize) -> CGSize {
        let controller = UIHostingController(
            rootView: content
                .environment(\.dynamicTypeSize, size)
                .fixedSize(horizontal: true, vertical: true),
        )
        controller.loadViewIfNeeded()
        let measured = controller.sizeThatFits(in: CGSize(width: 10000, height: 10000))
        #expect(measured.width.isFinite && measured.width > 0)
        #expect(measured.height.isFinite && measured.height > 0)
        return measured
    }
}
