rm -f test_database.json && clear && cargo test geth

geth --dev --dev.period 10 --allow-insecure-unlock --http --http.api personal,debug,admin,net,txpool,eth,web3 --ws --ws.api personal,debug,admin,net,txpool,eth,web3

personal.newAccount
eth.coinbase
eth.accounts
eth.getBalance(eth.accounts[1])/1e18


