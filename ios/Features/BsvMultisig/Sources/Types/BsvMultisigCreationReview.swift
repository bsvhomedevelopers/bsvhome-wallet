import struct BsvHomeCore.BsvHomeBsvMultisigCreationDraft
import struct BsvHomeCore.BsvHomeBsvMultisigSignerBinding

public struct BsvMultisigCreationReview: Hashable, Sendable {
    let draft: BsvHomeBsvMultisigCreationDraft
    var name: String { draft.name }
    var threshold: UInt32 { draft.threshold }
    var memberNames: [String] { draft.memberNames }
    var identity: BsvHomeBsvMultisigSignerBinding { draft.signingIdentity }
}
