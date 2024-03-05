#![allow(unused_variables)]
//#!{allow(unused_variables)}

use sailfish::TemplateOnce;
use std::path::Path;

#[derive(TemplateOnce)]
#[template(path = "cargo.toml.stpl")]
pub struct CargoToml<'a> {
	pub runtime_name: &'a str,
	pub source_repo: &'a str,
	pub source_rev: &'a str,
}

#[derive(TemplateOnce)]
#[template(path = "lib.rs.stpl")]
pub struct LibRs<'a> {
	pub snap_path: &'a Path,
	pub raw_block_path: &'a Path,
}
