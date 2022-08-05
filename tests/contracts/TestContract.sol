// SPDX-License-Identifier: UNLICENSED
pragma solidity ^0.8.13;

contract TestContract {
    uint256 public i;

    function alwaysRevert() public {
        require(false, "`alwaysRevert` reverted");
        i++;
    }

    function increment() public {
        i++;
    }

    function set(uint256 _i) public {
        i = _i;
    }
}
