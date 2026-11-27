import Formatters
import Primitives
import PrimitivesComponents
import PrimitivesTestKit
import Testing

struct FiatTransactionViewModelTests {
    @Test(arguments: ["CNY", "EUR", "USD"])
    func keepsOrderCurrencyAndTokenAmount(currency: String) {
        let order = FiatTransactionAssetData.mock(fiatAmount: 720, fiatCurrency: currency, value: "100000000")
        let model = FiatTransactionViewModel(info: order)

        #expect(model.listItemModel.subtitleExtra == CurrencyFormatter(currencyCode: currency).string(720))
        #expect(model.listItemModel.subtitle == "1.0000 BTC")
        if currency != "USD" {
            #expect(model.listItemModel.subtitleExtra != CurrencyFormatter(currencyCode: "USD").string(720))
        }
    }
}
