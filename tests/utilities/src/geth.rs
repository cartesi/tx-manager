use std::{
    io::{BufRead, BufReader},
    process::Command,
    time::{Duration, Instant},
};

use eth_tx_manager::Chain;

use crate::{Account, ProviderWrapper};

/// The geth command.
const GETH: &str = "geth";

pub struct Geth_ {
    url: String,
    process: std::process::Child,
}

impl Geth_ {
    pub fn start(port: u16, block_time: u16) -> Geth_ {
        let mut cmd = Command::new(GETH);

        // Using stderr for logs.
        cmd.stderr(std::process::Stdio::piped());

        // Opening the HTTP API.
        cmd.arg("--http");
        cmd.arg("--http.port").arg(port.to_string());
        cmd.arg("--http.api")
            .arg("eth,net,web3,txpool,personal,debug");

        // Dev mode with custom block times.
        cmd.arg("--dev");
        cmd.arg("--dev.period").arg(block_time.to_string());

        let mut child = cmd.spawn().expect("Could not start geth.");
        let stdout = child
            .stderr
            .expect("Unable to get stderr for geth child process.");

        let start = Instant::now();
        let mut reader = BufReader::new(stdout);

        /// How long we will wait for geth to indicate that it is ready.
        const GETH_STARTUP_TIMEOUT: u64 = 10;
        loop {
            let timeout = Duration::from_secs(GETH_STARTUP_TIMEOUT);
            if start + timeout <= Instant::now() {
                panic!("Timed out waiting for geth to start. Is geth installed?")
            }

            let mut line = String::new();
            reader
                .read_line(&mut line)
                .expect("Failed to read line from geth process.");

            // Geth 1.9.23 uses "server started" while 1.9.18 uses "endpoint opened".
            if line.contains("HTTP endpoint opened") || line.contains("HTTP server started") {
                break;
            }
        }

        child.stderr = Some(reader.into_inner());

        let url = format!("http://localhost:{}", port);
        Geth_ {
            url: url.clone(),
            process: child,
        }
    }
}

impl Drop for Geth_ {
    fn drop(&mut self) {
        self.process.kill().expect("could not kill geth");
    }
}

pub struct Geth {
    pub geth_: Geth_,
    pub provider: ProviderWrapper,
}

impl Geth {
    pub fn start(port: u16, block_time: u16, chain: Chain, signer: &Account) -> Geth {
        let geth_ = Geth_::start(port, block_time);
        let url = geth_.url.clone();
        Geth {
            geth_,
            provider: ProviderWrapper::new(url, chain, &signer),
        }
    }

    pub async fn give_funds(&self, to: &Account, gwei: u64) {
        let balance = self.provider.get_balance_in_gwei(to).await;
        let mut instruction: String = "personal.sendTransaction(".to_owned();
        instruction.push_str("{from: eth.coinbase, to: \"");
        instruction.push_str(&to.address);
        instruction.push_str("\", value: web3.toWei(");
        instruction.push_str(&gwei.to_string());
        instruction.push_str(", \"gwei\")}");
        instruction.push_str(", \"\")");
        let output = self.new_command(&instruction);
        let _ = std::str::from_utf8(&output).unwrap();

        // Waiting for the funds to be credited.
        loop {
            if self.provider.get_balance_in_gwei(to).await == balance + gwei {
                break;
            } else {
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        }
    }

    /// Auxiliary.
    fn new_command(&self, instruction: &String) -> Vec<u8> {
        Command::new(GETH)
            .args(["attach", "--exec", instruction, &self.geth_.url])
            .output()
            .unwrap()
            .stdout
    }
}
