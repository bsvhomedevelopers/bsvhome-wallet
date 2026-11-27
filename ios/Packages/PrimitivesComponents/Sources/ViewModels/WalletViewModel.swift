import Components
import Foundation
import class BsvHomeCore.BsvHomeAddressService
import struct BsvHomeCore.BsvHomeWalletRow
import func BsvHomeCore.walletRow
import BsvHomeCorePrimitives
import Localization
import Primitives
import Style
import SwiftUI

public struct WalletViewModel: Sendable {
    public let wallet: Wallet

    public init(wallet: Wallet) {
        self.wallet = wallet
    }

    public var name: String {
        wallet.name
    }

    public var subType: String {
        switch row.subtitle {
        case .multicoin: Localized.Wallet.multicoin
        case let .account(chain, address): BsvHomeAddressService.shared.format(address: address, chain: Primitives.Chain(core: chain), style: .extra(extra: 1))
        }
    }

    public var image: Image {
        switch row.placeholder {
        case .multicoin: Images.Logo.logo
        case let .chain(chain): ChainImage(chain: Primitives.Chain(core: chain)).image
        }
    }

    public var subImage: Image? {
        row.showsWatchBadge ? Images.Wallets.watch : nil
    }

    private var row: BsvHomeWalletRow {
        walletRow(wallet: wallet.map())
    }

    public var hasAvatar: Bool {
        wallet.imageUrl != nil
    }

    public var avatarImage: AssetImage {
        AssetImage(
            type: .text(wallet.name),
            imageURL: wallet.imageUrl.map { ImageSource($0).url },
            placeholder: image,
            chainPlaceholder: subImage,
        )
    }
}

extension WalletViewModel: Identifiable, Hashable {
    public var id: String {
        wallet.id.id
    }
}
