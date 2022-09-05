use std::cmp::Ordering as O;
use std::path::PathBuf;

struct SyncItem {
	path: PathBuf,
	// since append only, offset = 0 means dst is empty or nonexist
	offset: usize,
}

fn get_filelist(root: PathBuf) -> Vec<PathBuf> {
	let mut queue: Vec<PathBuf> = vec![root.clone()];
	let mut filelist: Vec<PathBuf> = Vec::new();
	while let Some(object) = queue.pop() {
		for entry in std::fs::read_dir(object).unwrap() {
			let entry = entry.unwrap();
			let ty = entry.file_type().unwrap();
			let path = entry.path();
			if ty.is_file() {
				filelist.push(
					path.strip_prefix(&root)
						.unwrap()
						.to_path_buf()
				);
			} else if ty.is_dir() {
				queue.push(path);
			} else {
				panic!("Unknown type: {:?}", path)
			}
		}
	}
	filelist
}

// only sync file, not directory(but mkdir when necessary)
pub fn aosync(src: &str, dst: &str) {
	let src = PathBuf::from(src);
	let dst = PathBuf::from(dst);
	if !std::path::Path::new(&dst).exists() {
		std::fs::create_dir(&dst).unwrap();
	}
	let mut list_src = get_filelist(src.clone());
	list_src.sort_unstable();
	let mut list_dst = get_filelist(dst.clone());
	list_dst.sort_unstable();
	eprintln!("Collected {} and {} files", list_src.len(), list_dst.len());

	// exact path + filename takes priority
	let mut iter_src = list_src.into_iter().peekable();
	let mut iter_dst = list_dst.into_iter().peekable();
	let mut list_src = Vec::new();
	let mut list_dst = Vec::new();
	loop {
		let p_src = if let Some(p) = iter_src.peek() {
			p.clone()
		} else {
			// iter 1 exhausted
			list_dst.extend(iter_dst);
			break
		};
		let p_dst = if let Some(p) = iter_dst.peek() {
			p.clone()
		} else {
			list_src.extend(iter_src);
			break
		};
		match p_src.cmp(&p_dst) {
			O::Less => {
				// src is too small, increate src, push src to unmatched list
				list_src.push(iter_src.next().unwrap());
			},
			O::Greater => {
				list_dst.push(iter_dst.next().unwrap());
			}
			O::Equal => {
				iter_src.next();
				iter_dst.next();
				let len1 = std::fs::metadata(src.clone().join(p_src))
					.unwrap()
					.len();
				let len2 = std::fs::metadata(dst.clone().join(p_dst))
					.unwrap()
					.len();
				if len1 != len2 {
					unimplemented!("Cannot handle appended file");
				}
			},
		}
	}

	if !list_dst.is_empty() {
		unimplemented!("cannot handle moved files");
	}

	// perform the update
	for remain_src in list_src.into_iter() {
		let src_path = src.clone().join(&remain_src);
		let dst_path = dst.clone().join(&remain_src);
		std::fs::create_dir_all(dst_path.parent().unwrap()).unwrap();
		std::fs::copy(&src_path, &dst_path).unwrap();
		eprintln!("{:?} -> {:?}", src_path, dst_path);
	}
}
