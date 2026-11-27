import BsvHomeCore
import BsvHomeCorePrimitives
import Localization
import Primitives

final class BsvHomeCoreLocalizer: BsvHomeLocalizer, Sendable {
    func text(text: BsvHomeLocalizedText) -> String {
        switch text {
        case let .walletDefaultName(index):
            Localized.Wallet.defaultName(Int(index))
        case let .walletDefaultNameChain(chain, index):
            Localized.Wallet.defaultNameChain(Chain(core: chain).networkName, Int(index))
        }
    }
}
