# Transaction Manager

The `tx-manager` is a rust library for robustly submitting transactions to the blockchain.

TODO: It is synchronous.

## Usage example
_The code for this example can be consulted at ???_

To start sending transactions you must first instantiate a `TransactionManager` object by calling its
constructor.

```
TransactionManager::new(provider, gas_oracle, database, chain, configuration)
```

The `chain` parameter is a `Chain` object that holds a chain ID and a boolean indicating whether or
not the target blockchain implements the EIP-1559. We can instantiate it by calling the `Chain::new`
function (or `Chain::legacy` for chains that don't implement the EIP-1559).

```
let chain = Chain::new(1337);
```

The `provider` is an object that implements the `ethers::providers::Middleware` trait.

In our examples, we will send transactions to a local geth node running on develop mode, hence, we
can instantiate a new provider as follows:

```
let provider = Provider::<Http>::try_from("http://localhost:8545").unwrap();
```

The provider is responsible for signing the transactions we will be sending, therefore, we wrap it
using a signer middleware.

```
let wallet = LocalWallet::new(&mut thread_rng()).with_chain_id(chain.id);
let provider = SignerMiddleware::new(provider, wallet);
```

The `gas_oracle` and `database` parameters are dependencies injected into the transaction manager
to, respectivelly, deal with gas prices and guarantee robustness. The `configuration` is used for
for fine tuning waiting times internally. We will discuss them in length in the next sections but,
for now, we will use their default provided implementations.

```
let gas_oracle = DefaultGasOracle::new();
let database = FileSystemDatabase::new("database.json".to_string());
let configuration = Configuration::default();
```

We can, then, instantiate the transaction manager:

```
let (manager, receipt) =
    TransactionManager::new(provider, gas_oracle, database, chain, configuration)
        .await
        .unwrap();

assert!(receipt.is_none());
```

Note that the `new` function is asynchronous and returns both a transaction manager and a
`Option<ethers::types::TransactionReceipt>`.
We designed the transaction manager to be robust.
In case the manager is interrupted while sending a transaction (a hardware crash, for example),
it will try to confirm that transaction during its next instantiation, thus, the possible receipt
and the need for `async`.

With a manager in hands we can send a transaction by calling the aptly named `send_transaction`
method.

```
pub async fn send_transaction(
    mut self,
    transaction: Transaction,
    confirmations: usize,
    priority: Priority,
) -> Result<(Self, TransactionReceipt), Error<M, GO, DB>> {
```

The `send_transaction` method takes a `mut self` transaction manager, effectivelly taking ownership
of the manager instance.
When the function is done, it returns that instance alongside the expected transaction receipt.
This implies and enforces (through the type system) that (1) we will need to instantiate a new
manager in case the function fails and (2) we can only send transactions sequentially and never
concurrently. 

```
let transaction = Transaction {
    from: wallet.address(),
    to: H160::random(),
    value: Value::Number(U256::from(1e9 as u64)),
    call_data: None,
};

let result = manager
    .send_transaction(transaction, 1, Priority::Normal)
    .await;
assert!(result.is_err());
```

In our contrived example, we are sending funds from a random wallet, so it is pretty clear that
that transaction will fail with a "out of funds" error.


## TODO

To submit a transaction through the manager, we must first The strategy is used to determine the gas price. The manager will constantly monitor the blockchain, and will resubmit transactions if

If a transaction is dropped from the transaction pool, it will be resubmitted eventually by the manager. Nonces are also managed automatically. When a transaction is first submitted, the manager will allocate the correct nonce to the transaction, reusing the nonce when re-submitting the transaction.
