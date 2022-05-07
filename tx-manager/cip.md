---
CIP: <to be assigned>
title: The (New) Transaction Manager
author: Renan Santos (@renan061)
discussions-to: <URL>
status: Draft
type: Meta
created: 2022-04-04
---

## Abstract

TODO : something like this. talk about read and write?

Conceptually, applications interact with the blockchain by either updating its state or analyzing the information within it.
In truth, applications alter the state of the blockchain by sending transactions and waiting for them to be mined.
Likewise, the proccess of retrieving information from the blockchain requires the application to condense data and the flow of transactions into a state bound by the block number.
The transaction manager is a tool for sending transactions that greatly simplifies development.

<! TODO : These two primitive operations represent the core interface used by applications to !>

## Motivation
- Sending transactions to the blockchain and verifying they were mined is not a trivial task.
- Still, it is one of its the two primitive operations. 
- There are inumerous pitfalls and corner cases to worry about, ranging from... 
- There is a lack in mature tooling that simplifies this task for the programmer.
- Our current implementation is async, we have decided to make a simpler library with blocking functions.


## Specification
- list the functions.
- talk about the injected code (the gas oracle and the database).
- show the flow diagram.
- etc.

## Rationale
- sync not async.
- why we did not use labels? reference the issue.

The rationale fleshes out the specification by describing what motivated the design and why particular design decisions were made. It should describe alternate designs that were considered and related work. The rationale may also provide evidence of consensus within the community, and should discuss important objections or concerns raised during discussion.

## Backwards Compatibility
- problems with the previous transaction manager?

All CIPs that introduce backwards incompatibilities must include a section describing these incompatibilities and their severity. The CIP must explain how the author proposes to deal with these incompatibilities. CIP submissions without a sufficient backwards compatibility treatise may be rejected outright.

## Test Cases
This section is optional. Tests should either be inlined in the CIP as data (such as input/expected output pairs, or included in `../assets/cip-###/<filename>`.

## Security Considerations
All CIPs must contain a section that discusses the security implications/considerations relevant to the proposed change. Include information that might be important for security discussions, surfaces risks, and can be used throughout the life-cycle of the proposal. E.g. include security-relevant design decisions, concerns, important discussions, implementation-specific guidance and pitfalls, an outline of threats and risks and how they are being addressed. CIP submissions missing the "Security Considerations" section will be rejected. A CIP cannot proceed to status "Final" without a Security Considerations discussion deemed sufficient by the reviewers.

## Copyright
Copyright and related rights waived via [CC0](https://creativecommons.org/publicdomain/zero/1.0/).
