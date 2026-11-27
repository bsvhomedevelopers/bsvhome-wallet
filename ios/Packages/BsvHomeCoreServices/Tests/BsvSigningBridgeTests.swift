import CryptoKit
import Foundation
import BsvHomeCore
import BsvHomeCorePrimitives
import BsvHomeCorePrimitivesTestKit
@testable import BsvHomeCoreServices
import BsvHomeCoreServicesTestKit
import Primitives
import Testing

struct BsvSigningBridgeTests {
    @Test
    func coldSigningAndQrMatchIndependentSdkBytes() throws {
        let vector = try JSONDecoder().decode(ColdVector.self, from: BsvColdWalletSigningScopeFixture.signingVectorData())
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString, isDirectory: true)
        defer { try? FileManager.default.removeItem(at: directory) }
        let keystore = try LocalKeystore(directoryURL: directory, keystorePassword: MockKeystorePassword(memoryPassword: LocalKeystore.password), environment: .wowPay)
        let wallet = try keystore.importWallet(name: "Public cold fixture", type: .singlePhrase(words: LocalKeystore.words, chain: Primitives.Chain.bitcoinSv.map()))
        let native = keystore.bsvHomeKeystore
        let review = try native.bsvColdReview(requestJson: vector.requestJson, descriptorJson: vector.pairingJson)
        #expect(review == vector.reviewJson)

        let signedJson = try native.bsvColdSign(
            keystoreId: native.keystoreId(walletId: wallet.id.id),
            requestJson: vector.requestJson,
            reviewJson: review,
            password: native.decodePassword(password: keystore.keystorePassword(createIfMissing: false)),
        )
        let signed = try JSONDecoder().decode(ColdVector.Signed.self, from: Data(signedJson.utf8))
        #expect(signed == vector.signed)
        #expect(try native.bsvColdTransactionId(requestJson: vector.requestJson, reviewJson: review, signedJson: signedJson) == vector.signed.transactionId)
        #expect(try native.bsvColdQrFrames(payloadJson: vector.requestJson) == vector.qrFrames)
        #expect(try native.bsvColdQrAssemble(frames: Array(vector.qrFrames.reversed())) == vector.requestJson)
    }

    @Test
    func multisigKeystoreSignatureAndAssemblyMatchIndependentSdkBytes() async throws {
        let fixture = try #require(JSONSerialization.jsonObject(with: resource("bsv-multisig-signing-bridge")) as? [String: Any])
        let proposal = try #require(fixture["proposal"] as? [String: Any])
        let policy = try #require(proposal["policy"] as? [String: Any])
        let signatureSets = try #require(fixture["signatureSets"] as? [[String: Any]])
        let recipient = try #require(proposal["recipient"] as? String)
        let amount = try #require(proposal["amount"] as? UInt64)
        let fee = try #require(proposal["fee"] as? UInt64)
        let expectedRaw = try #require(fixture["signedTransaction"] as? String)
        let expectedTransactionId = try #require(fixture["signedTransactionId"] as? String)
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString, isDirectory: true)
        defer { try? FileManager.default.removeItem(at: directory) }
        let keystore = try LocalKeystore(directoryURL: directory, keystorePassword: MockKeystorePassword(memoryPassword: LocalKeystore.password), environment: .wowPay)
        let wallet = try keystore.importWallet(name: "Public multisig fixture", type: .privateKey(value: String(repeating: "01", count: 32), chain: Primitives.Chain.bitcoinSv.map()))
        let publicKey = try await keystore.bsvMultisigPublicKey(wallet: wallet)
        let expectedSignature = try #require(signatureSets.first { $0["publicKey"] as? String == publicKey })
        let policyJson = try json(policy)
        let proposalJson = try json(proposal)
        let scopeFixture = try await BsvMultisigActionScopeFixture.make()
        let scope = try scopeFixture.beginRetryFunding()
        let signedJson = try await keystore.signBsvMultisigProposal(wallet: wallet, policyJSON: policyJson, proposalJSON: proposalJson, recipient: recipient, amount: amount, maximumFee: fee, scope: scope)
        let signed = try #require(JSONSerialization.jsonObject(with: Data(signedJson.utf8)) as? [String: Any])
        #expect(try json(signed) == json(expectedSignature))

        let signatures = signatureSets.map { $0["publicKey"] as? String == publicKey ? signed : $0 }
        let raw = try keystore.bsvHomeKeystore.bsvMultisigAssemble(policyJson: policyJson, proposalJson: proposalJson, signaturesJson: json(signatures))
        #expect(raw == expectedRaw)
        let transactionBytes = try Data.from(hex: raw)
        let firstHash = SHA256.hash(data: transactionBytes)
        let transactionId = Data(SHA256.hash(data: Data(firstHash)).reversed()).hex
        #expect(transactionId == expectedTransactionId)
    }

    private func resource(_ name: String) throws -> Data {
        let url = try #require(Bundle.module.url(forResource: name, withExtension: "json"))
        return try Data(contentsOf: url)
    }

    private func json(_ value: Any) throws -> String {
        try String(decoding: JSONSerialization.data(withJSONObject: value, options: [.sortedKeys, .withoutEscapingSlashes]), as: UTF8.self)
    }

    private struct ColdVector: Decodable {
        struct Signed: Decodable, Equatable {
            let transactionId: String
            let rawTransaction: String
        }

        let pairingJson: String
        let requestJson: String
        let reviewJson: String
        let signed: Signed
        let qrFrames: [String]
    }
}
