use ethers::types::{Address, Signature, TxHash, U256};

pub struct Data {
    pub address: [Address; 4],
    pub transaction_hash: [TxHash; 4],
    pub signature: [Signature; 4],
}

impl Data {
    pub fn new() -> Data {
        let address = [
            "0xba763b97851b653aaaf631723bab41a500f03b29",
            "0x29e425df042e83e4ddb3ee3348d6d745c58fce8f",
            "0x905f3bd1bd9cd23be618454e58ab9e4a104909a9",
            "0x7e2d4b75bbf489e691f8a1f7e5f2f1148e15feed",
        ]
        .map(|s| s.parse().unwrap());

        let transaction_hash = [
            "0x2b34df791cc4eb898f6d4437713e946f216cac6a3921b2899db919abe26739b2",
            "0x4eb76dd4a6f6d37212f3b26da6a026c30a92700cdf560f81b14bc42c2cffb218",
            "0x08bd64232916289006f3de2c1cad8e5afa6eabcf4efff219721b87ee6f9084ec",
            "0xffff364da9e2b4bca9199197c220ae334174527c12180f9d667005a887ff2fd6",
        ]
        .map(|s| s.parse().unwrap());

        let signature = [
            Signature {
                r: u256(1),
                s: u256(1),
                v: 1,
            },
            Signature {
                r: u256(2),
                s: u256(2),
                v: 2,
            },
            Signature {
                r: u256(3),
                s: u256(3),
                v: 3,
            },
            Signature {
                r: u256(4),
                s: u256(4),
                v: 4,
            },
        ];

        Data {
            address,
            transaction_hash,
            signature,
        }
    }
}

fn u256(n: u32) -> U256 {
    U256::from_dec_str(&n.to_string()).unwrap()
}
