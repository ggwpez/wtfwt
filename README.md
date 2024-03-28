# What The Fuck Was That?

Find out what just happened to your Substrate Transaction by attaching a Rust debugger and re-executing it in a unit test environment.
You can either manually create a call or decode and rerun the actual block.

# Example

Investigating a recent issue on [SE](https://substrate.stackexchange.com/questions/11228) about trapped assets.  
First gather the information of the block hash, runtime and runtime verison.  
You also need an **archive node** in order to acquire the correct state at the time of the block. In this example, we run one locally:

```sh
wtfwt --rpc ws://127.0.0.1:9944 --block 0x053fdbedde5c2439025e582681711d09b41b93d19dd60db389c7e5f35b3ee597 --runtime-name polkadot --source-repo "polkadot-fellows/runtimes" --source-rev "v1.1.2" --force
```

Should download the state:  
```pre
Runtime version: 1001002
Finding parent block of: 0x053fdbedde5c2439025e582681711d09b41b93d19dd60db389c7e5f35b3ee597
Parent block: 0x9b254ccf97d1472627fb5349e0eb2fd3c05d7abd749856cf61908f04f64dbaeb
Downloading raw block
Block already exists at: "block-0x053fdbedde5c2439025e582681711d09b41b93d19dd60db389c7e5f35b3ee597.raw"
replacing ws:// in uri with http://: "http://127.0.0.1:9955" (ws is currently unstable for fetching remote storage, for more see https://github.com/paritytech/jsonrpsee/issues/1086)
since no prefix is filtered, the data for all pallets will be downloaded
scraping key-pairs from remote at block height 0x9b254ccf97d1472627fb5349e0eb2fd3c05d7abd749856cf61908f04f64dbaeb
✅ Found 2154512 keys (18.68s)
Downloading key values 42,055.4939/s [===================>-------------------------------------------------------------------------] 462436/2154512 (40s)
```

This will create a `replay` directory. You can open this folder now with your IDE and rust-analyezr enabled.  
The `replay` function can be used as playground to either write a normal rust-unit test or inspect / replay the transactions of a specific block.

```rust
fn replay(block: Block) {
	Executive::initialize_block(&block.header);

	for extrinsic in block.extrinsics {
		let _ = Executive::apply_extrinsic(extrinsic);
	}

	eprintln!("Events: {:#?}", System::events());

	let _ = Executive::finalize_block();
}
```

This function acts as a Rust-unit test and can be debugged with normal Rust debugging tools. The `#[test]` attribute is on its closure function, that provides externalities and the decoded state.  

You can run it with:
```sh
cargo test --release -- --nocapture
```
