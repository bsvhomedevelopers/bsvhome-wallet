import struct BsvHomeCore.BsvHomeBsvMultisigField
import Localization

extension BsvHomeBsvMultisigField {
    var localizedValue: String {
        title == "network" ? Localized.multisigMainnet : value
    }

    var localizedTitle: String {
        switch title {
        case "network": Localized.wowpayAssetNetwork
        case "configurationId": Localized.multisigConfigurationId
        case "signerWalletId": Localized.multisigSigningWallet
        case "publicKey": Localized.multisigPublicKey
        case "input": Localized.multisigInput
        case "payment": Localized.multisigPaymentOutput
        case "amount": Localized.wowpayAmount
        case "inputCount": Localized.multisigInput
        case "change": Localized.multisigChangeOutput
        case "fee": Localized.Transfer.networkFee
        default: title
        }
    }
}
