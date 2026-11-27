# Deep links

A deep link opens a screen inside the app. BsvHome uses the `bsvhome://` app scheme. Core also accepts legacy `gem://` links. Links to `https://gemwallet.com/` reach an installed app when the path is included in the platform's verified-link configuration.

## Supported links

An asset is written as `{chain}` for a coin and `{chain}/{token_id}` for a token. A screen for that asset is an action appended to its path.

| Screen | Path | Parameters | Verified HTTPS app-link coverage |
|---|---|---|---|
| Asset | `/tokens/{asset}` | — | iOS and Android |
| Receive | `/tokens/{asset}/receive` | — | iOS and Android |
| Buy | `/tokens/{asset}/buy?amount={fiat}` | Amount is optional | iOS and Android |
| Sell | `/tokens/{asset}/sell?amount={fiat}` | Amount is optional | iOS and Android |
| Swap | `/tokens/{asset}/swap` | Opens swap with the asset to pay from | iOS and Android |
| Perpetuals | `/perpetuals` | — | Android only; iOS association is outstanding |
| Rewards | `/rewards?code={code}`, `/join/{code}` | Referral code is optional | Use `/join/{code}` for iOS and Android; `/rewards` is app-scheme only |

Examples:

```
bsvhome://tokens/bitcoin/receive
bsvhome://tokens/bitcoin/buy?amount=100
bsvhome://tokens/ethereum/0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48/sell?amount=49
bsvhome://tokens/solana/swap
https://gemwallet.com/tokens/solana/buy?amount=25
```

`amount` is a fiat amount in whole USD, not a crypto amount. Values that are not a positive whole number, including fractional ones like `49.5`, are ignored and the screen opens with its default amount. A locale segment in front of the path is accepted and skipped, so `https://gemwallet.com/zh-cn/tokens/bitcoin/buy` resolves like `https://gemwallet.com/tokens/bitcoin/buy`. Every action requires an asset. A link with an unknown path, an unknown chain, an unknown action, or a missing asset is not a deep link and opens in the browser.

Sell availability controls whether Sell can be selected in the screen. A direct sell link still opens Sell when the asset is not marked as sellable.

## Links in the support chat

Core parses support chat messages and displays deep links as rows that open in the app. Chat agents can use a Markdown link with a `bsvhome://` or `https://gemwallet.com/` URL.

```markdown
[Buy Bitcoin](bsvhome://tokens/bitcoin/buy?amount=100)
```

## Implementation

- [Deep link parsing and building](../core/crates/primitives/src/deeplink.rs)
- [URL action routing](../core/crates/primitives/src/url_action.rs) — WalletConnect and deep links are routed before payments
- [UniFFI bridge](../core/bsv_home_core/src/deeplink.rs)
- [Support message links](../core/crates/support/src/text.rs)
- iOS: [`NavigationHandler`](../ios/BsvHome/Navigation/NavigationHandler.swift), schemes and associated domains in `ios/BsvHome/Resources/Info.plist` and `ios/BsvHome/Resources/BsvHome.entitlements`
- Android: [`WebDeepLinks`](../android/app/src/main/kotlin/com/gemwallet/android/WebDeepLinks.kt), intent filters in `android/app/src/main/AndroidManifest.xml`

## Web requirements

`https://gemwallet.com/` links only reach the app when the app declares them and the website publishes the matching association. They should also have a browser fallback for people without the app.

- `https://gemwallet.com/.well-known/apple-app-site-association` currently lists token and join paths; `/perpetuals` and `/rewards` are not iOS Universal Links
- `https://gemwallet.com/.well-known/assetlinks.json` must list the Android package and signing certificate
- Android currently declares token, join, and perpetual paths; it does not declare `/rewards`
- As checked on 2026-09-02, token action URLs such as `/tokens/bitcoin/buy` return `404` in a browser. Publishing a non-app fallback remains required
