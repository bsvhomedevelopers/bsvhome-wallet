import Localization
import Style
import SwiftUI

public struct BsvHdImportReview: View {
    private let accountPath: String
    private let firstAddress: String
    @Binding private var isConfirmed: Bool

    public init(accountPath: String, firstAddress: String, isConfirmed: Binding<Bool>) {
        self.accountPath = accountPath
        self.firstAddress = firstAddress
        _isConfirmed = isConfirmed
    }

    public var body: some View {
        VStack(alignment: .leading, spacing: Spacing.medium) {
            Text(Localized.bsvHdCompareHint)
            Text(Localized.bsvHdAccountPath).font(.headline)
            Text(accountPath).fixedSize(horizontal: false, vertical: true)
            Text(firstAddress).font(.body.monospaced()).fixedSize(horizontal: false, vertical: true)
            Toggle(Localized.bsvHdCompareConfirm, isOn: $isConfirmed)
        }
    }
}
