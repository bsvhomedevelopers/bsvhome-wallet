import AuthenticationServices
import CryptoKit
import Foundation
import enum BsvHomeCore.BsvHomePlatformAccountError
import protocol BsvHomeCore.BsvHomePlatformAuthenticator
import enum BsvHomeCore.BsvHomePlatformPasskeyOperation
#if canImport(UIKit)
    import UIKit
#else
    import AppKit
#endif

@MainActor
public final class BsvHomeCorePlatformAuthenticator: NSObject, Sendable {
    private var continuation: CheckedContinuation<String, Error>?
    private var controller: ASAuthorizationController?
    private var anchor: ASPresentationAnchor?
    private var operationId: UUID?

    override public init() {
        super.init()
    }

    private func finish(_ result: Result<String, Error>) {
        let pending = continuation
        continuation = nil
        controller = nil
        operationId = nil
        pending?.resume(with: result)
    }

    private static func response(_ credential: any ASAuthorizationCredential) throws -> String {
        let identifier: Data
        let response: [String: Any]
        if let credential = credential as? ASAuthorizationPlatformPublicKeyCredentialRegistration {
            guard let attestation = credential.rawAttestationObject else { throw BsvHomePlatformAccountError.InvalidResponse }
            identifier = credential.credentialID
            response = ["clientDataJSON": PlatformWebAuthnOptions.encoded(credential.rawClientDataJSON), "attestationObject": PlatformWebAuthnOptions.encoded(attestation)]
        } else if let credential = credential as? ASAuthorizationPlatformPublicKeyCredentialAssertion {
            identifier = credential.credentialID
            response = [
                "clientDataJSON": PlatformWebAuthnOptions.encoded(credential.rawClientDataJSON),
                "authenticatorData": PlatformWebAuthnOptions.encoded(credential.rawAuthenticatorData),
                "signature": PlatformWebAuthnOptions.encoded(credential.signature),
                "userHandle": credential.userID.isEmpty ? NSNull() : PlatformWebAuthnOptions.encoded(credential.userID),
            ]
        } else {
            throw BsvHomePlatformAccountError.AuthenticationUnavailable
        }
        let object: [String: Any] = ["id": PlatformWebAuthnOptions.encoded(identifier), "rawId": PlatformWebAuthnOptions.encoded(identifier), "type": "public-key", "response": response]
        return try String(decoding: JSONSerialization.data(withJSONObject: object), as: UTF8.self)
    }
}

extension BsvHomeCorePlatformAuthenticator: BsvHomePlatformAuthenticator {
    public func authenticate(operation: BsvHomePlatformPasskeyOperation, optionsJson: String) async throws -> String {
        guard continuation == nil else { throw BsvHomePlatformAccountError.AuthenticationUnavailable }
        try Task.checkCancellation()
        let request = try PlatformWebAuthnOptions.decode(optionsJson).request(operation: operation)
        #if canImport(UIKit)
            anchor = UIApplication.shared.connectedScenes.compactMap { $0 as? UIWindowScene }
                .filter { $0.activationState == .foregroundActive }.flatMap(\.windows).first(where: \.isKeyWindow)
        #else
            anchor = NSApplication.shared.keyWindow
        #endif
        guard anchor != nil else { throw BsvHomePlatformAccountError.AuthenticationUnavailable }
        let id = UUID()
        operationId = id
        return try await withTaskCancellationHandler {
            try await withCheckedThrowingContinuation { continuation in
                self.continuation = continuation
                let controller = ASAuthorizationController(authorizationRequests: [request])
                self.controller = controller
                controller.delegate = self
                controller.presentationContextProvider = self
                controller.performRequests()
            }
        } onCancel: {
            Task { @MainActor in
                guard self.operationId == id else { return }
                self.controller?.cancel()
                self.finish(.failure(BsvHomePlatformAccountError.Cancelled))
            }
        }
    }

    public nonisolated func verifyOfflineProof(publicKey: String, actionHash: String, signature: String) throws -> Bool {
        do {
            let key = try P256.Signing.PublicKey(derRepresentation: PlatformWebAuthnOptions.binary(publicKey))
            let signature = try P256.Signing.ECDSASignature(derRepresentation: PlatformWebAuthnOptions.binary(signature))
            return key.isValidSignature(signature, for: Data(actionHash.utf8))
        } catch { return false }
    }
}

extension BsvHomeCorePlatformAuthenticator: ASAuthorizationControllerDelegate {
    public func authorizationController(controller: ASAuthorizationController, didCompleteWithAuthorization authorization: ASAuthorization) {
        guard controller === self.controller else { return }
        finish(Result { try Self.response(authorization.credential) })
    }

    public func authorizationController(controller: ASAuthorizationController, didCompleteWithError error: Error) {
        guard controller === self.controller else { return }
        if let error = error as? ASAuthorizationError, error.code == .canceled {
            finish(.failure(BsvHomePlatformAccountError.Cancelled))
        } else {
            finish(.failure(BsvHomePlatformAccountError.AuthenticationUnavailable))
        }
    }
}

extension BsvHomeCorePlatformAuthenticator: ASAuthorizationControllerPresentationContextProviding {
    public func presentationAnchor(for _: ASAuthorizationController) -> ASPresentationAnchor {
        precondition(anchor != nil)
        return anchor!
    }
}
