# Transaction Manager


At Cartesi, we run into issues related to a lack of mature tooling every day.
Blockchain is a nascent technology, and the software stack is still in its early days and evolving at a fast pace.
Writing _ad-hoc_ solutions to these issues is not scalable.
A better approach is building abstractions on top of established lower-level solutions in a way that they can be reused.

It must be noted that these lower-level solutions are open source, developed by the community.
In the same way we’ve benefited from those, we are publishing our own in-house tools that may benefit the community.

Previously, we talked about [reading the state of smart contracts with the State Fold](https://medium.com/cartesi/state-fold-cfe5f4d79639),
the first step in interacting with the blockchain.
The Transaction Manager is the next step.
This tool addresses the issue of sending transactions to the blockchain.
It is, in many ways, the dual of the State Fold: the State Fold reads the state of the ledger, and the Transaction Manager writes to the state of the ledger.

## Background

Transactions move the blockchain state forward.
They are the way users outside the blockchain enact state changes to the ledger.
Common types of transactions include transferring Eether to other addresses, invoking smart contracts’ methods (such as an ERC20 token transfer) and instantiating smart contracts.
In the context of Cartesi rollups, examples of transactions are adding inputs to the rollups, making state hash claims, and interacting with fraud-proof disputes.

Transactions are sent by user accounts, also known as externally owned accounts.
Transactions are first signed by the user, using the account’s private key, and then broadcasted to the network to be included in the ledger at a future block.
They contain information such as the “to” address (which can be another user account or a smart contract), the payload (containing data we want to send to smart contracts), gas price, and the value we want to transfer in ethers.

From the miners’ side of things, transactions are first added to what’s called the transaction pool.
The transaction pool is a structure local to each Ethereum node.
It holds the set of transactions miners draw from when they are creating new blocks.
Think of them as a queue of transactions to be included in the blockchain.
However, as this queue is local to each node, its ordering is decided by each individual miner.

Generally speaking, miners first choose the transactions that will yield the highest profit: the order in which the transactions are added to the blockchain does not correspond to the ordering in which they were added to the pool.
A reasonable heuristic for the profitability of adding transactions to the blockchain is the transaction’s gas price.
As such, transactions with a higher gas price are usually added to the ledger first.

Transactions sent to the blockchain from a user account must be uniquely numbered, in ascending order, without gaps.
This numbering is called the _nonce_ and is part of the transaction.
Transactions with duplicated nonces are mutually exclusive.
If the transaction pool contains two transactions with the same nonce, the miner can only include one of them to the blockchain (generally the one with the higher gas price).

In practice, when we want to send a transaction, we do it through Ethereum’s JSON-RPC API, which is usually wrapped in some high-level library like `web3.js`.
This library communicates with a remote provider, which we must specify before using it.
Trust in this provider is imperative: a malicious provider could simply ignore our requests and refuse to broadcast our transactions.
However, it cannot forge or modify our transactions: the cryptographic signature makes sure of that.

Since the provider must communicate with the Ethereum network, it must run an Ethereum node.
There are generally two ways of setting this provider up.
The first one is doing it ourselves.
We choose an implementation of the Ethereum protocol, such as Geth or Parity, and run it in some machine.
This setup is not trivial.
The second way is delegating this to some external provider, such as Infura or Alchemy.

# Sending Transactions

Making sure a transaction gets added to the blockchain is easier said than done.
There are multiple pitfalls and obstacles to managing transactions, which must be addressed if we wish to create a robust application.


The first step is to build the transaction data itself, before signing it and sending it to an Ethereum node.
Part of the transaction data is fixed, in the sense that we may not arbitrarily change it without altering the effects of a transaction.
Examples of those are the `to` address, payload and Ether value.
They are intrinsic to the transaction and are a part of the “application logic”, separate from the “submit transaction logic”.

There are parameters of the transaction data that are extrinsic, in the sense that they (generally) do not change the effects of the transaction.
Examples of those are the nonce and the gas price.
They are not part of the application logic and are, in a sense, purely bureaucratic.

Nevertheless, they must be carefully chosen to ensure our transitions are added to the blockchain: a wrong nonce will cause the transaction never to be added, and specifying a low gas price will clog the user account.
Even then, gas prices are not constant and are subject to wild fluctuations.
Submitting a transaction with a seemingly reasonable gas price may still clog the user account if the gas price spikes, and specifying a gas price that is too high will waste money.

Discovering the correct nonce in a robust way is not as straightforward as it may initially appear.
The canonical way of getting the nonce is counting how many transactions by that user account have been added to the blockchain.
This works well if the user sends transactions infrequently, always waiting for the previous transaction to be mined before sending the next one.
Otherwise, we may run into duplicate nonces: the transaction pool may contain further transactions that are not counted.

Making things worse, transactions may be dropped from the transaction pool arbitrarily.
Along with possibly missing an important reaction, a dropped transaction may poison the user account: if there are gaps in the nonce numbering, the account will not be able to include further transactions until the gap is filled.

It is clear that we cannot just submit a transaction and expect it to be added to the ledger.
There must be some bookkeeping logic between the application and the provider if we want to write a robust application.
This bookkeeping logic also has to be resilient to process restarts: we must be able to turn on our application with a clean slate and have it function properly.


# Transaction Manager

The Transaction Manager is our solution for robustly submitting transactions to the blockchain.
It is available as a Rust library (a crate, in Rust parlance), and handles all this bookkeeping logic.

To submit a transaction through the manager, we must first specify the intrinsic transaction data and a submission strategy.
The strategy is used to determine the gas price.
The manager will constantly monitor the blockchain and gas prices, and will resubmit transactions with outdated prices following the submission strategy.

If a transaction is dropped from the transaction pool, it will be resubmitted automatically by the manager.
Nonces are also managed automatically.
When a transaction is first submitted, the manager will allocate the correct nonce to the transaction, reusing the nonce when re-submitting the transaction.
If a transaction is submitted with a higher strategy than previous transactions, the manager will promote the old transactions to the newer strategy.
This is done to make sure old underpriced transactions will not clog the user account.
All of this is handled concurrently and does not block the main application.

Our manager can also attempt to cancel sent transactions.
Canceling transactions is not a guaranteed thing.
Once committed to the blockchain, transactions cannot be revoked.
However, while they are still in the transaction pool, we may attempt to override them.
The trick consists of submitting a new transaction which does nothing (like transferring zero ether to oneself), but with the same nonce as the transaction we wish to cancel, and with a higher gas price.
This way, there’s a chance miners will include the transaction with the higher gas price instead of the original one, effectively canceling the original transaction.
The user still has to pay the base cost of a transaction.

We are releasing the first version of our Transaction Manager.
There are multiple improvements in the pipeline that we are eager to share with the community when they are ready, including adjustments related to EIP-1559.
The Transaction Manager is our solution for robustly submitting transactions to the blockchain.
It is available as a Rust library (a crate, in Rust parlance), and handles all this bookkeeping logic.

To submit a transaction through the manager, we must first specify the intrinsic transaction data and a submission strategy.
The strategy is used to determine the gas price.
The manager will constantly monitor the blockchain and gas prices, and will resubmit transactions with outdated prices following the submission strategy.

If a transaction is dropped from the transaction pool, it will be resubmitted automatically by the manager.
Nonces are also managed automatically.
When a transaction is first submitted, the manager will allocate the correct nonce to the transaction, reusing the nonce when re-submitting the transaction.
If a transaction is submitted with a higher strategy than previous transactions, the manager will promote the old transactions to the newer strategy.
This is done to make sure old underpriced transactions will not clog the user account.
All of this is handled concurrently and does not block the main application.

Our manager can also attempt to cancel sent transactions.
Canceling transactions is not a guaranteed thing.
Once committed to the blockchain, transactions cannot be revoked.
However, while they are still in the transaction pool, we may attempt to override them.
The trick consists of submitting a new transaction which does nothing (like transferring zero ether to oneself), but with the same nonce as the transaction we wish to cancel, and with a higher gas price.
This way, there’s a chance miners will include the transaction with the higher gas price instead of the original one, effectively canceling the original transaction.
The user still has to pay the base cost of a transaction.

We are releasing the first version of our Transaction Manager.
There are multiple improvements in the pipeline that we are eager to share with the community when they are ready, including adjustments related to EIP-1559.
