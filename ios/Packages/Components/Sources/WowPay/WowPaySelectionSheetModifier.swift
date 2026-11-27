import SwiftUI

private struct WowPaySelectionSheetModifier<Sheet: View>: ViewModifier {
    @Binding var isPresented: Bool
    let sheet: () -> Sheet

    func body(content: Content) -> some View {
        content
            .fullScreenCover(isPresented: $isPresented) {
                sheet()
                    .presentationBackground(.clear)
                    .transaction { $0.disablesAnimations = false }
            }
            .transaction(value: isPresented) { $0.disablesAnimations = true }
    }
}

public extension View {
    func wowPaySelectionSheet(isPresented: Binding<Bool>, @ViewBuilder content: @escaping () -> some View) -> some View {
        modifier(WowPaySelectionSheetModifier(isPresented: isPresented, sheet: content))
    }
}
