import Style
import SwiftUI

public struct WowPayPaymailForm: View {
    private let title: String
    private let description: String
    private let fieldTitle: String
    private let placeholder: String
    private let saveTitle: String
    private let doneTitle: String
    private let isSaving: Bool
    private let isEnabled: Bool
    private let onSave: () -> Void
    @Binding private var name: String
    @FocusState private var isFocused: Bool

    public init(title: String, description: String, fieldTitle: String, placeholder: String, saveTitle: String, doneTitle: String, name: Binding<String>, isSaving: Bool = false, isEnabled: Bool = true, onSave: @escaping () -> Void) {
        self.title = title
        self.description = description
        self.fieldTitle = fieldTitle
        self.placeholder = placeholder
        self.saveTitle = saveTitle
        self.doneTitle = doneTitle
        self.isSaving = isSaving
        self.isEnabled = isEnabled
        _name = name
        self.onSave = onSave
    }

    public var body: some View {
        VStack(alignment: .leading, spacing: Spacing.wowPay.card) {
            VStack(alignment: .leading, spacing: .small) {
                Text(title).font(.wowPay.cardTitle)
                    .foregroundStyle(Colors.WowPay.heading)
                    .accessibilityAddTraits(.isHeader)
                Text(description).font(.wowPay.caption)
                    .foregroundStyle(Colors.WowPay.secondaryText)
            }
            VStack(alignment: .leading, spacing: Spacing.wowPay.grid) {
                Text(fieldTitle).font(.wowPay.label)
                    .foregroundStyle(Colors.WowPay.heading)
                HStack(spacing: Spacing.wowPay.grid) {
                    Image("wowpay-paymail").renderingMode(.template).resizable().scaledToFit()
                        .frame(width: Sizing.wowPay.icon, height: Sizing.wowPay.icon)
                        .foregroundStyle(Colors.WowPay.brand)
                        .accessibilityHidden(true)
                    TextField(fieldTitle, text: $name, prompt: Text(placeholder).foregroundStyle(Colors.WowPay.inputPlaceholder))
                        .textInputAutocapitalization(.never)
                        .autocorrectionDisabled()
                        .keyboardType(.emailAddress)
                        .submitLabel(.done)
                        .focused($isFocused)
                        .onSubmit { isFocused = false }
                        .accessibilityIdentifier("wowPayPaymailInput")
                        .disabled(isSaving)
                }
                .wowPayInput()
            }
            StateButton(text: saveTitle, type: .wowPayPrimary(isSaving ? .loading() : isEnabled ? .normal : .disabled)) {
                isFocused = false
                onSave()
            }
            .accessibilityIdentifier("wowPaySavePaymail")
        }
        .wowPayCard()
        .toolbar {
            ToolbarItemGroup(placement: .keyboard) {
                Spacer()
                Button(doneTitle) { isFocused = false }
            }
        }
    }
}
