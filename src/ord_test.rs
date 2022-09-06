use std::io::Read;
use std::path::{Path, PathBuf};

const SAMPLE_LEN: usize = 4096;

pub struct OrdTest {
	pub full_path: PathBuf,
	pub path: PathBuf,
	pub sample: [u8; SAMPLE_LEN],
	pub sample_len: usize,
}

pub fn sample_objects(prefix: &Path, path_list: &[PathBuf]) -> Vec<OrdTest> {
	let mut objects = Vec::new();
	for path in path_list.iter() {
		let full_path = prefix.to_path_buf().join(path);
		let mut file = std::fs::File::open(&full_path).unwrap();
		let mut chunk = [0u8; SAMPLE_LEN];
		let sample_len = file.read(&mut chunk).unwrap();
		objects.push(OrdTest {
			full_path,
			path: path.clone(),
			sample: chunk,
			sample_len,
		});
	}
	objects
}
