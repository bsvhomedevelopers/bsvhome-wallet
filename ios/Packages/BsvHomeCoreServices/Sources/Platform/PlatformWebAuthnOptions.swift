import AuthenticationServices
import Foundation
import enum BsvHomeCore.BsvHomePlatformAccountError
import enum BsvHomeCore.BsvHomePlatformPasskeyOperation

struct PlatformWebAuthnOptions: Decodable {
    struct RelyingParty: Decodable { let id: String }
    struct User: Decodable {
        let id: String
        let name: String
        let displayName: String?
    }

    struct Credential: Decodable {
        let id: String
        let type: String
    }

    struct Parameter: Decodable {
        let type: String
        let alg: Int
    }

    struct Selection: Decodable {
        let authenticatorAttachment: String?
        let userVerification: String?
        let residentKey: String?
    }

    let challenge: String
    let rp: RelyingParty?
    let rpId: String?
    let user: User?
    let pubKeyCredParams: [Parameter]?
    let excludeCredentials: [Credential]?
    let allowCredentials: [Credential]?
    let authenticatorSelection: Selection?
    let userVerification: String?
    let attestation: String?

    static func decode(_ json: String) throws -> Self {
        let data = Data(json.utf8)
        guard data.count <= 262_144,
              let root = try JSONSerialization.jsonObject(with: data) as? [String: Any]
        else { throw BsvHomePlatformAccountError.InvalidResponse }
        let object = root["publicKey"] as? [String: Any] ?? root
        if let extensions = object["extensions"] {
            guard let extensions = extensions as? [String: Any] else { throw BsvHomePlatformAccountError.InvalidResponse }
            guard extensions.isEmpty else { throw BsvHomePlatformAccountError.AuthenticationUnavailable }
        }
        return try JSONDecoder().decode(Self.self, from: JSONSerialization.data(withJSONObject: object))
    }

    @MainActor
    func request(operation: BsvHomePlatformPasskeyOperation) throws -> ASAuthorizationRequest {
        let identifier = operation == .register ? rp?.id : rpId
        guard let identifier, !identifier.isEmpty, identifier == identifier.lowercased(),
              !identifier.contains(":"), !identifier.contains("/"), !identifier.contains("@")
        else { throw BsvHomePlatformAccountError.InvalidResponse }
        let provider = ASAuthorizationPlatformPublicKeyCredentialProvider(relyingPartyIdentifier: identifier)
        let challenge = try Self.binary(challenge)
        if operation == .register {
            guard let user, !user.name.isEmpty,
                  pubKeyCredParams?.contains(where: { $0.type == "public-key" && $0.alg == -7 }) == true,
                  [nil, "platform"].contains(authenticatorSelection?.authenticatorAttachment),
                  [nil, "required", "preferred", "discouraged"].contains(authenticatorSelection?.residentKey)
            else { throw BsvHomePlatformAccountError.AuthenticationUnavailable }
            let request = try provider.createCredentialRegistrationRequest(challenge: challenge, name: user.name, userID: Self.binary(user.id))
            request.displayName = user.displayName
            request.userVerificationPreference = try verification(authenticatorSelection?.userVerification)
            switch attestation ?? "none" {
            case "none": request.attestationPreference = .none
            case "direct": request.attestationPreference = .direct
            case "indirect": request.attestationPreference = .indirect
            default: throw BsvHomePlatformAccountError.AuthenticationUnavailable
            }
            let excluded = try descriptors(excludeCredentials ?? [])
            if #available(iOS 17.4, macOS 14.4, *) {
                request.excludedCredentials = excluded
            } else if !excluded.isEmpty {
                throw BsvHomePlatformAccountError.AuthenticationUnavailable
            }
            return request
        }
        let request = provider.createCredentialAssertionRequest(challenge: challenge)
        request.userVerificationPreference = try verification(userVerification)
        request.allowedCredentials = try descriptors(allowCredentials ?? [])
        return request
    }

    private func verification(_ value: String?) throws -> ASAuthorizationPublicKeyCredentialUserVerificationPreference {
        switch value ?? "required" {
        case "required": .required
        default: throw BsvHomePlatformAccountError.InvalidResponse
        }
    }

    private func descriptors(_ values: [Credential]) throws -> [ASAuthorizationPlatformPublicKeyCredentialDescriptor] {
        try values.map {
            guard $0.type == "public-key" else { throw BsvHomePlatformAccountError.InvalidResponse }
            return try ASAuthorizationPlatformPublicKeyCredentialDescriptor(credentialID: Self.binary($0.id))
        }
    }

    static func binary(_ text: String) throws -> Data {
        guard !text.isEmpty, text.utf8.allSatisfy({ (65 ... 90).contains($0) || (97 ... 122).contains($0) || (48 ... 57).contains($0) || $0 == 45 || $0 == 95 }),
              text.count % 4 != 1,
              let data = Data(base64Encoded: text.replacingOccurrences(of: "-", with: "+").replacingOccurrences(of: "_", with: "/") + String(repeating: "=", count: (4 - text.count % 4) % 4)),
              encoded(data) == text
        else { throw BsvHomePlatformAccountError.InvalidResponse }
        return data
    }

    static func encoded(_ data: Data) -> String {
        data.base64EncodedString().replacingOccurrences(of: "+", with: "-").replacingOccurrences(of: "/", with: "_").replacingOccurrences(of: "=", with: "")
    }
}
