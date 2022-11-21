# Transaction Manager

The `tx-manager` is a Rust library for submitting transactions to the
blockchain.
It tries to account for the many scenarios that can befall a transaction sent
to the transaction pool and adjust accordingly.

Most notably, it:

- Manages nonces automatically.
- Resends transactions if necessary.
- Waits for a given number of block confirmations.
- Recovers from hard crashes.

## Usage example
_The code for this example is available at tests/doc_test.rs. You can run it
with the `cargo run --example send_transaction` command_

To start sending transactions we must first instantiate a `TransactionManager`
object by calling its constructor.

```
TransactionManager::new(provider, gas_oracle, database, chain, configuration)
```

The `chain` parameter is a `Chain` object that holds a chain ID and a boolean
indicating whether or not the target blockchain implements the EIP-1559.
We can instantiate it by calling the `Chain::new` or `Chain::legacy` functions.

```
let chain = Chain::new(1337);
```

The `provider` is an object that implements the `ethers::providers::Middleware`
trait.
(In our examples, we will send transactions to a local geth node running on
develop mode.)

```
let provider = Provider::<Http>::try_from("http://localhost:8545").unwrap();
```

The provider is responsible for signing the transactions we will be sending,
therefore, we wrap it using a signer middleware.

```
let wallet = LocalWallet::new(&mut thread_rng()).with_chain_id(chain.id);
let provider = SignerMiddleware::new(provider, wallet);
```

The `gas_oracle` and `database` parameters are dependencies injected into the
transaction manager to, respectivelly, deal with gas prices and guarantee
robustness.
The `configuration` is used for fine tuning internal waiting times.
We will discuss these in the next sections but, for now, we will use their
default provided implementations.

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

Note that the `new` function is asynchronous and returns both a transaction
manager and a `Option<ethers::types::TransactionReceipt>`.
We designed the transaction manager to be robust.
In case the manager fails or gets interrupted while sending a transaction (a
hardware crash, for example), it will try to confirm that transaction during
its next instantiation, thus, the possible receipt and the need for `async`.
In essence, this guarantees that we always deal with pending transactions.

With the manager in hands we can send a transaction by calling the aptly named
`send_transaction` method.

```
pub async fn send_transaction(
    mut self,
    transaction: Transaction,
    confirmations: usize,
    priority: Priority,
) -> Result<(Self, TransactionReceipt), Error<M, GO, DB>> {
```

The `send_transaction` method takes a `mut self` transaction manager,
effectivelly taking ownership of the manager instance.
When the function is done, it returns that instance alongside the expected
transaction receipt.
This enforces through the type system that (1) we will need to instantiate a new
manager in case the function fails and (2) we can only send transactions
sequentially. 
Property 1 guarantees that any pending transactions will be managed by the
constructor and property 2 enforces synchronicity (concurrency leads to a lot
of undesired complexity).

The function also takes a `confirmations` argument.
The number of confirmations is the number of blocks that must be mined, after
the transaction is placed in a block, so that the function returns successfully.
In other words, if the number of confirmations is 0, the function returns
immediatelly after the transaction gets mined, otherwise, it will wait for
`confirmations` more blocks to be mined before returning.
(This basically accounts for network reorganizations.)

The `priority` level is a parameter sent to the gas oracle to calculate the
appropriate gas fees. Higher priorities cost more, but reduce waiting times,
and vice-versa for lower priorities.

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

In our contrived example, we are sending funds from a random wallet, so it is
pretty clear that that transaction will fail with an "insufficient funds" error.

## Gas Oracle 

TODO.

## Database 

TODO.

## Configuration 

TODO.

## Inner workings

TODO.

