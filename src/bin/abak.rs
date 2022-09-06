use abak::aosync;

fn main() {
	let aarg = aarg::parse().unwrap();
	let dry_run = aarg.get("--dry").is_some();
	let check_ratio = if let Some(v) = aarg.get("--check") {
		v[0].parse::<f32>().unwrap()
	} else {
		0.05
	};
	let dirs = aarg.get("").unwrap();
	aosync(&dirs[1], &dirs[2], dry_run, check_ratio);
}
