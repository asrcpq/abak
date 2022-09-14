use rand::Rng;
use std::cmp::Ordering as O;
use std::ffi::OsString;
use std::io::{Read, Seek, Write};
use std::fs::metadata;
use std::os::unix::fs::MetadataExt;
use std::path::PathBuf;

use crate::append_checker::AppendChecker;
use crate::sync_item::SyncItem;
use crate::ord_test::sample_objects;
use crate::BUFLEN;

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
				filelist.push(path.strip_prefix(&root).unwrap().to_path_buf());
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

pub struct Aosync {
	src: PathBuf,
	dst: PathBuf,
	append_checker: AppendChecker,
	dry: bool,
	check: f32,
	buf: [u8; BUFLEN],
	limit: u64,
}

impl Aosync {
	pub fn new(src: &str, dst: &str) -> Self {
		Self {
			src: src.into(),
			dst: dst.into(),
			append_checker: AppendChecker::default(),
			dry: false,
			check: 0.03,
			buf: [0; BUFLEN],
			limit: u64::MAX,
		}
	}

	pub fn set_dry_run(&mut self) {
		self.dry = true;
	}

	pub fn set_check(&mut self, check: f32) {
		self.check = check;
	}

	// limit the update(less then limit, in MiB)
	pub fn set_limit(&mut self, limit: u64) {
		self.limit = limit;
	}

	fn quick_pruning(
		&mut self,
		list_src: &mut Vec<PathBuf>,
		list_dst: &mut Vec<PathBuf>,
	) -> usize {
		// exact path + filename takes priority
		let mut rng = rand::thread_rng();
		let mut iter_src = std::mem::take(list_src)
			.into_iter()
			.peekable();
		let mut iter_dst = std::mem::take(list_dst)
			.into_iter()
			.peekable();
		let mut cmp_count = 0;
		let mut same_count = 0;
		loop {
			let p_src = if let Some(p) = iter_src.peek() {
				p
			} else {
				// iter 1 exhausted
				list_dst.extend(iter_dst);
				break;
			};
			let p_dst = if let Some(p) = iter_dst.peek() {
				p
			} else {
				list_src.extend(iter_src);
				break;
			};
			match p_src.cmp(p_dst) {
				O::Less => {
					// src is too small, increate src, push src to unmatched list
					list_src.push(iter_src.next().unwrap());
				}
				O::Greater => {
					list_dst.push(iter_dst.next().unwrap());
				}
				O::Equal => {
					let p_src = iter_src.next().unwrap();
					let p_dst = iter_dst.next().unwrap();
					let len1 = metadata(self.src.clone().join(&p_src))
						.unwrap()
						.len();
					let len2 = metadata(self.dst.clone().join(&p_dst))
						.unwrap()
						.len();
					if len1 != len2 {
						list_src.push(p_src);
						list_dst.push(p_dst);
					} else {
						// rand f32 gen won't take 0 and 1, so check == 1 is safe for compare all
						if rng.gen::<f32>() < self.check {
							eprintln!(
								"\x1b[2Kc:{} s:{} r:{}",
								cmp_count,
								same_count,
								iter_dst.len(),
							);
							cmp_count += 1;
							if !self.append_checker.is_append_of(
								&self.src.clone().join(&p_src),
								&self.dst.clone().join(&p_dst),
							) {
								panic!(
									"Size match but comparison fail {:?}, corruption?",
									p_dst
								);
							}
						}
						same_count += 1;
					}
				}
			}
		}
		println!(
			"After quick pruning: {} and {} files",
			list_src.len(),
			list_dst.len()
		);
		same_count
	}

	pub fn compute_append_items(&mut self, list_src: &[PathBuf], list_dst: &[PathBuf]) -> Vec<SyncItem> {
		let mut append_items = Vec::new();
		if !list_dst.is_empty() {
			let mut dst_objects = sample_objects(&self.dst, list_dst);
			let src_objects = sample_objects(&self.src, list_src);
			dst_objects.sort_unstable_by_key(|x| x.sample_len);
			for (idx, dst_obj) in dst_objects.iter().rev().enumerate() {
				eprintln!("\x1b[2K{}/{}\r", idx, dst_objects.len());
				// NOTE: replace this brute force method by a more efficient one
				let mut match_idx = None;
				for (idx, src_obj) in src_objects.iter().enumerate() {
					// quick match
					if src_obj.sample_len < dst_obj.sample_len {
						continue;
					}
					if src_obj.sample[0..dst_obj.sample_len]
						!= dst_obj.sample[0..dst_obj.sample_len]
					{
						continue;
					}

					// exact match
					if !self
						.append_checker
						.is_append_of(&src_obj.full_path, &dst_obj.full_path)
					{
						continue;
					}

					if match_idx.is_some() {
						panic!(
							"dst object {:?} matched 2 src objects",
							dst_obj.path
						)
					}
					match_idx = Some(idx)
				}
				let match_idx = if let Some(idx) = match_idx {
					idx
				} else {
					panic!(
						"dst object {:?} matched no src object",
						dst_obj.path
					)
				};
				let dst_len = metadata(&dst_obj.full_path).unwrap().size();
				let src_obj = &src_objects[match_idx];
				let src_len = metadata(&src_obj.full_path).unwrap().size();
				append_items.push(SyncItem {
					src: src_obj.path.clone(),
					dst: dst_obj.path.clone(),
					offset: dst_len,
					len: src_len - dst_len,
				})
			}
		}
		append_items.sort_unstable_by_key(|item| item.src.clone());
		if (1..append_items.len())
			.any(|idx| append_items[idx].src == append_items[idx - 1].src)
		{
			panic!("src object matched 2 dst objects");
		}
		append_items
	}

	fn perform_move_append(&mut self, append_items: &[SyncItem]) -> Vec<(PathBuf, PathBuf)> {
		let mut final_move_list = Vec::new();
		for item in append_items.iter() {
			println!(
				"Append {:?} to {:?}, offset {}",
				item.src, item.dst, item.offset
			);
			if self.dry { continue }
			let concat_src = self.src.clone().join(&item.src);
			let concat_dst = self.dst.clone().join(&item.dst);
			let concat_dst_moved = self.dst.clone().join(&item.src);
			let concat_dst_tmp = build_tmp_path(&concat_dst_moved);
			std::fs::create_dir_all(concat_dst_moved.parent().unwrap())
				.unwrap();
			std::fs::rename(&concat_dst, &concat_dst_tmp).unwrap();
			let mut dst_file = std::fs::OpenOptions::new()
				.append(true)
				.open(&concat_dst_tmp)
				.unwrap();
			let mut src_file = std::fs::File::open(&concat_src).unwrap();
			src_file
				.seek(std::io::SeekFrom::Start(item.offset))
				.unwrap();
			loop {
				let size = src_file.read(&mut self.buf).unwrap();
				if size == 0 {
					break;
				}
				dst_file.write_all(&self.buf[..size]).unwrap();
			}
			final_move_list.push((concat_dst_tmp, concat_dst_moved));
		}
		final_move_list
	}

	fn perform_new(&mut self, new_items: &[SyncItem]) {
		for item in new_items.iter() {
			println!("Create {:?}", item.src);
			if self.dry { continue }
			let concat_src = self.src.clone().join(&item.src);
			let concat_dst = self.dst.clone().join(&item.dst);
			std::fs::create_dir_all(concat_dst.parent().unwrap()).unwrap();
			let mut dst_file = std::fs::OpenOptions::new()
				.create_new(true)
				.write(true)
				.open(&concat_dst)
				.unwrap();
			let mut src_file = std::fs::File::open(&concat_src).unwrap();
			src_file
				.seek(std::io::SeekFrom::Start(item.offset))
				.unwrap();
			loop {
				let size = src_file.read(&mut self.buf).unwrap();
				if size == 0 {
					break;
				}
				dst_file.write_all(&self.buf[..size]).unwrap();
			}
		}
	}

	// only sync file, not directory(but mkdir when necessary)
	pub fn aosync(&mut self) {
		if !std::path::Path::new(&self.dst).exists() {
			std::fs::create_dir(&self.dst).unwrap();
		}

		let mut list_src = get_filelist(self.src.clone());
		list_src.sort_unstable();
		let original_src_len = list_src.len();
		let mut list_dst = get_filelist(self.dst.clone());
		list_dst.sort_unstable();
		println!("Collected {} and {} files", list_src.len(), list_dst.len());

		let same_count = self.quick_pruning(&mut list_src, &mut list_dst);
		let append_items = self.compute_append_items(&list_src, &list_dst);
		let move_len: u64 = append_items.iter().map(|x| x.len).sum();
		let moved_count = append_items.len();
		let mut new_items = Vec::new();
		let mut new_len = 0;
		for path in list_src.into_iter() {
			if append_items
				.binary_search_by_key(&path, |item| item.src.clone())
				.is_ok()
			{
				continue;
			}
			let len = std::fs::metadata(self.src.clone().join(&path))
				.unwrap()
				.size();
			new_items.push(SyncItem {
				src: path.clone(),
				dst: path,
				offset: 0,
				len,
			});
			new_len += len;
		}
		let new_count = new_items.len();
		println!("Summary:");
		println!("create: {} = {}M", new_count, new_len / (1 << 20));
		println!("append: {} = {}M", moved_count, move_len / (1 << 20));
		println!("same: {}", same_count);
		let sum = new_count + moved_count + same_count;
		assert_eq!(sum, original_src_len);
		if new_len + move_len >= self.limit {
			println!("Limit exceeded, exit");
			panic!("Limit exceeded, exit")
		}

		// perform the update
		let final_move_list = self.perform_move_append(&append_items);
		self.perform_new(&new_items);
		for (src, dst) in final_move_list.into_iter() {
			std::fs::rename(&src, &dst).unwrap();
		}
	}
}
