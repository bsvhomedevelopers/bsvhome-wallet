// Copyright (c). Gem Wallet. All rights reserved.

import Components
import struct BsvHomeCore.BsvHomeConfirmButton
import enum BsvHomeCore.BsvHomeKeystoreAuthentication
import BsvHomeCoreServices
import Localization
import Primitives
import Style
import SwiftUI

struct ConfirmButtonViewModel: StateButtonViewable {
    private let onAction: @MainActor @Sendable () -> Void
    private let button: BsvHomeConfirmButton
    private let authentication: BsvHomeKeystoreAuthentication?

    init(
        button: BsvHomeConfirmButton,
        authentication: BsvHomeKeystoreAuthentication?,
        onAction: @MainActor @Sendable @escaping () -> Void,
    ) {
        self.button = button
        self.authentication = authentication
        self.onAction = onAction
    }

    var title: String {
        switch button.kind {
        case .confirm: Localized.Transfer.confirm
        case .retry: Localized.Common.tryAgain
        }
    }

    var icon: Image? {
        guard button.kind == .confirm, button.state == .enabled,
              let authentication,
              let systemName = KeystoreAuthenticationViewModel(authentication: authentication).authenticationImage
        else { return nil }
        return Image(systemName: systemName)
    }

    var type: ButtonType {
        switch button.state {
        case .disabled: .primary(.disabled)
        case .loading: .primary(.loading())
        case .enabled: .primary(.normal)
        }
    }

    func action() {
        onAction()
    }
}
