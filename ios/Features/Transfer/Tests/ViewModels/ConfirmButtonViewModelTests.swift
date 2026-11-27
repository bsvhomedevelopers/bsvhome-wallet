// Copyright (c). Gem Wallet. All rights reserved.

import struct BsvHomeCore.BsvHomeConfirmButton
import Localization
@testable import Primitives
import Testing
@testable import Transfer

struct ConfirmButtonViewModelTests {
    @Test
    func confirmTitle() {
        let model = ConfirmButtonViewModel(button: BsvHomeConfirmButton(kind: .confirm, state: .enabled), authentication: nil, onAction: {})
        #expect(model.title == Localized.Transfer.confirm)
        #expect(!model.type.isDisabled)
    }

    @Test
    func retryTitle() {
        let model = ConfirmButtonViewModel(button: BsvHomeConfirmButton(kind: .retry, state: .enabled), authentication: nil, onAction: {})
        #expect(model.title == Localized.Common.tryAgain)
    }

    @Test
    func disabledState() {
        let model = ConfirmButtonViewModel(button: BsvHomeConfirmButton(kind: .confirm, state: .disabled), authentication: nil, onAction: {})
        #expect(model.type.isDisabled)
    }

    @Test
    func loadingStateHidesTheAuthenticationIcon() {
        let model = ConfirmButtonViewModel(button: BsvHomeConfirmButton(kind: .confirm, state: .loading), authentication: .biometrics, onAction: {})
        #expect(model.icon == nil)
        #expect(ConfirmButtonViewModel(button: BsvHomeConfirmButton(kind: .confirm, state: .enabled), authentication: .biometrics, onAction: {}).icon != nil)
    }
}
