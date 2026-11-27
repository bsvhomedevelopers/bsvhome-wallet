import enum BsvHomeCore.BsvHomeHeaderActions
import struct BsvHomeCore.BsvHomeHeaderButton
import Primitives
@testable import PrimitivesComponents
import PrimitivesTestKit
import Testing

struct WalletHeaderViewModelTests {
    @Test
    func coldObserverKeepsWatchIdentityWhenCoreEnablesUnsignedRequests() {
        let model = WalletHeaderViewModel(
            totalValue: .mock(),
            currencyCode: Currency.usd.rawValue,
            showsPnl: false,
            actions: .buttons(buttons: [BsvHomeHeaderButton(kind: .send, isEnabled: true), BsvHomeHeaderButton(kind: .receive, isEnabled: true)]),
            walletType: .view,
        )
        #expect(model.isWatchWallet)
        #expect(model.isEnabled(.send))
        #expect(model.isEnabled(.receive))
        #expect(!model.isEnabled(.swap))
    }

    @Test
    func title() {
        let model = WalletHeaderViewModel(
            totalValue: .mock(value: 1000),
            currencyCode: Currency.usd.rawValue,
            showsPnl: true,
            actions: .buttons(buttons: []),
        )
        #expect(model.title == "$1,000.0000")
    }

    @Test
    func titleSmallValue() {
        let model = WalletHeaderViewModel(
            totalValue: .mock(value: 0.1041, pnlAmount: 0),
            currencyCode: Currency.usd.rawValue,
            showsPnl: true,
            actions: .buttons(buttons: []),
        )
        #expect(model.title == "$0.1041")
    }

    @Test
    func subtitle() {
        let model = WalletHeaderViewModel(
            totalValue: .mock(value: 1000, pnlAmount: 50, pnlPercentage: 5),
            currencyCode: Currency.usd.rawValue,
            showsPnl: true,
            actions: .buttons(buttons: []),
        )
        #expect(model.subtitle == "+$50.0000 (5.0000%)")
    }

    @Test
    func subtitleSmallPnlAmount() {
        let model = WalletHeaderViewModel(
            totalValue: .mock(value: 61.40, pnlAmount: 0.1041, pnlPercentage: 0.17),
            currencyCode: Currency.usd.rawValue,
            showsPnl: true,
            actions: .buttons(buttons: []),
        )
        #expect(model.subtitle == "+$0.1041 (0.1700%)")
    }

    @Test
    func buttonsDisabled() {
        let model = WalletHeaderViewModel(
            totalValue: .mock(),
            currencyCode: Currency.usd.rawValue,
            showsPnl: true,
            actions: .buttons(buttons: [BsvHomeHeaderButton(kind: .send, isEnabled: false), BsvHomeHeaderButton(kind: .swap, isEnabled: false)]),
        )
        #expect(model.buttons.allSatisfy { !$0.isEnabled })
        #expect(!model.isEnabled(.send))
        #expect(!model.isEnabled(.swap))
    }

    @Test
    func watchOnlyActionsDisabled() {
        let model = WalletHeaderViewModel(
            totalValue: .mock(),
            currencyCode: Currency.usd.rawValue,
            showsPnl: true,
            actions: .watchOnly,
        )
        #expect(!model.isEnabled(.send))
        #expect(!model.isEnabled(.receive))
        #expect(!model.isEnabled(.swap))
    }

    @Test
    func onlyDeclaredActionsEnabled() {
        let model = WalletHeaderViewModel(
            totalValue: .mock(),
            currencyCode: Currency.usd.rawValue,
            showsPnl: true,
            actions: .buttons(buttons: [BsvHomeHeaderButton(kind: .receive, isEnabled: true)]),
        )
        #expect(model.isEnabled(.receive))
        #expect(!model.isEnabled(.send))
        #expect(!model.isEnabled(.swap))
    }
}
