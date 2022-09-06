use std::io::Read;
use std::os::unix::fs::MetadataExt;
use std::path::PathBuf;

use crate::BUFLEN;

pub struct AppendChecker {
	buf1: [u8; BUFLEN],
	buf2: [u8; BUFLEN],
}

impl Default for AppendChecker {
	fn default() -> Self {
		Self {
			buf1: [0; BUFLEN],
			buf2: [0; BUFLEN],
		}
	}
}

impl AppendChecker {
	// src can be longer than dst
	pub fn is_append_of(&mut self, src: &PathBuf, dst: &PathBuf) -> bool {
		let mut src_reader = std::fs::File::open(src).unwrap();
		let size = std::fs::metadata(dst).unwrap().size() / BUFLEN as u64;
		let show_progress = size > 100;
		let mut dst_reader = std::fs::File::open(dst).unwrap();
		let mut count = 0;
		loop {
			let size2 = dst_reader.read(&mut self.buf2).unwrap();
			if size2 == 0 {
				break true;
			}
			let _ = src_reader.read(&mut self.buf1).unwrap();
			// dst is shorter
			if self.buf1[..size2] != self.buf2[..size2] {
				break false;
			}

			if show_progress {
				eprint!("\x1b[2Kprogress {}/{}\r", count, size);
				count += 1;
			}
		}
	}
}
