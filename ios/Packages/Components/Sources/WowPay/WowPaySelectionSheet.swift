import Style
import SwiftUI

public struct WowPaySelectionSheet<Item: Identifiable, Row: View>: View {
    private let title: String
    private let closeTitle: String
    private let closeImage: Image
    private let items: [Item]
    private let selectedID: Item.ID?
    private let onSelect: (Item) -> Void
    private let row: (Item) -> Row

    public init(
        title: String,
        closeTitle: String,
        closeImage: Image,
        items: [Item],
        selectedID: Item.ID?,
        onSelect: @escaping (Item) -> Void,
        @ViewBuilder row: @escaping (Item) -> Row,
    ) {
        self.title = title
        self.closeTitle = closeTitle
        self.closeImage = closeImage
        self.items = items
        self.selectedID = selectedID
        self.onSelect = onSelect
        self.row = row
    }

    public var body: some View {
        WowPayBottomPanel(title: title, closeTitle: closeTitle, closeImage: closeImage) { close in
            VStack(spacing: Spacing.wowPay.grid) {
                ForEach(items) { item in
                    Button {
                        onSelect(item)
                        close()
                    } label: {
                        row(item)
                            .frame(maxWidth: .infinity, alignment: .leading)
                            .contentShape(Rectangle())
                    }
                    .accessibilityAddTraits(selectedID == item.id ? .isSelected : [])
                }
            }
        }
    }
}
