import AuthenticationServices
import Foundation
import BsvHomeCore
@testable import BsvHomeCoreServices
import Testing

@MainActor
struct PlatformAuthenticatorTests {
    @Test
    func preservesChallengeAndRequiredVerificationInSystemRequest() throws {
        let options = try PlatformWebAuthnOptions.decode(#"{"challenge":"AQID","rp":{"id":"example.com"},"user":{"id":"BAUG","name":"test@example.com"},"pubKeyCredParams":[{"type":"public-key","alg":-7}],"authenticatorSelection":{"userVerification":"required","residentKey":"required","authenticatorAttachment":"platform"}}"#)
        let request = try #require(options.request(operation: .register) as? ASAuthorizationPlatformPublicKeyCredentialRegistrationRequest)
        #expect(request.challenge == Data([1, 2, 3]))
        #expect(request.userID == Data([4, 5, 6]))
        #expect(request.relyingPartyIdentifier == "example.com")
        #expect(request.userVerificationPreference == .required)
    }

    @Test(arguments: ["AQID=", "AQI+", "AQI/", "A", "AR"])
    func rejectsPaddedStandardOrNoncanonicalBase64URL(_ text: String) {
        #expect(throws: BsvHomePlatformAccountError.self) { try PlatformWebAuthnOptions.binary(text) }
    }

    @Test(arguments: [
        #"{"challenge":"AQID","rpId":"example.com","userVerification":"discouraged"}"#,
        #"{"challenge":"AQID","rpId":"example.com","extensions":true}"#,
        #"{"challenge":"AQID","rpId":"example.com","extensions":{"prf":{}}}"#,
    ])
    func refusesUnsupportedRequirementsInsteadOfDowngradingThem(_ json: String) {
        #expect(throws: BsvHomePlatformAccountError.self) { try PlatformWebAuthnOptions.decode(json).request(operation: .login) }
    }

    @Test
    func independentlyVerifiesP256SpkiAndDerOverActionHashText() throws {
        let url = try #require(Bundle.module.url(forResource: "phaseone-p256-public-proof", withExtension: "json"))
        let vector = try JSONDecoder().decode(PublicProof.self, from: Data(contentsOf: url))
        let authenticator = BsvHomeCorePlatformAuthenticator()
        #expect(try authenticator.verifyOfflineProof(publicKey: vector.publicKey, actionHash: vector.actionHash, signature: vector.signature))
        #expect(try !authenticator.verifyOfflineProof(publicKey: vector.publicKey, actionHash: "0" + vector.actionHash, signature: vector.signature))
        #expect(try !authenticator.verifyOfflineProof(publicKey: vector.publicKey, actionHash: vector.actionHash, signature: "AA"))
    }
}

private struct PublicProof: Decodable {
    let publicKey: String
    let actionHash: String
    let signature: String
}
