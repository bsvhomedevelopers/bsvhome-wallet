import PhaseOne

@MainActor
struct PhaseOneModels {
    let account: PlatformAccountViewModel
    let mpc: MpcWalletViewModel
    let hd: PlatformHdViewModel
    let alternative: PlatformAlternativeViewModel
    let recipientInputs: MpcRecipientInputCoordinator

    init(services: PhaseOneServices, authenticate: @escaping @MainActor @Sendable () async throws -> Void, onWalletChange: @escaping @MainActor @Sendable () async throws -> Void) {
        account = PlatformAccountViewModel(service: services.account)
        let account = account
        let onSessionInvalidated: @MainActor @Sendable () async -> Void = { await account.refreshSessionProjection() }
        mpc = MpcWalletViewModel(service: services.mpc, authenticate: authenticate, onWalletChange: onWalletChange, onSessionInvalidated: onSessionInvalidated)
        hd = PlatformHdViewModel(service: services.hd, onSessionInvalidated: onSessionInvalidated)
        alternative = PlatformAlternativeViewModel(service: services.alternative, onSessionInvalidated: onSessionInvalidated)
        recipientInputs = services.recipientInputs
    }
}
