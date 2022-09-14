use abak::aosync::Aosync;

fn main() {
	let aarg = aarg::parse().unwrap();
	let dirs = aarg.get("").unwrap();
	let mut abak = Aosync::new(&dirs[1], &dirs[2]);
	if aarg.get("--dry").is_some() {
		abak.set_dry_run();
	}
	if let Some(v) = aarg.get("--check") {
		abak.set_check(v[0].parse::<f32>().unwrap());
	};
	if let Some(v) = aarg.get("--limit") {
		abak.set_limit(v[0].parse::<u64>().unwrap());
	};
	abak.aosync();
}
