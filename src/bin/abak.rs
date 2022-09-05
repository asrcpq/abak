use abak::aosync;

fn main() {
	let aarg = aarg::parse().unwrap();
	let dry_run = aarg.get("--dry").is_some();
	let dirs = aarg.get("").unwrap();
	aosync(&dirs[1], &dirs[2], dry_run);
}
