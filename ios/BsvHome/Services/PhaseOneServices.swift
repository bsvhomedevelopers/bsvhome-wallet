import class BsvHomeCore.BsvHomeMpcWalletService
import class BsvHomeCore.BsvHomePlatformAccountService
import class BsvHomeCore.BsvHomePlatformAlternativeService
import class BsvHomeCore.BsvHomePlatformHdService

struct PhaseOneServices: Sendable {
    let account: BsvHomePlatformAccountService
    let mpc: BsvHomeMpcWalletService
    let hd: BsvHomePlatformHdService
    let alternative: BsvHomePlatformAlternativeService
    let recipientInputs: MpcRecipientInputCoordinator
}
