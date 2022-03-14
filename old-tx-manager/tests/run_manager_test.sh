# Start geth
echo "Starting geth"
geth --dev --dev.period 1 --allow-insecure-unlock --http --http.api personal,eth,web3 --ws --ws.api personal,eth,web3 --txpool.pricelimit 20000000000  >>/dev/null 2>&1 &
pid=$!
sleep 3
echo "Started geth with pid $pid"

# Run tests flagged with ignore
cargo test -p transaction_manager --test manager_test -- --nocapture --ignored --test-threads 3

# kill geth
echo "Killing geth, pid $pid"
kill "$pid"
sleep 5
