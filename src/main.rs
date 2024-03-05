mod render;

use std::process::Command;
use std::path::PathBuf;
use anyhow::Result;
use anyhow::anyhow;
use clap::Parser;
use std::env;
use std::process::Stdio;
use std::path::Path;
use sailfish::TemplateOnce;
use parity_scale_codec::Encode;

use subxt::{OnlineClient, PolkadotConfig, utils::H256};

type Api = OnlineClient::<PolkadotConfig>;

#[derive(Parser, Clone)]
pub struct Cmd {
    /// RPC endpoint to query from. Must be an archive node or still have the block cached.
    #[clap(long, required = true)]
    pub rpc: String,

    /// Block to re-run.
    #[clap(long, required = true)]
    pub block: String,

    /// Name of the runtime (excluding the `-runtime`) suffix.
    #[clap(long)]
    pub runtime_name: String,

    /// GitHub repo name in the form of `org/project` of the runtime.
    #[clap(long)]
    pub source_repo: String,

    /// Git commit hash of the runtime.
    #[clap(long)]
    pub source_rev: String,

    /// Force overwrite of existing project.
    #[clap(long)]
    pub force: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    init_log();

    let cmd = Cmd::parse();
    cmd.run().await?;

    Ok(())
}

impl Cmd {
    pub async fn run(&self) -> Result<()> {
        self.validate_args()?;
        let api = OnlineClient::<PolkadotConfig>::from_url(&self.rpc).await?;

        let parent = self.find_parent_block(&api).await?;
        let raw_block = self.download_raw_block(&api).await?;

        let snap = self.create_snap(parent)?;
        let project = self.setup_project(&snap, &raw_block)?;

        // move snap to project folder
        let snap_path = project.join(snap.file_name().unwrap());
        std::fs::rename(&snap, &snap_path)?;
        // move raw block to project folder
        let raw_block_path = project.join(raw_block.file_name().unwrap());
        std::fs::rename(&raw_block, &raw_block_path)?;

        self.download_lockfile(&project).await?;

        Ok(())
    }

    async fn download_lockfile(&self, project: &Path) -> Result<()> {
        let url = format!("https://raw.githubusercontent.com/{}/{}/Cargo.lock", self.source_repo, self.source_rev);
        let body = reqwest::get(&url)
            .await?
            .text()
            .await?;
        let lockfile = project.join("Cargo.lock");
        std::fs::write(&lockfile, body)?;

        Ok(())
    }

    fn setup_project(&self, snap: &Path, raw_block: &Path) -> Result<PathBuf> {
        let path = PathBuf::from("replay");
        if path.exists() {
            if self.force {
                log::info!("Removing existing project at: {:?}", path);
                std::fs::remove_dir_all(&path)?;
            } else {
                return Err(anyhow!("Project already exists at: {:?}", path));
            }
        }

        // create dir
        std::fs::create_dir(&path)?;
        // create Cargo.toml
        let cargo_toml = path.join("Cargo.toml");
        std::fs::write(&cargo_toml, render::CargoToml {
            runtime_name: &self.runtime_name,
            source_repo: &self.source_repo,
            source_rev: &self.source_rev,
        }.render_once()?)?;
        // create lib.rs
        let lib_rs = path.join("src").join("lib.rs");
        std::fs::create_dir_all(lib_rs.parent().unwrap())?;
        std::fs::write(&lib_rs, render::LibRs {
            snap_path: snap,
            raw_block_path: raw_block,
        }.render_once()?)?;

        if !path.exists() || !path.is_dir() {
            return Err(anyhow!("Failed to create project"));
        }
        log::info!("Project created at: {:?}", path);

        Ok(path)
    }

    async fn find_parent_block(&self, api: &Api) -> Result<H256> {
        log::info!("Runtime version: {}", api.runtime_version().spec_version);
        let block_hash = array_bytes::hex2bytes(&self.block).map_err(|_| anyhow!("Invalid block hash"))?;
        let block_hash = H256::from_slice(&block_hash);
        log::info!("Finding parent block of: {:?}", block_hash);
        let block = api.blocks().at(block_hash).await?;
        let parent = block.header().parent_hash;
        log::info!("Parent block: {:?}", parent);

        Ok(parent)
    }

    async fn download_raw_block(&self, api: &Api) -> Result<PathBuf> {
        log::info!("Downloading raw block");
        let hash = array_bytes::hex2bytes(&self.block).map_err(|_| anyhow!("Invalid block hash"))?;
        let hash = H256::from_slice(&hash);
        let filename = format!("block-{}.raw", array_bytes::bytes2hex("0x", &hash.0));
        let path = PathBuf::from(&filename);

        if path.exists() {
            log::info!("Block already exists at: {:?}", path);
            return Ok(path);
        }

        let header = api.backend().block_header(hash).await?.ok_or(anyhow!("Block not found"))?;
        let extrinsics = api.backend().block_body(hash).await?.ok_or(anyhow!("Block not found"))?;
        // concat all extrinsics
        let mut raw_extrinsics = Vec::new();
        for extrinsic in extrinsics.iter() {
            raw_extrinsics.extend(extrinsic.encode());
        }
        
        let raw = header.encode();
        std::fs::write(&path, raw)?;
        assert!(path.exists());
        log::info!("Block downloaded at: {:?}", path);

        Ok(path)
    }

    fn create_snap(&self, block: H256) -> Result<PathBuf> {
        let block = array_bytes::bytes2hex("0x", &block.0);
        let filename = format!("snap-{}.raw", block);
        let path = PathBuf::from(&filename);

        if path.exists() {
            log::info!("Snap already exists at: {:?}", path);
            return Ok(path);
        }

        let mut cmd = Command::new("try-runtime");
        cmd.arg("create-snapshot")
            .args(["--uri", &self.rpc])
            .args(["--at", &block])
            .arg(&filename)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        log::debug!("Running command: {:?}", cmd);
        if !cmd.output()?.status.success() {
            return Err(anyhow!("Failed to create snap"));
        }

        if !path.exists() {
            return Err(anyhow!("Failed to create snap"));
        }
        log::info!("Snap created at: {:?}", path);

        Ok(path)
    }

    fn validate_args(&self) -> Result<()> {
        if !self.rpc.starts_with("wss://") && !self.rpc.starts_with("ws://") {
            return Err(anyhow!("Need wss or ws RPC url"));
        }
        if !self.block.starts_with("0x") {
            return Err(anyhow!("Block argument must be a block hash starting with 0x"));
        }

        Ok(())
    }
}

fn init_log() {
    if env::var(env_logger::DEFAULT_FILTER_ENV).is_err() {
        env::set_var(env_logger::DEFAULT_FILTER_ENV, "info");
    }
    env_logger::init();
}
