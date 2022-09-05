use std::cmp::Ordering as O;
use std::path::PathBuf;
use std::os::unix::fs::MetadataExt;
use std::io::{Read, Write, Seek};
use std::ffi::OsString;

struct SyncItem {
	pub src: PathBuf,
	pub dst: PathBuf,
	// since append only, offset = 0 means dst is empty or nonexist
	pub offset: u64,
}

fn build_tmp_path(path: &PathBuf) -> PathBuf {
	let mut string: OsString = path.into();
	loop {
		string.push(".abakt");
		if !std::path::Path::new(&string).exists() {
			return string.into();
		}
	}
}

fn get_filelist(root: PathBuf) -> Vec<PathBuf> {
	let mut queue: Vec<PathBuf> = vec![root.clone()];
	let mut filelist: Vec<PathBuf> = Vec::new();
	let mut count = 0;
	while let Some(object) = queue.pop() {
		eprint!("\x1b[2K{}/{}\r", queue.len(), count);
		for entry in std::fs::read_dir(object).unwrap() {
			let entry = entry.unwrap();
			let ty = entry.file_type().unwrap();
			let path = entry.path();
			if ty.is_file() {
				count += 1;
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
	eprintln!();
	filelist
}

const SAMPLE_LEN: usize = 4096;

struct OrdTest {
	full_path: PathBuf,
	path: PathBuf,
	sample: [u8; SAMPLE_LEN],
	sample_len: usize,
}

fn sample_objects(prefix: &PathBuf, path_list: &[PathBuf]) -> Vec<OrdTest> {
	let mut objects = Vec::new();
	for path in path_list.iter() {
		let full_path = prefix.clone().join(path);
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

// only sync file, not directory(but mkdir when necessary)
pub fn aosync(src: &str, dst: &str, dry_run: bool) {
	let src = PathBuf::from(src);
	let dst = PathBuf::from(dst);
	let mut buf1 = [0u8; 1 << 20];
	let mut buf2 = [0u8; 1 << 20];
	if !std::path::Path::new(&dst).exists() {
		std::fs::create_dir(&dst).unwrap();
	}
	let mut list_src = get_filelist(src.clone());
	list_src.sort_unstable();
	let original_src_len = list_src.len();
	let mut list_dst = get_filelist(dst.clone());
	list_dst.sort_unstable();
	eprintln!("Collected {} and {} files", list_src.len(), list_dst.len());

	// exact path + filename takes priority
	let mut iter_src = list_src.into_iter().peekable();
	let mut iter_dst = list_dst.into_iter().peekable();
	let mut list_src = Vec::new();
	let mut list_dst = Vec::new();
	let mut same_count = 0;
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
				let p_src = iter_src.next().unwrap();
				let p_dst = iter_dst.next().unwrap();
				let len1 = std::fs::metadata(src.clone().join(&p_src))
					.unwrap()
					.len();
				let len2 = std::fs::metadata(dst.clone().join(&p_dst))
					.unwrap()
					.len();
				if len1 != len2 {
					list_src.push(p_src);
					list_dst.push(p_dst);
				} else {
					same_count += 1;
				}
				// TODO: option full compare content of src and dst
			},
		}
	}
	eprintln!("After quick pruning: {} and {} files", list_src.len(), list_dst.len());

	// compute append item
	let mut append_items = Vec::new();
	let mut new_items = Vec::new();
	if !list_dst.is_empty() {
		let mut dst_objects = sample_objects(&dst, &list_dst);
		let src_objects = sample_objects(&src, &list_src);
		dst_objects.sort_unstable_by_key(|x| x.sample_len);
		for dst_obj in dst_objects.iter().rev() {
			// NOTE: replace this brute force method by a more efficient one
			let mut match_idx = None;
			for (idx, src_obj) in src_objects.iter().enumerate() {
				// quick match
				if src_obj.sample_len < dst_obj.sample_len {
					continue
				}
				if src_obj.sample[0..dst_obj.sample_len] !=
					dst_obj.sample[0..dst_obj.sample_len]
				{
					continue
				}

				// exact match
				let mut src_reader = std::fs::File::open(&src_obj.full_path).unwrap();
				let mut dst_reader = std::fs::File::open(&dst_obj.full_path).unwrap();
				let success = loop {
					let size2 = dst_reader.read(&mut buf2).unwrap();
					if size2 == 0 { break true }
					src_reader.read(&mut buf1).unwrap();
					// dst is shorter
					if buf1[..size2] != buf2[..size2] { break false }
				};
				if !success {
					continue
				}

				if match_idx.is_some() {
					panic!("dst object {:?} matched 2 src objects", dst_obj.path)
				}
				match_idx = Some(idx)
			}
			let match_idx = if let Some(idx) = match_idx {
				idx
			} else {
				panic!("dst object {:?} matched no src object", dst_obj.path)
			};
			append_items.push(SyncItem {
				src: src_objects[match_idx].path.clone(),
				dst: dst_obj.path.clone(),
				offset: std::fs::metadata(&dst_obj.full_path).unwrap().size(),
			})
		}
	}
	append_items.sort_unstable_by_key(|item| item.src.clone());
	if (1..append_items.len())
		.any(|idx| append_items[idx].src == append_items[idx - 1].src)
	{
		panic!("src object matched 2 dst objects");
	}
	let moved_count = append_items.len();
	for path in list_src.into_iter() {
		if append_items.binary_search_by_key(&path, |item| item.src.clone()).is_ok() {
			continue
		}
		new_items.push(SyncItem {
			src: path.clone(),
			dst: path,
			offset: 0,
		});
	}
	let append_count = new_items.len();
	eprintln!("Summary: {} new files + {} moved files + {} same files",
		append_count,
		moved_count,
		same_count,
	);
	let sum = append_count + moved_count + same_count;
	assert_eq!(sum, original_src_len);

	// perform the update
	for item in new_items.iter() {
		if dry_run {
			eprintln!("Sync {:?} to {:?}, offset {}", item.src, item.dst, item.offset);
			continue
		}
		let concat_src = src.clone().join(&item.src);
		let concat_dst = dst.clone().join(&item.dst);
		std::fs::create_dir_all(concat_dst.parent().unwrap()).unwrap();
		let mut dst_file = std::fs::OpenOptions::new()
			.create_new(true)
			.write(true)
			.open(&concat_dst)
			.unwrap();
		let mut src_file = std::fs::File::open(&concat_src).unwrap();
		src_file.seek(std::io::SeekFrom::Start(item.offset)).unwrap();
		loop {
			let size = src_file.read(&mut buf1).unwrap();
			if size == 0 {
				break
			}
			dst_file.write_all(&buf1[..size]).unwrap();
		}
	}

	let mut final_move_list = Vec::new();
	for item in append_items.iter() {
		if dry_run {
			eprintln!("Sync {:?} to {:?}, offset {}", item.src, item.dst, item.offset);
			continue
		}
		let concat_src = src.clone().join(&item.src);
		let concat_dst = dst.clone().join(&item.dst);
		let concat_dst_moved = dst.clone().join(&item.src);
		let concat_dst_tmp = build_tmp_path(&concat_dst_moved);
		std::fs::create_dir_all(concat_dst_moved.parent().unwrap()).unwrap();
		std::fs::rename(&concat_dst, &concat_dst_tmp).unwrap();
		let mut dst_file = std::fs::OpenOptions::new()
			.append(true)
			.open(&concat_dst_tmp)
			.unwrap();
		let mut src_file = std::fs::File::open(&concat_src).unwrap();
		src_file.seek(std::io::SeekFrom::Start(item.offset)).unwrap();
		loop {
			let size = src_file.read(&mut buf1).unwrap();
			if size == 0 {
				break
			}
			dst_file.write_all(&buf1[..size]).unwrap();
		}
		final_move_list.push((concat_dst_tmp, concat_dst_moved));
	}

	for (src, dst) in final_move_list.into_iter() {
		std::fs::rename(&src, &dst).unwrap();
	}
}
