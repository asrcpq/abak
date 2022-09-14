use std::path::PathBuf;

pub struct SyncItem {
	pub src: PathBuf,
	pub dst: PathBuf,
	// since append only, offset = 0 means dst is empty or nonexist
	pub offset: u64,
	pub len: u64,
}
