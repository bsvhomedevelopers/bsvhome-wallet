// Copyright (c). Gem Wallet. All rights reserved.

import Components
import Foundation
import BsvHomeCorePrimitives
import Localization
import Primitives

@Observable
final class AcceptTermsViewModel {
    let onNext: VoidAction

    init(onNext: VoidAction) {
        self.onNext = onNext
    }

    var termsAndServicesURL: URL {
        AppUrl.page(.termsOfService)
    }

    let title: String = Localized.Onboarding.AcceptTerms.title
    let message: String = Localized.Onboarding.AcceptTerms.message

    var items: [TermItemViewModel] = [
        .init(message: Localized.wowpayTermsBackup),
        .init(message: Localized.wowpayTermsUsage),
        .init(message: Localized.wowpayTermsRecovery),
    ]

    var isConfirmed: Bool {
        items.allSatisfy(\.isConfirmed)
    }

    var state: StateViewType<Bool> {
        isConfirmed ? .data(true) : .noData
    }
}
