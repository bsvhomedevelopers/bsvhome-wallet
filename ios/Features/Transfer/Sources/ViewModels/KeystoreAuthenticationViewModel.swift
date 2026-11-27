// Copyright (c). Gem Wallet. All rights reserved.

import Foundation
import enum BsvHomeCore.BsvHomeKeystoreAuthentication
import BsvHomeCoreServices
import Style

struct KeystoreAuthenticationViewModel {
    let authentication: BsvHomeKeystoreAuthentication

    var authenticationImage: String? {
        switch authentication {
        case .biometrics: SystemImage.faceid
        case .passcode: SystemImage.lock
        case .none: .none
        }
    }
}
