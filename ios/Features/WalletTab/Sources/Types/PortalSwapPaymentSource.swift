import Foundation

public enum PortalSwapPaymentSource: Equatable {
    case tab
    case detail(UUID)
    case acquisition(UUID)
}
