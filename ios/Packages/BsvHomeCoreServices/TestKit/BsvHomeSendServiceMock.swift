import BigInt
import typealias BsvHomeCore.Asset
import struct BsvHomeCore.Contact
import struct BsvHomeCore.ContactAddress
import class BsvHomeCore.BsvHomeAmountService
import struct BsvHomeCore.BsvHomeBsvMultisigPaymentRequest
import enum BsvHomeCore.BsvHomeNameRecordState
import class BsvHomeCore.BsvHomePaymailDestinationService
import struct BsvHomeCore.BsvHomePaymentRecipient
import struct BsvHomeCore.BsvHomeRecipient
import class BsvHomeCore.BsvHomeRecipientService
import struct BsvHomeCore.BsvHomeSendContactRecipient
import enum BsvHomeCore.BsvHomeSendDestination
import class BsvHomeCore.BsvHomeSendPublicFundingPreparation
import class BsvHomeCore.BsvHomeSendService
import protocol BsvHomeCore.BsvHomeSendServiceProtocol
import struct BsvHomeCore.BsvHomeSendWalletRecipient
import struct BsvHomeCore.BsvHomeTransferData
import struct BsvHomeCore.Wallet
import typealias BsvHomeCore.WalletId
import BsvHomeCorePrimitivesTestKit
import Primitives

@MainActor
public final class BsvHomeSendServiceMock: BsvHomeSendServiceProtocol {
    private let service: BsvHomeSendService
    private let onTransferData: (@MainActor (Asset, String, BigInt, Bool) async throws -> BsvHomeTransferData)?
    public private(set) var inputs: [String] = []
    public var onResolveDestination: (@MainActor (Asset, String, BigInt, Bool) async throws -> BsvHomeSendDestination)?

    public init(onTransferData: (@MainActor (Asset, String, BigInt, Bool) async throws -> BsvHomeTransferData)? = nil) {
        service = BsvHomeSendService(
            recipient: BsvHomeRecipientService.mock(),
            amount: BsvHomeAmountService.mock(),
            paymail: BsvHomePaymailDestinationService(provider: StubAlienProvider()),
        )
        self.onTransferData = onTransferData
    }

    public nonisolated func getCurrency() -> String {
        service.getCurrency()
    }

    public nonisolated func isPaymailInput(input: String) -> Bool {
        service.isPaymailInput(input: input)
    }

    public nonisolated func supportsPaymail(asset: Asset) -> Bool {
        service.supportsPaymail(asset: asset)
    }

    public nonisolated func recipient(asset: Asset, input: String, state: BsvHomeNameRecordState, name: String?) throws -> BsvHomeRecipient {
        try service.recipient(asset: asset, input: input, state: state, name: name)
    }

    public nonisolated func scan(asset: Asset, input: String) throws -> BsvHomePaymentRecipient {
        try service.scan(asset: asset, input: input)
    }

    public nonisolated func contactRecipients(asset: Asset, contacts: [BsvHomeCore.Contact], addresses: [BsvHomeCore.ContactAddress]) throws -> [BsvHomeSendContactRecipient] {
        try service.contactRecipients(asset: asset, contacts: contacts, addresses: addresses)
    }

    public nonisolated func walletRecipients(asset: Asset, wallets: [BsvHomeCore.Wallet], sourceWalletId: BsvHomeCore.WalletId) throws -> [BsvHomeSendWalletRecipient] {
        try service.walletRecipients(asset: asset, wallets: wallets, sourceWalletId: sourceWalletId)
    }

    public func transferData(asset: Asset, input: String, state: BsvHomeNameRecordState, name: String?, value: BigInt, useMaxAmount: Bool) async throws -> BsvHomeTransferData {
        inputs.append(input)
        if let onTransferData {
            return try await onTransferData(asset, input, value, useMaxAmount)
        }
        return try await service.transferData(asset: asset, input: input, state: state, name: name, value: value, useMaxAmount: useMaxAmount)
    }

    public func resolveDestination(asset: Asset, input: String, state: BsvHomeNameRecordState, name: String?, value: BigInt, useMaxAmount: Bool) async throws -> BsvHomeSendDestination {
        inputs.append(input)
        if let onResolveDestination {
            return try await onResolveDestination(asset, input, value, useMaxAmount)
        }
        if let onTransferData {
            return try await .transfer(transfer: onTransferData(asset, input, value, useMaxAmount))
        }
        return try await service.resolveDestination(asset: asset, input: input, state: state, name: name, value: value, useMaxAmount: useMaxAmount)
    }

    public nonisolated func confirmPublicFunding(preparation: BsvHomeSendPublicFundingPreparation, value: BigInt) async throws -> BsvHomeBsvMultisigPaymentRequest {
        try await service.confirmPublicFunding(preparation: preparation, value: value)
    }
}
