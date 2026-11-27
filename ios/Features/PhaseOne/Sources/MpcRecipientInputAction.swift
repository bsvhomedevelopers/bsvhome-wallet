import struct BsvHomeCore.BsvHomePaymentRecipient

public typealias MpcRecipientInputAction = @MainActor (@escaping @MainActor @Sendable (BsvHomePaymentRecipient) -> Void) -> Void
