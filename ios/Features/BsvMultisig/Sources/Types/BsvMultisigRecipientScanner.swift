import SwiftUI

public typealias BsvMultisigRecipientScanner = @MainActor (@escaping @MainActor (String) -> Void) -> Void

public extension EnvironmentValues {
    @Entry var bsvMultisigRecipientScanner: BsvMultisigRecipientScanner? = nil
}
