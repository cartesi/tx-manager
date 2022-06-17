rm -f test_database.json && clear && cargo test geth

geth --dev --dev.period 10 --http --http.api personal,debug,admin,net,txpool,eth,web3 --ws --ws.api personal,debug,admin,net,txpool,eth,web3

personal.newAccount
eth.coinbase
eth.accounts
eth.getBalance(eth.accounts[1])/1e18

personal.importRawKey("8da4ef21b864d2cc526dbdb2a120bd2874c36c9d0a1fb7f8c63d7f7a8b41de8f", "")

personal.sendTransaction({from: eth.coinbase, to: "0x63fac9201494f0bd17b9892b9fae4d52fe3bd377", value: web3.toWei(100, "ether")}, "")