import SwiftUI

public typealias BsvMultisigInvitationScanner = @MainActor (@escaping @MainActor (String) -> Void) -> Void

public extension EnvironmentValues {
    @Entry var bsvMultisigInvitationScanner: BsvMultisigInvitationScanner? = nil
}
