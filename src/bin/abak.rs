use abak::aosync;

fn main() {
	let mut iter = std::env::args();
	iter.next();
	let src = iter.next().unwrap();
	let dst = iter.next().unwrap();
	aosync(&src, &dst);
}
