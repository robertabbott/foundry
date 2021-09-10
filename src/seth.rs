//! Seth
//!
//! TODO
use ethers::{
    abi::{ParamType, Tokenizable},
    core::abi::parse_abi,
    providers::{self, Http, Middleware, Provider},
    types::*,
    utils,
};
use eyre::Result;
use rustc_hex::ToHex;
use std::convert::TryFrom;
use std::str::FromStr;

// TODO: SethContract with common contract initializers? Same for SethProviders?

pub struct Seth {
    provider: Provider<Http>,
}

fn to_table(value: serde_json::Value) -> String {
    match value {
        serde_json::Value::String(s) => s,
        serde_json::Value::Object(map) => {
            let mut s = String::new();
            for (k, v) in map.iter() {
                s.push_str(&format!("{: <20} {}\n", k, v));
            }
            s
        }
        _ => "".to_owned(),
    }
}

impl Seth {
    /// Converts ASCII text input to hex
    ///
    /// ```
    /// use dapptools::Seth;
    ///
    /// # async fn foo() -> eyre::Result<()> {
    /// let seth = Seth::new("http://localhost:8545").await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn new(rpc_url: &str) -> Result<Self> {
        let provider = providers::Provider::try_from(rpc_url)?;
        Ok(Self { provider })
    }

    // TODO: `send`, same story but sending the tx.
    /// ```no_run
    /// use dapptools::Seth;
    /// use dapptools::ethers::types::Address;
    /// use std::str::FromStr;
    ///
    /// # async fn foo() -> eyre::Result<()> {
    /// let seth = Seth::new("http://localhost:8545").await?;
    /// let to = Address::from_str("0xB3C95ff08316fb2F2e3E52Ee82F8e7b605Aa1304")?;
    /// let sig = "function greeting(uint256 i) public returns (string)";
    /// let args = vec!["5".to_owned()];
    /// let data = seth.call(to, sig, args).await?;
    /// println!("{}", data);
    /// # Ok(())
    /// # }
    /// ```
    pub async fn call(&self, to: Address, sig: &str, args: Vec<String>) -> Result<String> {
        // TODO: Make human readable ABI better / more minimal
        let abi = parse_abi(&[sig])?;
        // get the function
        let func = {
            let (_, func) = abi
                .functions
                .iter()
                .next()
                .ok_or_else(|| eyre::eyre!("function name not found"))?;
            let func = func
                .get(0)
                .ok_or_else(|| eyre::eyre!("functions array empty"))?;
            if args.len() != func.inputs.len() {
                eyre::bail!("function inputs do len does not match provided args len");
            }
            func
        };

        // Dynamically build up the calldata via the function sig
        let mut inputs = Vec::new();
        for (i, input) in func.inputs.iter().enumerate() {
            let input = match input.kind {
                // TODO: Do the rest of the types
                ParamType::Address => Address::from_str(&args[i])?.into_token(),
                ParamType::Uint(256) => U256::from_str(&args[i])?.into_token(),
                _ => Address::zero().into_token(),
            };
            inputs.push(input);
        }

        // encode args
        let data = func.encode_input(&inputs)?;

        // make the call
        let tx = Eip1559TransactionRequest::new().to(to).data(data).into();
        let res = self.provider.call(&tx, None).await?;

        // decode args into tokens
        let res = func.decode_output(res.as_ref())?;

        // concatenate them
        let mut s = Vec::new();
        for output in res {
            s.push(format!("{}", output));
        }

        // return string
        Ok(s.join(","))
    }

    /// ```no_run
    /// use dapptools::Seth;
    ///
    /// # async fn foo() -> eyre::Result<()> {
    /// let seth = Seth::new("http://localhost:8545").await?;
    /// let block = seth.block(5, true, None, false).await?;
    /// println!("{}", block);
    /// # Ok(())
    /// # }
    /// ```
    pub async fn block<T: Into<BlockId>>(
        &self,
        block: T,
        full: bool,
        field: Option<String>,
        to_json: bool,
    ) -> Result<String> {
        let block = block.into();
        let block = if full {
            let block = self
                .provider
                .get_block_with_txs(block)
                .await?
                .ok_or_else(|| eyre::eyre!("block {:?} not found", block))?;
            if let Some(ref field) = field {
                // TODO: Use custom serializer to serialize
                // u256s as decimals
                serde_json::to_value(&block)?
                    .get(field)
                    .cloned()
                    .ok_or_else(|| eyre::eyre!("field {} not found", field))?
            } else {
                serde_json::to_value(&block)?
            }
        } else {
            let block = self
                .provider
                .get_block(block)
                .await?
                .ok_or_else(|| eyre::eyre!("block {:?} not found", block))?;
            if let Some(ref field) = field {
                serde_json::to_value(block)?
                    .get(field)
                    .cloned()
                    .ok_or_else(|| eyre::eyre!("field {} not found", field))?
            } else {
                serde_json::to_value(&block)?
            }
        };

        let block = if to_json {
            serde_json::to_string(&block)?
        } else {
            to_table(block)
        };

        Ok(block)
    }

    /// Converts ASCII text input to hex
    ///
    /// ```
    /// use dapptools::Seth;
    ///
    /// let bin = Seth::from_ascii("yo");
    /// assert_eq!(bin, "0x796f")
    /// ```
    pub fn from_ascii(s: &str) -> String {
        let s: String = s.as_bytes().to_hex();
        format!("0x{}", s)
    }

    /// Converts an Ethereum address to its checksum format
    /// according to [EIP-55](https://github.com/ethereum/EIPs/blob/master/EIPS/eip-55.md)
    ///
    /// ```
    /// use dapptools::Seth;
    /// use dapptools::ethers::types::Address;
    /// use std::str::FromStr;
    ///
    /// # fn main() -> eyre::Result<()> {
    /// let addr = Address::from_str("0xb7e390864a90b7b923c9f9310c6f98aafe43f707")?;
    /// let addr = Seth::to_checksum_address(&addr)?;
    /// assert_eq!(addr, "0xB7e390864a90b7b923C9f9310C6F98aafE43F707");
    ///
    /// # Ok(())
    /// # }
    /// ```
    pub fn to_checksum_address(address: &Address) -> Result<String> {
        Ok(utils::to_checksum(address, None))
    }

    /// Converts hexdata into bytes32 value
    /// ```
    /// use dapptools::Seth;
    ///
    /// # fn main() -> eyre::Result<()> {
    /// let bytes = Seth::to_bytes32("1234")?;
    /// assert_eq!(bytes, "0x1234000000000000000000000000000000000000000000000000000000000000");
    ///
    /// let bytes = Seth::to_bytes32("0x1234")?;
    /// assert_eq!(bytes, "0x1234000000000000000000000000000000000000000000000000000000000000");
    ///
    /// let err = Seth::to_bytes32("0x123400000000000000000000000000000000000000000000000000000000000011").unwrap_err();
    /// assert_eq!(err.to_string(), "string >32 bytes");
    ///
    /// # Ok(())
    /// # }
    pub fn to_bytes32(s: &str) -> Result<String> {
        let s = strip_0x(s);
        if s.len() > 64 {
            eyre::bail!("string >32 bytes");
        }

        let padded = format!("0x{:0<64}", s);
        // need to use the Debug implementation
        Ok(format!("{:?}", H256::from_str(&padded)?))
    }
}

fn strip_0x(s: &str) -> &str {
    s.strip_prefix("0x").unwrap_or(s)
}