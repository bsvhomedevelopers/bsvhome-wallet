public import class BsvHomeCore.BsvHomeBsvMultisigActionScope
public import protocol BsvHomeCore.BsvHomeBsvMultisigSigner
public import struct BsvHomeCore.Wallet
import enum BsvHomeCore.BsvHomeBsvMultisigError
import enum BsvHomeCore.BsvHomeServiceError
import BsvHomeCorePrimitives
import Primitives

public final class BsvHomeCoreBsvMultisigSigner: BsvHomeBsvMultisigSigner, Sendable {
    private let keystore: LocalKeystore

    public init(keystore: LocalKeystore) {
        self.keystore = keystore
    }

    public func publicKey(wallet: BsvHomeCore.Wallet) async throws -> String {
        do {
            return try await keystore.bsvMultisigPublicKey(wallet: wallet.map())
        } catch is CancellationError { throw BsvHomeServiceError.Cancelled }
        catch BsvHomeBsvMultisigError.Cancelled { throw BsvHomeServiceError.Cancelled }
        catch where error.isAuthenticationCancelled {
            throw BsvHomeServiceError.Cancelled
        }
    }

    public func authenticate(wallet: BsvHomeCore.Wallet, scope: BsvHomeBsvMultisigActionScope?) async throws {
        do {
            try await keystore.authenticateBsvMultisigWallet(wallet: wallet.map(), scope: scope)
        } catch is CancellationError { throw BsvHomeServiceError.Cancelled }
        catch BsvHomeBsvMultisigError.Cancelled { throw BsvHomeServiceError.Cancelled }
        catch where error.isAuthenticationCancelled { throw BsvHomeServiceError.Cancelled }
    }

    public func signProposal(wallet: BsvHomeCore.Wallet, policyJson: String, proposalJson: String, recipient: String, amount: UInt64, maximumFee: UInt64, scope: BsvHomeBsvMultisigActionScope) async throws -> String {
        do {
            return try await keystore.signBsvMultisigProposal(wallet: wallet.map(), policyJSON: policyJson, proposalJSON: proposalJson, recipient: recipient, amount: amount, maximumFee: maximumFee, scope: scope)
        } catch is CancellationError { throw BsvHomeServiceError.Cancelled }
        catch BsvHomeBsvMultisigError.Cancelled { throw BsvHomeServiceError.Cancelled }
        catch where error.isAuthenticationCancelled {
            throw BsvHomeServiceError.Cancelled
        }
    }

    public func signFunding(wallet: BsvHomeCore.Wallet, fundingJson: String, reviewJson: String, scope: BsvHomeBsvMultisigActionScope) async throws -> String {
        do {
            return try await keystore.signBsvMultisigFunding(wallet: wallet.map(), fundingJSON: fundingJson, reviewJSON: reviewJson, scope: scope)
        } catch is CancellationError { throw BsvHomeServiceError.Cancelled }
        catch BsvHomeBsvMultisigError.Cancelled { throw BsvHomeServiceError.Cancelled }
        catch where error.isAuthenticationCancelled {
            throw BsvHomeServiceError.Cancelled
        }
    }
}
