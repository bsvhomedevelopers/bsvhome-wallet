import SwiftUI

public struct BsvMultisigIdentityContext: Sendable {
    public let options: [BsvMultisigIdentityOption]
    public let selectedWalletId: String?
    public let isSelecting: Bool
    public let onSelect: @MainActor @Sendable (String) -> Void
    public let onCreateDedicated: (@MainActor @Sendable (BsvMultisigIdentityEntry) -> Void)?

    public init(
        options: [BsvMultisigIdentityOption],
        selectedWalletId: String?,
        isSelecting: Bool = false,
        onSelect: @escaping @MainActor @Sendable (String) -> Void,
        onCreateDedicated: (@MainActor @Sendable (BsvMultisigIdentityEntry) -> Void)? = nil,
    ) {
        self.options = options
        self.selectedWalletId = selectedWalletId
        self.isSelecting = isSelecting
        self.onSelect = onSelect
        self.onCreateDedicated = onCreateDedicated
    }
}

public extension EnvironmentValues {
    @Entry var bsvMultisigIdentityContext: BsvMultisigIdentityContext? = nil
}
