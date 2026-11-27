enum MpcWalletRoute: Hashable {
    case create, restore, wallet(String), backup(String), receive(String), send(String), transfer(String), history(String), paymail(String), recovery
}
